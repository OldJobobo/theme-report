//! Real-world theme layouts that must degrade to limitations, never abort an audit.
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use theme_report::{
    contract::Contract,
    inputs::{BACKGROUND_LIMIT, Config, PROFILE, Snapshot},
    rules,
};

fn tmp() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/edge-cases");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}
fn fixture(root: &Path) -> PathBuf {
    let p = root.join("theme");
    fs::create_dir_all(p.join("backgrounds")).unwrap();
    fs::copy("fixtures/themes/minimal/colors.toml", p.join("colors.toml")).unwrap();
    fs::copy(
        "fixtures/themes/minimal/backgrounds/night.png",
        p.join("backgrounds/night.png"),
    )
    .unwrap();
    p
}
fn config(root: &Path, theme: PathBuf) -> Config {
    Config {
        theme,
        current: false,
        home: root.into(),
        omarchy: root.into(),
        cache: root.join("cache"),
        profile: Some(PROFILE.into()),
    }
}
fn audit(path: &Path, cache: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args(["audit", path.to_str().unwrap(), "--profile", PROFILE])
        .args(["--format", "json", "--cache-dir", cache.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
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
fn folders_notes_and_dotfiles_in_backgrounds_are_ignored() {
    let t = tmp();
    let p = fixture(t.path());
    // Stock Omarchy themes ship backgrounds/intros/ with intro videos.
    fs::create_dir(p.join("backgrounds/intros")).unwrap();
    fs::write(p.join("backgrounds/intros/clip.mp4"), "not read").unwrap();
    fs::write(p.join("backgrounds/README.md"), "credits").unwrap();
    for i in 0..40 {
        fs::write(p.join(format!("backgrounds/.cache{i}")), "").unwrap();
    }
    let r = audit(&p, &t.path().join("cache"));
    assert_eq!(r["grade"], "A");
    assert_eq!(r["state"], "ready");
    let f = finding(&r, "validity.backgrounds");
    assert_eq!(f["severity"], "pass");
    assert_eq!(
        f["evidence"]["ignored"],
        serde_json::json!(["backgrounds/README.md", "backgrounds/intros/"])
    );
}

#[test]
fn large_background_sets_sample_a_bounded_prefix() {
    let t = tmp();
    let p = fixture(t.path());
    for i in 0..BACKGROUND_LIMIT {
        fs::copy(
            "fixtures/themes/minimal/backgrounds/night.png",
            p.join(format!("backgrounds/w{i:02}.png")),
        )
        .unwrap();
    }
    let s = Snapshot::capture(&config(t.path(), p)).unwrap();
    // Sorted names: night.png, w00..w30 are sampled; w31 is the one left out.
    assert_eq!(s.unsupported_backgrounds.len(), 1);
    assert!(s.unsupported_backgrounds["backgrounds/w31.png"].contains("more than 32"));
    let r = rules::audit(&s);
    assert_eq!(r.state, "provisional");
    assert!(r.grade.is_some());
}

#[test]
fn escaping_image_symlink_is_a_limitation_not_an_abort() {
    use std::os::unix::fs::symlink;
    let t = tmp();
    let p = fixture(t.path());
    fs::copy(
        "fixtures/themes/minimal/backgrounds/night.png",
        t.path().join("outside.png"),
    )
    .unwrap();
    symlink(t.path().join("outside.png"), p.join("backgrounds/link.png")).unwrap();
    let s = Snapshot::capture(&config(t.path(), p)).unwrap();
    assert!(!s.files.contains_key("backgrounds/link.png"));
    assert!(s.unsupported_backgrounds["backgrounds/link.png"].contains("outside the theme"));
}

#[test]
fn webp_wallpapers_are_sampled() {
    let t = tmp();
    let p = fixture(t.path());
    fs::remove_file(p.join("backgrounds/night.png")).unwrap();
    let img = image::RgbImage::from_pixel(320, 180, image::Rgb([20, 30, 40]));
    img.save(p.join("backgrounds/night.webp")).unwrap();
    let r = audit(&p, &t.path().join("cache"));
    let f = finding(&r, "validity.backgrounds");
    assert_eq!(f["severity"], "pass", "{f}");
    let samples = &f["evidence"]["thumbnails"]["backgrounds/night.webp"]["sampleColors"];
    assert_eq!(samples.as_array().unwrap().len(), 64);
    assert_eq!(samples[0], "#141e28");
}

#[test]
fn passing_findings_have_no_fix_advice_or_repeated_sources() {
    let t = tmp();
    let p = fixture(t.path());
    // Translucent surfaces exercise the modeled paths that used to overwrite advice.
    fs::write(
        p.join("shell.toml"),
        "[tooltip]\nbackground-alpha = 0.97\n[launcher]\nbackground-alpha = 0.95\n",
    )
    .unwrap();
    let r = audit(&p, &t.path().join("cache"));
    for f in r["findings"].as_array().unwrap() {
        if f["severity"] == "pass" {
            assert_eq!(f["remediation"], "No change needed.", "{}", f["ruleId"]);
        }
        let sources = f["sources"].as_array().unwrap();
        let unique: std::collections::BTreeSet<_> = sources.iter().map(|s| s.to_string()).collect();
        assert_eq!(unique.len(), sources.len(), "{}", f["ruleId"]);
    }
}

#[test]
fn absent_theme_shell_toml_is_not_cited() {
    let t = tmp();
    let p = fixture(t.path());
    let r = audit(&p, &t.path().join("cache"));
    for f in r["findings"].as_array().unwrap() {
        for s in f["sources"].as_array().unwrap() {
            assert!(
                !(s["file"] == "shell.toml" && s["key"] == ""),
                "{} cites a missing shell.toml",
                f["ruleId"]
            );
        }
    }
}

#[test]
fn user_override_errors_are_advisory_not_theme_failures() {
    let t = tmp();
    let p = fixture(t.path());
    let mut s = Snapshot::capture(&config(t.path(), p)).unwrap();
    s.config.current = true;
    s.files.insert(
        "user.shell.toml".into(),
        b"[popups]\nbackground-alpha = 7\n".to_vec(),
    );
    let r = rules::audit(&s);
    let shell = r
        .findings
        .iter()
        .find(|f| f.rule_id == "validity.shell")
        .unwrap();
    assert_eq!(shell.severity, "pass");
    let advisory = r
        .findings
        .iter()
        .find(|f| f.rule_id == "advisory.user-shell")
        .unwrap();
    assert!(!advisory.scored);
    assert_eq!(
        advisory.evidence["invalidKeys"],
        serde_json::json!(["popups.background-alpha"])
    );
    assert!(r.cap_reasons.is_empty());
}

#[test]
fn unreviewed_runtime_behavior_pauses_only_dependent_checks() {
    let t = tmp();
    let p = fixture(t.path());
    let mut s = Snapshot::capture(&config(t.path(), p)).unwrap();
    s.config.current = true;
    s.contract = Contract {
        status: "partial",
        palette: true,
        selection: true,
        shell: false,
        unverified: vec!["Style.qml control states".into()],
    };
    let r = rules::audit(&s);
    for f in r.findings.iter().filter(|f| f.scored) {
        let paused = f.summary.starts_with("Paused");
        assert_eq!(
            paused,
            theme_report::contract::group(&f.rule_id) == "shell",
            "{}",
            f.rule_id
        );
        if paused {
            assert_eq!(f.assessed_budget, 0.);
            assert!(f.evidence["reason"].as_str().unwrap().contains("Style.qml"));
        }
    }
    assert_eq!(r.state, "provisional");
    assert_eq!(r.runtime_contract.as_ref().unwrap()["status"], "partial");
    assert!(
        r.categories
            .iter()
            .any(|c| c.id == "validity" && c.grade.is_some())
    );
}

#[test]
fn stale_scratch_from_killed_audits_is_swept() {
    let t = tmp();
    let cache = t.path().join("cache");
    // PID 0x7fff_fff0 is never a live process; a fresh .tmp may belong to a running publish.
    let dead = cache.join(format!("run-{}", 0x7fff_fff0));
    fs::create_dir_all(&dead).unwrap();
    fs::write(dead.join(".tmpvideo"), "x").unwrap();
    fs::write(cache.join(".tmpfresh"), "x").unwrap();
    theme_report::cache::sweep(&cache);
    assert!(!dead.exists());
    assert!(cache.join(".tmpfresh").exists());
}

#[test]
fn metadata_fast_path_reuses_cache_until_inputs_change() {
    let t = tmp();
    let p = t.path().join(".local/state/omarchy/current/theme");
    fs::create_dir_all(p.join("backgrounds")).unwrap();
    fs::copy("fixtures/themes/minimal/colors.toml", p.join("colors.toml")).unwrap();
    let run = || {
        let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
            .env("HOME", t.path())
            .args(["audit", "--current", "--format", "json"])
            .args(["--omarchy-root", t.path().to_str().unwrap()])
            .args(["--cache-dir", t.path().join("cache").to_str().unwrap()])
            .output()
            .unwrap();
        assert!(out.status.success());
        serde_json::from_slice::<Value>(&out.stdout).unwrap()
    };
    let first = run();
    assert!(t.path().join("cache/current.manifest").exists());
    let second = run();
    assert_eq!(first["auditTimestamp"], second["auditTimestamp"]);
    std::thread::sleep(std::time::Duration::from_millis(20));
    fs::write(p.join("colors.toml"), "background = \"#000000\"\n").unwrap();
    let third = run();
    assert_ne!(first["inputFingerprint"], third["inputFingerprint"]);
    // No staging directory survives a completed audit.
    assert!(
        fs::read_dir(t.path().join("cache"))
            .unwrap()
            .flatten()
            .all(|e| !e.file_name().to_string_lossy().starts_with("run-"))
    );
}
