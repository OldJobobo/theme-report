use crate::color::Color;
use anyhow::{Context, Result, bail};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub struct Samples {
    pub colors: Vec<Color>,
    pub evidence: serde_json::Value,
}
const SRGB_PROFILE: &[u8] = include_bytes!("../profiles/srgb.icc");
pub fn thumbnail_tool() -> &'static str {
    for name in ["magick", "convert"] {
        if std::env::var_os("PATH")
            .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
        {
            return name;
        }
    }
    "magick"
}
pub fn backend_identity() -> String {
    let mut cmd = Command::new(thumbnail_tool());
    cmd.arg("-version");
    let identity = crate::video::run(cmd, Duration::from_secs(2), 16384)
        .map(|v| String::from_utf8_lossy(&v).into_owned())
        .unwrap_or_else(|_| "magick: unavailable".into());
    format!(
        "thumbnail-model:v4-preserved-alpha\n{identity}\nsrgb-profile:{}",
        crate::inputs::hash(SRGB_PROFILE)
    )
}
pub fn sample(bytes: &[u8], cache: &Path) -> Result<Samples> {
    fs::create_dir_all(cache)?;
    let mut file = tempfile::NamedTempFile::new_in(cache)?;
    file.write_all(bytes)?;
    file.flush()?;
    sample_path(file.path(), cache)
}
// Large media stays in a disk snapshot; only the 160×90 thumbnail and 64 samples leave decoding.
pub fn sample_path(path: &Path, cache: &Path) -> Result<Samples> {
    // Avoid full-resolution JPEG allocation for oversized encoded sources when reduction is available.
    let mut header = [0u8; 2];
    fs::File::open(path)?
        .read_exact(&mut header)
        .context("invalid-image: truncated image header")?;
    let has_tool = std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|dir| dir.join(thumbnail_tool()).is_file()));
    if header == [0xff, 0xd8] && fs::metadata(path)?.len() > crate::inputs::IMAGE_LIMIT && has_tool
    {
        return managed_thumbnail(
            path,
            cache,
            "large encoded JPEG; decoder-assisted thumbnail",
        );
    }
    match sample_native(path) {
        Ok(colors) => Ok(Samples {
            colors,
            evidence: serde_json::json!({"backend":"pinned-rust-image","thumbnailWidth":160,"thumbnailHeight":90,"gridSamples":64}),
        }),
        Err(e)
            if e.to_string()
                .starts_with("unsupported-image: embedded ICC profile")
                || e.to_string().starts_with("image resource limit:")
                || e.to_string() == "image pixel limit" =>
        {
            managed_thumbnail(path, cache, &e.to_string())
        }
        Err(e) => Err(e),
    }
}
fn managed_thumbnail(path: &Path, cache: &Path, reason: &str) -> Result<Samples> {
    let mut header = [0u8; 16];
    let count = fs::File::open(path)?.read(&mut header)?;
    let coder = match image::guess_format(&header[..count])? {
        image::ImageFormat::Jpeg => "JPEG",
        image::ImageFormat::Png => "PNG",
        _ => bail!("unsupported-image: managed thumbnails support PNG/JPEG only"),
    };
    let mut profile = tempfile::Builder::new().suffix(".icc").tempfile_in(cache)?;
    profile.write_all(SRGB_PROFILE)?;
    profile.flush()?;
    let mut command = Command::new(thumbnail_tool());
    command
        .env("MAGICK_OCL_DEVICE", "OFF")
        .env("MAGICK_THREAD_LIMIT", "1")
        .env("OMP_NUM_THREADS", "1")
        .args([
            "-limit",
            "memory",
            "256MiB",
            "-limit",
            "map",
            "0",
            "-limit",
            "disk",
            "0",
            "-limit",
            "width",
            "16384",
            "-limit",
            "height",
            "16384",
            "-limit",
            "area",
            "64MP",
            "-define",
            "jpeg:size=160x90",
        ])
        .arg(format!("{coder}:{}[0]", path.display()))
        .args(["-auto-orient", "-profile"])
        .arg(profile.path())
        .args([
            "-filter",
            "Point",
            "-resize",
            "160x90^",
            "-gravity",
            "center",
            "-background",
            "none",
            "-alpha",
            "on",
            "-extent",
            "160x90",
            "-depth",
            "8",
            "RGBA:-",
        ]);
    let raw=crate::video::run(command,Duration::from_secs(4),160*90*4).map_err(|e|anyhow::anyhow!("managed-image thumbnail unavailable or resource limited (requires ImageMagick with LCMS): {e}"))?;
    if raw.len() != 160 * 90 * 4 {
        bail!("managed-image thumbnail returned invalid RGB size");
    }
    let mut colors = Vec::with_capacity(64);
    for y in 0..8 {
        for x in 0..8 {
            let i = (((2 * y + 1) * 90 / 16) * 160 + (2 * x + 1) * 160 / 16) * 4;
            if raw[i + 3] != 255 {
                bail!("unsupported-image: sampled image transparency");
            }
            colors.push(Color([
                raw[i] as f64 / 255.,
                raw[i + 1] as f64 / 255.,
                raw[i + 2] as f64 / 255.,
            ]));
        }
    }
    Ok(Samples {
        colors,
        evidence: serde_json::json!({"backend":"imagemagick-lcms-thumbnail-v4","inputReason":reason,"colorManagement":"embedded ICC converted to canonical sRGB when present; untagged decoded channels assumed sRGB","thumbnailWidth":160,"thumbnailHeight":90,"gridSamples":64,"jpegDecode":"decoder-assisted reduction when available","scope":"representative color-managed thumbnail, not an exhaustive source pixel minimum"}),
    })
}
fn sample_native(path: &Path) -> Result<Vec<Color>> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg("decode-image")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        if let Some(status) = child.try_wait()? {
            let mut raw = String::new();
            child
                .stdout
                .take()
                .context("Missing decoder output")?
                .take(32768)
                .read_to_string(&mut raw)?;
            if !status.success() {
                bail!("decoder resource limit or crash")
            }
            let output: serde_json::Value = serde_json::from_str(&raw)?;
            if let Some(reason) = output.get("error").and_then(|v| v.as_str()) {
                bail!("{reason}")
            }
            let colors: Vec<Color> = serde_json::from_value(output["samples"].clone())?;
            if colors.len() != 64 {
                bail!("invalid decoder response")
            };
            return Ok(colors);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("decoder deadline exceeded")
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn image_error(error: image::ImageError) -> anyhow::Error {
    match error {
        image::ImageError::Limits(_) => anyhow::anyhow!("image resource limit: {error}"),
        image::ImageError::Unsupported(_) => anyhow::anyhow!("unsupported-image: {error}"),
        _ => anyhow::anyhow!("invalid-image: {error}"),
    }
}

pub fn decode(path: &Path) -> Result<Vec<Color>> {
    // Child-only resource limits: the parent and UI are never constrained.
    #[cfg(target_os = "linux")]
    unsafe {
        let memory = libc::rlimit {
            rlim_cur: 512 * 1024 * 1024,
            rlim_max: 512 * 1024 * 1024,
        };
        let cpu = libc::rlimit {
            rlim_cur: 4,
            rlim_max: 4,
        };
        if libc::setrlimit(libc::RLIMIT_AS, &memory) != 0
            || libc::setrlimit(libc::RLIMIT_CPU, &cpu) != 0
        {
            bail!("cannot establish decoder resource limits")
        }
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW);
    }
    let input = options.open(path)?;
    if !input.metadata()?.is_file() {
        bail!("unsupported-image: not a regular file")
    }
    let mut bytes = Vec::new();
    input
        .take(crate::inputs::IMAGE_DISK_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > crate::inputs::IMAGE_DISK_LIMIT as usize {
        bail!("image byte limit")
    }
    let format = image::guess_format(&bytes).context("unsupported-image: unknown format")?;
    if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        bail!("unsupported-image: static PNG/JPEG only")
    }
    if format == image::ImageFormat::Png {
        let mut position = 8usize;
        let mut profiled = false;
        let mut unsupported_color = None;
        while position + 12 <= bytes.len() {
            let length =
                u32::from_be_bytes(bytes[position..position + 4].try_into().unwrap()) as usize;
            let end = position
                .checked_add(12)
                .and_then(|n| n.checked_add(length))
                .context("invalid-image: PNG chunk overflow")?;
            if end > bytes.len() {
                bail!("invalid-image: truncated PNG chunk")
            }
            let kind = &bytes[position + 4..position + 8];
            if kind == b"acTL" {
                bail!("unsupported-image: animated PNG")
            }
            if kind == b"gAMA"
                && (length != 4
                    || u32::from_be_bytes(bytes[position + 8..position + 12].try_into().unwrap())
                        != 45455)
            {
                unsupported_color = Some("unsupported-image: non-sRGB PNG gamma");
            }
            if kind == b"cHRM" {
                unsupported_color =
                    Some("unsupported-image: explicit PNG chromaticity requires color management");
            }
            if kind == b"iCCP" {
                profiled = true;
            }
            position = end;
            if kind == b"IEND" {
                break;
            }
        }
        if profiled {
            bail!("unsupported-image: embedded ICC profile");
        }
        if let Some(reason) = unsupported_color {
            bail!("{reason}");
        }
    }
    let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(image_error)?;
    use image::ImageDecoder;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > 32_000_000 {
        bail!("image pixel limit")
    }
    if decoder.icc_profile()?.is_some() {
        bail!("unsupported-image: embedded ICC profile")
    }
    let orientation = decoder.orientation()?;
    let mut img = image::DynamicImage::from_decoder(decoder).map_err(image_error)?;
    img.apply_orientation(orientation);
    let img = img
        .resize_to_fill(160, 90, image::imageops::FilterType::Nearest)
        .to_rgba8();
    let mut samples = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            let p = img
                .get_pixel((2 * x + 1) * 160 / 16, (2 * y + 1) * 90 / 16)
                .0;
            if p[3] != 255 {
                bail!("unsupported-image: sampled image transparency")
            }
            samples.push(Color([
                p[0] as f64 / 255.,
                p[1] as f64 / 255.,
                p[2] as f64 / 255.,
            ]));
        }
    }
    Ok(samples)
}
