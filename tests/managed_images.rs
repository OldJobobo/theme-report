use std::{fs, path::Path, process::Command};
use theme_report::{color::Color, inputs::PROFILE};
fn work() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}
fn setup(root: &Path) -> std::path::PathBuf {
    let p = root.join("theme");
    fs::create_dir_all(p.join("backgrounds")).unwrap();
    fs::copy("fixtures/themes/minimal/colors.toml", p.join("colors.toml")).unwrap();
    fs::write(
        p.join("shell.toml"),
        "[lock]\nbackground-alpha = 0\ntext = '#ffffff'\n",
    )
    .unwrap();
    p
}
fn report(p: &Path, root: &Path) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            p.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--cache-dir",
            root.join("cache").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn finding<'a>(r: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == id)
        .unwrap()
}
#[test]
fn embedded_wide_gamut_profile_is_converted_not_discarded() {
    let t = work();
    let p = setup(t.path());
    let source = t.path().join("source.png");
    image::RgbImage::from_pixel(16, 16, image::Rgb([120, 150, 100]))
        .save(&source)
        .unwrap();
    let out = Command::new(theme_report::wallpaper::thumbnail_tool())
        .arg(&source)
        .args(["-profile", "fixtures/profiles/wide-rgb.icc"])
        .arg(p.join("backgrounds/profiled.png"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let r = report(&p, t.path());
    let f = finding(&r, "validity.backgrounds");
    assert_eq!(f["severity"], "pass");
    assert_eq!(
        f["evidence"]["thumbnails"]["backgrounds/profiled.png"]["backend"],
        "imagemagick-lcms-thumbnail-v4"
    );
    // Independent LCMS reference for this D65/gamma-2.2 wide-RGB fixture: [120,150,100] -> [105,151,97].
    let expected = Color([105. / 255., 151. / 255., 97. / 255.]).contrast(Color([1.; 3]));
    let measured = finding(&r, "readability.lock")["evidence"]["contrast"]
        .as_f64()
        .unwrap();
    assert!(
        (measured - expected).abs() < 0.025,
        "{measured} != {expected}"
    );
    assert!(r["categories"].as_array().unwrap().iter().all(|c| {
        if c["weight"].as_f64().unwrap() > 0. {
            !c["grade"].is_null()
        } else {
            c["grade"].is_null()
        }
    }));
}
#[test]
fn high_resolution_jpeg_uses_decoder_reduced_thumbnail() {
    let t = work();
    let p = setup(t.path());
    image::RgbImage::from_pixel(8504, 4762, image::Rgb([120, 150, 100]))
        .save(p.join("backgrounds/large.jpg"))
        .unwrap();
    let r = report(&p, t.path());
    let f = finding(&r, "validity.backgrounds");
    assert_eq!(f["severity"], "pass");
    let e = &f["evidence"]["thumbnails"]["backgrounds/large.jpg"];
    assert_eq!(e["backend"], "imagemagick-lcms-thumbnail-v4");
    assert_eq!(e["thumbnailWidth"], 160);
    assert_eq!(e["gridSamples"], 64);
}
#[test]
fn profiled_transparency_is_not_silently_flattened() {
    let t = work();
    let p = setup(t.path());
    let source = t.path().join("transparent.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([120, 150, 100, 0]))
        .save(&source)
        .unwrap();
    let out = Command::new(theme_report::wallpaper::thumbnail_tool())
        .arg(&source)
        .args(["-profile", "fixtures/profiles/wide-rgb.icc"])
        .arg(p.join("backgrounds/profiled.png"))
        .output()
        .unwrap();
    assert!(out.status.success());
    let r = report(&p, t.path());
    assert_eq!(
        finding(&r, "validity.backgrounds")["severity"],
        "unassessed"
    );
    assert!(
        finding(&r, "validity.backgrounds")["evidence"]["unsupported"]
            .to_string()
            .contains("transparency")
    );
}
#[test]
fn missing_color_management_tool_is_a_limitation_and_changes_fingerprint() {
    let t = work();
    let p = setup(t.path());
    let source = t.path().join("source.png");
    image::RgbImage::from_pixel(16, 16, image::Rgb([120, 150, 100]))
        .save(&source)
        .unwrap();
    assert!(
        Command::new(theme_report::wallpaper::thumbnail_tool())
            .arg(&source)
            .args(["-profile", "fixtures/profiles/wide-rgb.icc"])
            .arg(p.join("backgrounds/profiled.png"))
            .status()
            .unwrap()
            .success()
    );
    let ready = report(&p, t.path());
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .env("PATH", t.path().join("no-tools"))
        .args([
            "audit",
            p.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--cache-dir",
            t.path().join("cache").to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let unavailable: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        finding(&unavailable, "validity.backgrounds")["severity"],
        "unassessed"
    );
    assert_ne!(ready["inputFingerprint"], unavailable["inputFingerprint"]);
}
#[test]
fn malformed_gradient_does_not_get_a_fabricated_border_grade() {
    let t = work();
    let p = setup(t.path());
    fs::copy(
        "fixtures/themes/minimal/backgrounds/night.png",
        p.join("backgrounds/night.png"),
    )
    .unwrap();
    fs::write(
        p.join("shell.toml"),
        "[popups]\nborder = '#88bbff not-a-color 45deg'\n",
    )
    .unwrap();
    let r = report(&p, t.path());
    assert_eq!(finding(&r, "shell.popup-border")["severity"], "unassessed");
    assert!(!r["categories"][3]["grade"].is_null());
}
