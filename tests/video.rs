use serde_json::Value;
use std::{fs, path::Path, process::Command};
use theme_report::inputs::{Config, PROFILE, Snapshot};
fn work() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/video-tests");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}
fn config(root: &Path) -> Config {
    let theme = root.join("theme");
    fs::create_dir_all(theme.join("backgrounds")).unwrap();
    fs::copy(
        "fixtures/themes/minimal/colors.toml",
        theme.join("colors.toml"),
    )
    .unwrap();
    Config {
        theme,
        current: false,
        home: root.into(),
        omarchy: root.into(),
        cache: root.join("cache"),
        profile: Some(PROFILE.into()),
    }
}
fn movie(path: &Path, hdr: bool) {
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=160x90:d=0.4:r=10",
            "-f",
            "lavfi",
            "-i",
            "color=c=white:s=160x90:d=0.6:r=10",
            "-filter_complex",
            "[0:v][1:v]concat=n=2:v=1:a=0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-color_primaries",
            if hdr { "bt2020" } else { "bt709" },
            "-color_trc",
            if hdr { "smpte2084" } else { "bt709" },
            "-colorspace",
            if hdr { "bt2020nc" } else { "bt709" },
            "-y",
        ])
        .arg(path);
    assert!(
        command
            .status()
            .expect("Video tests require ffmpeg")
            .success()
    );
}
fn report(config: &Config, missing_backend: bool) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_theme-report"));
    command.args([
        "audit",
        config.theme.to_str().unwrap(),
        "--profile",
        PROFILE,
        "--format",
        "json",
        "--cache-dir",
        config.cache.to_str().unwrap(),
    ]);
    if missing_backend {
        command.env("PATH", config.home.join("no-tools"));
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn finding<'a>(r: &'a Value, id: &str) -> &'a Value {
    r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == id)
        .unwrap()
}
#[test]
fn video_samples_include_later_bright_frames_and_are_deterministic() {
    let t = work();
    let c = config(t.path());
    movie(&c.theme.join("backgrounds/change.mp4"), false);
    let s = Snapshot::capture(&c).unwrap();
    let v = s.videos.values().next().unwrap();
    let a = theme_report::video::sample(v).unwrap();
    let b = theme_report::video::sample(v).unwrap();
    assert_eq!(a.evidence, b.evidence);
    assert_eq!(a.colors.len(), 320);
    assert!(a.colors[..64].iter().all(|c| c.0[0] < 0.05));
    assert!(a.colors[128..].iter().all(|c| c.0[0] > 0.95));
    assert_eq!(a.evidence["frames"], 5);
}
#[test]
fn video_wallpaper_affects_contrast_not_just_asset_presence() {
    let t = work();
    let c = config(t.path());
    movie(&c.theme.join("backgrounds/change.mp4"), false);
    fs::write(
        c.theme.join("shell.toml"),
        "[lock]\nbackground-alpha = 0.0\n",
    )
    .unwrap();
    let r = report(&c, false);
    assert_eq!(finding(&r, "validity.backgrounds")["severity"], "pass");
    assert_eq!(finding(&r, "readability.lock")["severity"], "failure");
    assert_eq!(finding(&r, "readability.lock")["pointsLost"], 2.0);
    assert_eq!(finding(&r, "wallpaper.lock-model")["pointsLost"], 0.0);
    assert_eq!(
        finding(&r, "wallpaper.lock-model")["evidence"]["referenceDefectOwnedByReadability"],
        true
    );
    assert!(
        finding(&r, "wallpaper.lock-model")["evidence"]["contrast"]
            .as_f64()
            .unwrap()
            < 1.5
    );
}
#[test]
fn missing_decoder_and_hdr_are_unassessed_not_theme_failures() {
    let t = work();
    let c = config(t.path());
    movie(&c.theme.join("backgrounds/hdr.mp4"), true);
    for missing in [false, true] {
        let r = report(&c, missing);
        let f = finding(&r, "validity.backgrounds");
        assert_eq!(f["severity"], "unassessed");
        assert_eq!(f["pointsLost"], 0.0);
        assert!(finding(&r, "wallpaper.lock-model")["assessedBudget"] == 0.0);
    }
}
#[test]
fn malformed_video_failure_is_owned_only_by_validity() {
    let t = work();
    let c = config(t.path());
    fs::write(c.theme.join("backgrounds/broken.mp4"), "not a movie").unwrap();
    let r = report(&c, false);
    assert_eq!(finding(&r, "validity.backgrounds")["severity"], "failure");
    assert_eq!(finding(&r, "wallpaper.lock-model")["pointsLost"], 0.0);
}
#[test]
fn immutable_video_snapshot_survives_source_edit_and_fingerprint_changes() {
    let t = work();
    let c = config(t.path());
    let p = c.theme.join("backgrounds/change.mp4");
    movie(&p, false);
    let first = Snapshot::capture(&c).unwrap();
    let video = first.videos.values().next().unwrap();
    fs::write(p, "new content").unwrap();
    assert!(theme_report::video::sample(video).is_ok());
    assert!(!first.stable().unwrap());
    let second = Snapshot::capture(&c).unwrap();
    assert_ne!(first.fingerprint, second.fingerprint);
}
#[test]
fn stalled_frame_decoder_is_bounded_and_unassessed() {
    use std::os::unix::fs::PermissionsExt;
    let t = work();
    let c = config(t.path());
    movie(&c.theme.join("backgrounds/change.mp4"), false);
    let bin = t.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::write(bin.join("ffprobe"), "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo test-probe; else printf '%s' '{\"streams\":[{\"width\":160,\"height\":90,\"pix_fmt\":\"rgb24\",\"avg_frame_rate\":\"10/1\",\"duration\":\"1.0\"}]}'; fi\n").unwrap();
    fs::write(bin.join("ffmpeg"), "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then echo test-decoder; else exec /usr/bin/sleep 20; fi\n").unwrap();
    for name in ["ffprobe", "ffmpeg"] {
        fs::set_permissions(bin.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
    let start = std::time::Instant::now();
    let output = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .env("PATH", bin)
        .args([
            "audit",
            c.theme.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--format",
            "json",
            "--cache-dir",
            c.cache.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(start.elapsed().as_secs() < 6);
    let r: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        finding(&r, "validity.backgrounds")["severity"],
        "unassessed"
    );
}

#[test]
fn video_symlinks_cannot_escape_theme_root() {
    let t = work();
    let c = config(t.path());
    let p = t.path().join("outside.mp4");
    movie(&p, false);
    std::os::unix::fs::symlink(&p, c.theme.join("backgrounds/escape.mp4")).unwrap();
    // Never read; reported as an unsampled wallpaper instead of aborting the audit.
    let s = Snapshot::capture(&c).unwrap();
    assert!(!s.videos.contains_key("backgrounds/escape.mp4"));
    assert!(s.unsupported_backgrounds["backgrounds/escape.mp4"].contains("outside the theme"));
}
