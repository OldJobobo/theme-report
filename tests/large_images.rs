use std::{
    fs,
    io::{Seek, Write},
    path::Path,
    process::Command,
};
use theme_report::inputs::{Config, IMAGE_LIMIT, PROFILE, Snapshot};
#[test]
fn large_images_use_immutable_disk_copies_and_tiny_thumbnails() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
    fs::create_dir_all(&root).unwrap();
    for extension in ["png", "jpg"] {
        let t = tempfile::tempdir_in(&root).unwrap();
        let theme = t.path().join("theme");
        fs::create_dir_all(theme.join("backgrounds")).unwrap();
        fs::copy(
            "fixtures/themes/minimal/colors.toml",
            theme.join("colors.toml"),
        )
        .unwrap();
        let path = theme.join(format!("backgrounds/white.{extension}"));
        image::RgbImage::from_pixel(16, 16, image::Rgb([255, 255, 255]))
            .save(&path)
            .unwrap();
        // Legal trailing padding makes the encoded asset exceed the memory threshold without a giant fixture.
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.set_len(IMAGE_LIMIT + 1024).unwrap();
        file.seek(std::io::SeekFrom::End(0)).unwrap();
        file.write_all(&[0]).unwrap();
        let c = Config {
            theme: theme.clone(),
            current: false,
            home: t.path().into(),
            omarchy: t.path().into(),
            cache: t.path().join("cache"),
            profile: Some(PROFILE.into()),
        };
        let s = Snapshot::capture(&c).unwrap();
        assert_eq!(s.large_images.len(), 1);
        assert!(s.files.keys().all(|k| !k.starts_with("backgrounds/")));
        assert!(s.unsupported_backgrounds.is_empty());
        let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
            .args([
                "audit",
                theme.to_str().unwrap(),
                "--profile",
                PROFILE,
                "--cache-dir",
                c.cache.to_str().unwrap(),
                "--format",
                "json",
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        let f = r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["ruleId"] == "validity.backgrounds")
            .unwrap();
        assert_eq!(f["severity"], "pass");
        let thumbnail = f["evidence"]["thumbnails"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(thumbnail["thumbnailWidth"], 160);
        assert_eq!(thumbnail["gridSamples"], 64);
        fs::write(&path, "changed source").unwrap();
        assert!(!s.stable().unwrap());
        let snapshot = s.large_images.values().next().unwrap();
        let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
            .arg("decode-image")
            .arg(snapshot.file.path())
            .output()
            .unwrap();
        assert!(out.status.success());
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(v["samples"].as_array().unwrap().len(), 64);
        assert!(out.stdout.len() < 4096);
        assert_ne!(s.fingerprint, Snapshot::capture(&c).unwrap().fingerprint);
    }
}
