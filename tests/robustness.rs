use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};
use theme_report::inputs::PROFILE;

fn tmp() -> tempfile::TempDir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/robustness");
    fs::create_dir_all(&p).unwrap();
    tempfile::tempdir_in(p).unwrap()
}
fn minimal(root: &Path) -> std::path::PathBuf {
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
fn path_report(p: &Path, cache: &Path) -> Value {
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            p.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--cache-dir",
            cache.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).unwrap()
}
fn current_command(home: &Path, cache: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_theme-report"));
    cmd.env("HOME", home).args([
        "audit",
        "--current",
        "--omarchy-root",
        home.to_str().unwrap(),
        "--cache-dir",
        cache.to_str().unwrap(),
        "--format",
        "json",
    ]);
    cmd
}
fn current_home(root: &Path) {
    let p = root.join(".local/state/omarchy/current/theme");
    fs::create_dir_all(&p).unwrap();
    fs::copy("fixtures/themes/minimal/colors.toml", p.join("colors.toml")).unwrap();
    fs::write(p.parent().unwrap().join("theme.name"), "same-slug").unwrap();
}
#[test]
fn runtime_root_uses_session_environment_with_explicit_override() {
    let t = tmp();
    current_home(t.path());
    let selected = t.path().join("selected-runtime");
    let explicit = t.path().join("explicit-runtime");
    for root in [&selected, &explicit] {
        fs::create_dir_all(root.join("shell/Commons")).unwrap();
        fs::write(root.join("shell/Commons/Color.qml"), "legacy").unwrap();
    }
    fs::write(selected.join("shell/Commons/ShellColor.qml"), "renamed").unwrap();
    for override_root in [None, Some(&explicit)] {
        let expected = override_root
            .unwrap_or(&selected)
            .join(if override_root.is_some() {
                "shell/Commons/Color.qml"
            } else {
                "shell/Commons/ShellColor.qml"
            });
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_theme-report"));
        cmd.env("HOME", t.path())
            .env("OMARCHY_PATH", &selected)
            .args([
                "audit",
                "--current",
                "--format",
                "json",
                "--cache-dir",
                t.path().join("cache").to_str().unwrap(),
            ]);
        if let Some(root) = override_root {
            cmd.arg("--omarchy-root").arg(root);
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success());
        let r: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(
            r["state"], "not-graded",
            "Unknown runtime source must still be rejected"
        );
        let paths = r["watchedPaths"].as_array().unwrap();
        assert!(paths.iter().any(|p| p.as_str() == expected.to_str()));
        if override_root.is_none() {
            assert!(
                !paths
                    .iter()
                    .any(|p| p.as_str().is_some_and(|p| p.ends_with("/Color.qml")))
            );
        }
    }
}
#[test]
fn concurrent_current_writers_publish_complete_json() {
    let t = tmp();
    current_home(t.path());
    let cache = t.path().join("cache");
    let mut children = Vec::new();
    for _ in 0..8 {
        children.push(
            current_command(t.path(), &cache)
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        );
    }
    let mut fingerprints = Vec::new();
    for child in children {
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let r: Value = serde_json::from_slice(&out.stdout).unwrap();
        fingerprints.push(r["inputFingerprint"].clone());
    }
    assert!(fingerprints.iter().all(|f| f == &fingerprints[0]));
    let r: Value = serde_json::from_slice(&fs::read(cache.join("current.json")).unwrap()).unwrap();
    assert_eq!(r["inputFingerprint"], fingerprints[0]);
}
#[test]
fn malformed_cache_is_replaced_without_using_its_grade() {
    let t = tmp();
    current_home(t.path());
    let cache = t.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("current.json"), "{\"grade\":\"A\"").unwrap();
    let out = current_command(t.path(), &cache).output().unwrap();
    assert!(out.status.success());
    let r: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(r["grade"].is_null());
    assert_eq!(r["state"], "not-graded");
    let mut cached: Value =
        serde_json::from_slice(&fs::read(cache.join("current.json")).unwrap()).unwrap();
    cached["grade"] = Value::String("A".into());
    cached["score"] = serde_json::json!(100.0);
    cached["state"] = Value::String("ready".into());
    fs::write(
        cache.join("current.json"),
        serde_json::to_vec(&cached).unwrap(),
    )
    .unwrap();
    let out = current_command(t.path(), &cache).output().unwrap();
    let fresh: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(
        fresh["grade"].is_null(),
        "An inconsistent cache must not supply a fabricated grade"
    );
}
#[test]
fn same_slug_input_edit_invalidates_current_cache() {
    let t = tmp();
    current_home(t.path());
    let cache = t.path().join("cache");
    let a = current_command(t.path(), &cache).output().unwrap();
    fs::write(
        t.path()
            .join(".local/state/omarchy/current/theme/shell.toml"),
        "[bar]\nbackground-alpha = 0.5\n",
    )
    .unwrap();
    let b = current_command(t.path(), &cache).output().unwrap();
    let a: Value = serde_json::from_slice(&a.stdout).unwrap();
    let b: Value = serde_json::from_slice(&b.stdout).unwrap();
    assert_eq!(a["theme"], b["theme"]);
    assert_ne!(a["inputFingerprint"], b["inputFingerprint"]);
}
#[test]
fn future_keys_are_not_theme_failures() {
    let t = tmp();
    let p = minimal(t.path());
    fs::write(
        p.join("shell.toml"),
        "[future-section]\nbackground-alpha = 2\n[bar]\nfuture-alpha = 2\n",
    )
    .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "validity.shell")
        .unwrap();
    assert_eq!(f["severity"], "pass");
    assert!(!f["evidence"]["ignoredLines"].as_array().unwrap().is_empty());
}
#[test]
fn collapsed_fill_does_not_ignore_unknown_redundant_cues() {
    let t = tmp();
    let p = minimal(t.path());
    fs::write(
        p.join("shell.toml"),
        "[controls]\nnormal-fill-alpha = 0\nselected-fill-alpha = 0\nfocus-fill-alpha = 0\n",
    )
    .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    for id in ["shell.selected-fill", "shell.focus-fill"] {
        let f = r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["ruleId"] == id)
            .unwrap();
        assert_eq!(f["severity"], "caution");
        assert_eq!(f["pointsLost"], f["budget"].as_f64().unwrap() / 2.);
        assert!(
            f["evidence"]["scope"]
                .as_str()
                .unwrap()
                .contains("not proof")
        );
    }
}
#[test]
fn native_gradient_is_sampled_instead_of_flattened() {
    let t = tmp();
    let p = minimal(t.path());
    fs::write(
        p.join("shell.toml"),
        "[popups]\nborder = '#88bbff #ff7777 45deg'\n",
    )
    .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "shell.popup-border")
        .unwrap();
    assert_eq!(f["assessmentStatus"], "assessed");
    assert_eq!(f["evidence"]["borderSamples"], 17);
    assert!(f["evidence"]["contrast"].as_f64().unwrap() > 0.);
}
#[test]
fn native_rgba_alias_gradient_is_measured_over_translucent_popups() {
    let t = tmp();
    let p = minimal(t.path());
    fs::write(p.join("shell.toml"), "[hyprland]\nactive-border = 'rgba(D83A66ee) rgba(F05C7Acc) rgba(A9E1DAaa) rgba(F05C7Acc) 35deg'\n[popups]\nbackground = '#2A0D1A'\nbackground-alpha = 0.94\nborder = 'hyprland.active-border'\nborder-alpha = 0.82\nborder-width = 2\n").unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "shell.popup-border")
        .unwrap();
    assert_eq!(f["assessmentStatus"], "assessed");
    assert_eq!(f["ruleVersion"], 5);
    assert_eq!(f["evidence"]["borderSamples"], 51);
    assert_eq!(f["evidence"]["backgroundSamples"], 17);
    assert_eq!(f["evidence"]["backgroundModel"], "application-gray17-v4");
    assert!(f["evidence"]["contrast"].as_f64().unwrap() > 0.);
    assert_eq!(
        r["state"], "ready",
        "Supported RGBA gradients must not make the grade provisional"
    );
}
#[test]
fn transparent_native_border_is_not_silently_treated_as_opaque() {
    let t = tmp();
    let p = minimal(t.path());
    fs::write(
        p.join("shell.toml"),
        "[popups]\nborder = 'rgba(ffffff00) rgba(ffffffff) 45deg'\n",
    )
    .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "shell.popup-border")
        .unwrap();
    assert_eq!(f["severity"], "failure");
    assert_eq!(f["evidence"]["contrast"], 1.);
    assert_eq!(f["assessedBudget"], 6.);
}
#[test]
fn unsupported_border_reports_the_actual_parser_limitation() {
    let t = tmp();
    let p = minimal(t.path());
    fs::write(
        p.join("shell.toml"),
        "[popups]\nbackground-alpha = 0.94\nborder = 'not-a-color #ffffff 45deg'\n",
    )
    .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "shell.popup-border")
        .unwrap();
    assert_eq!(f["assessmentStatus"], "unassessed");
    assert!(
        f["evidence"]["reason"]
            .as_str()
            .unwrap()
            .contains("color/gradient")
    );
    assert_eq!(f["evidence"]["borderToken"], "not-a-color #ffffff 45deg");
}
#[test]
fn terminal_selection_fallbacks_resolve() {
    let t = tmp();
    let p = minimal(t.path());
    let colors = fs::read_to_string(p.join("colors.toml")).unwrap();
    fs::write(
        p.join("colors.toml"),
        colors
            .lines()
            .filter(|l| !l.starts_with("selection_"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "readability.selection")
        .unwrap();
    assert_eq!(f["severity"], "pass");
}
#[test]
fn ordinary_fifo_cannot_block_capture() {
    use std::ffi::CString;
    let t = tmp();
    let p = minimal(t.path());
    fs::remove_file(p.join("colors.toml")).unwrap();
    let path = CString::new(p.join("colors.toml").to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0);
    let start = std::time::Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            p.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(start.elapsed().as_secs() < 2);
}
#[test]
fn decoder_rejects_named_pipe_without_blocking() {
    use std::ffi::CString;
    let t = tmp();
    let path = t.path().join("image-pipe");
    let c = CString::new(path.to_str().unwrap()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let start = std::time::Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args(["decode-image", path.to_str().unwrap()])
        .output()
        .unwrap();
    let result: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(result["error"].as_str().unwrap().contains("regular file"));
    assert!(start.elapsed().as_secs() < 2);
}
#[test]
fn unsupported_and_resource_limited_images_are_not_theme_deductions() {
    for image in [
        "animated.png",
        "gamma.png",
        "transparent.png",
        "oversized.png",
    ] {
        let t = tmp();
        let p = minimal(t.path());
        fs::copy(
            Path::new("fixtures/images").join(image),
            p.join("backgrounds/night.png"),
        )
        .unwrap();
        let r = path_report(&p, &t.path().join("cache"));
        let f = r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["ruleId"] == "validity.backgrounds")
            .unwrap();
        assert_eq!(f["severity"], "unassessed", "{image}: {}", f["evidence"]);
        assert_eq!(f["pointsLost"], 0.0);
    }
}

#[test]
fn command_target_validation_is_usage_error() {
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .arg("audit")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args(["explain", "unknown-rule"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn oversized_video_is_unassessed_without_bulk_capture() {
    let t = tmp();
    let p = minimal(t.path());
    fs::File::create(p.join("backgrounds/movie.mp4"))
        .unwrap()
        .set_len(theme_report::inputs::VIDEO_LIMIT + 1)
        .unwrap();
    let r = path_report(&p, &t.path().join("cache"));
    let f = r["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "validity.backgrounds")
        .unwrap();
    assert_eq!(f["severity"], "unassessed");
    assert!(
        f["evidence"]["unsupported"]
            .to_string()
            .contains("movie.mp4")
    );
    assert_eq!(f["pointsLost"], 0.0);
    assert_eq!(r["grade"], "A");
    assert_eq!(r["state"], "provisional");
}

#[test]
fn selected_unsupported_video_is_not_a_missing_background() {
    use theme_report::{
        inputs::{Config, Snapshot},
        rules,
    };
    let t = tmp();
    let p = minimal(t.path());
    let mut s = Snapshot::capture(&Config {
        theme: p,
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().join("cache"),
        profile: Some(PROFILE.into()),
    })
    .unwrap();
    s.config.current = true;
    s.unsupported_backgrounds
        .insert("current.background".into(), "unsupported video".into());
    let r = rules::audit(&s);
    let f = r
        .findings
        .iter()
        .find(|f| f.rule_id == "validity.backgrounds")
        .unwrap();
    assert_eq!(f.severity, "unassessed");
    assert_eq!(f.points_lost, 0.0);
}

#[test]
fn human_output_never_renders_theme_control_codes() {
    let t = tmp();
    let p = minimal(t.path());
    let named = t.path().join("theme\u{1b}[31m");
    fs::rename(&p, &named).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            named.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--cache-dir",
            t.path().join("cache").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(!out.stdout.contains(&0x1b));
}

#[test]
fn oversized_image_is_a_bounded_limitation_and_fingerprinted() {
    use theme_report::inputs::{Config, Snapshot};
    let t = tmp();
    let p = minimal(t.path());
    let image = p.join("backgrounds/large.jpg");
    let file = fs::File::create(&image).unwrap();
    file.set_len(theme_report::inputs::IMAGE_DISK_LIMIT + 1)
        .unwrap();
    let c = Config {
        theme: p.clone(),
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().join("cache"),
        profile: Some(PROFILE.into()),
    };
    let s = Snapshot::capture(&c).unwrap();
    assert!(!s.files.contains_key("backgrounds/large.jpg"));
    assert!(
        s.unsupported_backgrounds["backgrounds/large.jpg"].contains("media snapshot byte limit")
    );
    let mut selected = s;
    selected.config.current = true;
    selected.unsupported_backgrounds.insert(
        "current.background".into(),
        selected.unsupported_backgrounds["backgrounds/large.jpg"].clone(),
    );
    let r = theme_report::rules::audit(&selected);
    let background = r
        .findings
        .iter()
        .find(|f| f.rule_id == "validity.backgrounds")
        .unwrap();
    assert_eq!(background.severity, "unassessed");
    assert_eq!(background.points_lost, 0.);
    let r = path_report(&p, &c.cache);
    assert_eq!(r["grade"], "A");
    assert_eq!(r["state"], "provisional");
    file.set_len(theme_report::inputs::IMAGE_DISK_LIMIT + 2)
        .unwrap();
    assert_ne!(
        selected.fingerprint,
        Snapshot::capture(&c).unwrap().fingerprint
    );
}
