//! Bounded, local-only SDR video sampling. No playback service or network inputs.
use crate::{color::Color, inputs::MediaInput};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub const MODEL: &str = "center-crop-160x90-grid8-video5-v2";

/// Decoders must not outlive a cancelled audit (the panel kills the helper on close).
pub(crate) fn die_with_parent(command: &mut Command) {
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}
pub(crate) fn run(mut command: Command, timeout: Duration, limit: usize) -> Result<Vec<u8>> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    die_with_parent(&mut command);
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        command.pre_exec(|| {
            let memory = libc::rlimit {
                rlim_cur: 2 * 1024 * 1024 * 1024,
                rlim_max: 2 * 1024 * 1024 * 1024,
            };
            let cpu = libc::rlimit {
                rlim_cur: 8,
                rlim_max: 8,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &memory) != 0
                || libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("media decoder unavailable")?;
    let stdout = child.stdout.take().context("Missing video stdout")?;
    let stderr = child.stderr.take().context("Missing video stderr")?;
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let err = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stderr.take(16385).read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let bytes = out
        .join()
        .map_err(|_| anyhow::anyhow!("media output reader failed"))??;
    let errors = err
        .join()
        .map_err(|_| anyhow::anyhow!("media error reader failed"))??;
    let status = status.context("media decoding deadline exceeded")?;
    if bytes.len() > limit || errors.len() > 16384 {
        bail!("media decoder output limit")
    }
    if !status.success() {
        let message = String::from_utf8_lossy(&errors);
        if message.contains("Invalid data found when processing input")
            || message.contains("moov atom not found")
        {
            bail!("invalid-video: malformed container or sampled frame")
        }
        bail!("media decoding unavailable, resource limited, or failed")
    }
    Ok(bytes)
}

pub fn backend_identity() -> String {
    static IDENTITY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    IDENTITY.get_or_init(probe_identity).clone()
}
fn probe_identity() -> String {
    ["ffprobe", "ffmpeg"]
        .iter()
        .map(|tool| {
            let mut command = Command::new(tool);
            command.arg("-version");
            match run(command, Duration::from_secs(2), 16384) {
                Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                Err(_) => format!("{tool}: unavailable"),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn input(command: &mut Command, video: &MediaInput) {
    command.args(["-protocol_whitelist", "file,pipe", "-f", &video.format]);
    if video.format == "mov" {
        command.args(["-enable_drefs", "0", "-use_absolute_path", "0"]);
    }
}

pub struct Samples {
    pub colors: Vec<Color>,
    pub evidence: Value,
}

pub fn sample(video: &MediaInput) -> Result<Samples> {
    let mut probe = Command::new("ffprobe");
    probe.args(["-v", "error", "-threads", "1", "-max_alloc", "268435456"]);
    input(&mut probe, video);
    probe.args(["-select_streams", "v:0", "-show_entries", "stream=index,width,height,pix_fmt,color_space,color_transfer,color_primaries,color_range,duration,avg_frame_rate,codec_name:stream_side_data:format=duration", "-of", "json"]).arg(video.file.path());
    let metadata: Value = serde_json::from_slice(&run(probe, Duration::from_secs(2), 16384)?)?;
    let stream = metadata["streams"]
        .as_array()
        .and_then(|v| v.first())
        .context("video has no available picture stream")?;
    let width = stream["width"]
        .as_u64()
        .context("video width unavailable")?;
    let height = stream["height"]
        .as_u64()
        .context("video height unavailable")?;
    if width == 0 || height == 0 || width > 8192 || height > 8192 || width * height > 32_000_000 {
        bail!("video dimension/pixel limit")
    }
    let pixel_format = stream["pix_fmt"].as_str().unwrap_or("");
    if !matches!(
        pixel_format,
        "yuv420p"
            | "yuvj420p"
            | "yuv422p"
            | "yuvj422p"
            | "yuv444p"
            | "yuvj444p"
            | "nv12"
            | "rgb24"
            | "bgr24"
            | "gbrp"
    ) {
        bail!("video pixel format requires unsupported alpha, bit depth, or color management")
    }
    for key in ["color_space", "color_transfer", "color_primaries"] {
        if let Some(value) = stream[key].as_str()
            && !matches!(
                value,
                "unknown" | "unspecified" | "bt709" | "gbr" | "iec61966-2-1"
            )
        {
            bail!("video color metadata requires unsupported color management: {key}={value}")
        }
    }
    let duration = stream["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| {
            metadata["format"]["duration"]
                .as_str()
                .and_then(|s| s.parse().ok())
        })
        .context("video duration unavailable")?;
    if !duration.is_finite() || duration <= 0. || duration > 600. {
        bail!("video duration outside bounded 0–600 second model")
    }
    let rate = stream["avg_frame_rate"]
        .as_str()
        .and_then(|s| s.split_once('/'))
        .and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?))
        .filter(|rate| rate.is_finite() && *rate > 0. && *rate <= 240.)
        .context("video frame rate outside bounded model")?;
    let last = (duration - 1. / rate).max(0.);
    let mut colors = Vec::new();
    let mut timestamps = Vec::new();
    let started = Instant::now();
    for fraction in [0., 0.25, 0.5, 0.75, 0.95] {
        if started.elapsed() > Duration::from_secs(10) {
            bail!("video sampling deadline exceeded")
        }
        let timestamp = (fraction * duration).min(last);
        if timestamps.contains(&timestamp) {
            continue;
        }
        timestamps.push(timestamp);
        let mut decode = Command::new("ffmpeg");
        decode.args([
            "-nostdin",
            "-v",
            "error",
            "-max_alloc",
            "268435456",
            "-threads",
            "1",
        ]);
        input(&mut decode, video);
        decode
            .args(["-ss", &format!("{timestamp:.6}"), "-i"])
            .arg(video.file.path());
        decode.args(["-map", "0:v:0", "-an", "-sn", "-dn", "-frames:v", "1", "-filter_threads", "1", "-vf", "scale=160:90:force_original_aspect_ratio=increase:flags=neighbor,crop=160:90,format=rgb24", "-threads", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"]);
        let frame = run(decode, Duration::from_secs(2), 160 * 90 * 3)?;
        if frame.len() != 160 * 90 * 3 {
            bail!("video sample frame unavailable at {timestamp:.6}s")
        }
        for y in 0..8 {
            for x in 0..8 {
                let offset = (((2 * y + 1) * 90 / 16) * 160 + (2 * x + 1) * 160 / 16) * 3;
                colors.push(Color([
                    frame[offset] as f64 / 255.,
                    frame[offset + 1] as f64 / 255.,
                    frame[offset + 2] as f64 / 255.,
                ]));
            }
        }
    }
    Ok(Samples {
        colors,
        evidence: json!({"model":MODEL,"durationSeconds":duration,"seekTimesSeconds":timestamps,"frames":timestamps.len(),"samplesPerFrame":64,"width":width,"height":height,"pixelFormat":pixel_format,"colorMetadata":stream,"contentSha256":video.hash,"assumption":"Decoded 8-bit SDR RGB channels are modeled as encoded sRGB; temporal samples are representative, not an exhaustive frame minimum."}),
    })
}
