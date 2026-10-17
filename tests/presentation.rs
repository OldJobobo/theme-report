use std::{fs, path::Path, process::Command};
#[test]
fn richer_visual_evidence_preserves_the_v4_grading_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
    fs::create_dir_all(&root).unwrap();
    let t = tempfile::tempdir_in(root).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            "fixtures/themes/minimal",
            "--profile",
            theme_report::inputs::PROFILE,
            "--cache-dir",
            t.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let baseline: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/reports/grading-v4.json")).unwrap();
    for key in ["score", "grade", "state", "capReasons", "categories"] {
        assert_eq!(r[key], baseline[key], "{key}");
    }
    let findings = r["findings"].as_array().unwrap();
    for (actual, expected) in findings
        .iter()
        .zip(baseline["findings"].as_array().unwrap())
    {
        for key in [
            "ruleId",
            "budget",
            "assessedBudget",
            "pointsLost",
            "assessmentStatus",
            "severity",
            "policySuppressed",
        ] {
            assert_eq!(actual[key], expected[key], "{}: {key}", actual["ruleId"]);
        }
    }
    for f in findings
        .iter()
        .filter(|f| f["assessmentStatus"] == "assessed")
    {
        let e = &f["evidence"];
        if e.get("contrast").is_some() {
            for key in ["foreground", "effectiveBackground"] {
                assert!(
                    theme_report::color::Color::parse(e[key].as_str().unwrap()).is_ok(),
                    "{}: {key}",
                    f["ruleId"]
                );
            }
        }
        if e.get("oklabDistance").is_some() {
            for key in ["normalColor", "stateColor"] {
                assert!(theme_report::color::Color::parse(e[key].as_str().unwrap()).is_ok());
            }
        }
    }
    let media = findings
        .iter()
        .find(|f| f["ruleId"] == "validity.backgrounds")
        .unwrap();
    for thumbnail in media["evidence"]["thumbnails"]
        .as_object()
        .unwrap()
        .values()
    {
        let colors = thumbnail["sampleColors"].as_array().unwrap();
        assert_eq!(colors.len(), 64);
        for c in colors {
            assert!(theme_report::color::Color::parse(c.as_str().unwrap()).is_ok());
        }
    }
    let roles = findings
        .iter()
        .find(|f| f["ruleId"] == "validity.roles")
        .unwrap();
    let palette = roles["evidence"]["paletteColors"].as_object().unwrap();
    assert_eq!(palette["background"], "#101315");
    assert_eq!(palette["red"], "#ff7777");
    for color in palette.values() {
        assert!(theme_report::color::Color::parse(color.as_str().unwrap()).is_ok());
    }
    assert_eq!(
        roles["evidence"]["resolvedColors"]
            .as_object()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn overview_palette_contains_all_authored_ansi_colors() {
    let work = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/palette");
    fs::create_dir_all(&work).unwrap();
    let t = tempfile::tempdir_in(work).unwrap();
    let theme = t.path().join("theme");
    fs::create_dir_all(&theme).unwrap();
    let mut colors = fs::read_to_string("fixtures/themes/minimal/colors.toml").unwrap();
    for i in 0..16 {
        colors.push_str(&format!("color{i} = '#{:06X}'\n", i * 0x10101));
    }
    colors.push_str("unsupported_color = 'not-a-color'\n");
    fs::write(theme.join("colors.toml"), colors).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            theme.to_str().unwrap(),
            "--profile",
            theme_report::inputs::PROFILE,
            "--format",
            "json",
            "--cache-dir",
            t.path().join("cache").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(out.status.success());
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let colors = &report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["ruleId"] == "validity.roles")
        .unwrap()["evidence"]["paletteColors"];
    for i in 0..16 {
        assert_eq!(colors[format!("color{i}")], format!("#{:06x}", i * 0x10101));
    }
    assert!(colors.get("unsupported_color").is_none());
}
