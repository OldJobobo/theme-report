use std::{fs, path::Path, process::Command};
use theme_report::{color::Color, inputs::PROFILE, model::Report};
fn audit(shell: &str, unsupported: bool) -> Report {
    audit_palette(
        shell,
        unsupported,
        &fs::read_to_string("fixtures/themes/minimal/colors.toml").unwrap(),
    )
}
fn audit_palette(shell: &str, unsupported: bool, palette: &str) -> Report {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
    fs::create_dir_all(&root).unwrap();
    let t = tempfile::tempdir_in(root).unwrap();
    let theme = t.path().join("theme");
    fs::create_dir_all(theme.join("backgrounds")).unwrap();
    fs::write(theme.join("colors.toml"), palette).unwrap();
    image::RgbImage::from_pixel(1, 1, image::Rgb([255, 255, 255]))
        .save(theme.join("backgrounds/white.png"))
        .unwrap();
    if unsupported {
        fs::write(theme.join("backgrounds/vector.svg"), "<svg/>").unwrap();
    }
    fs::write(theme.join("shell.toml"), shell).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            theme.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--format",
            "json",
            "--cache-dir",
            t.path().join("cache").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn known_lock_wallpaper_composites_without_duplicate_deductions() {
    for (alpha, severity) in [
        (0., "failure"),
        (0.5, "caution"),
        (0.8, "pass"),
        (1., "pass"),
    ] {
        let r = audit(
            &format!(
                "[lock]\nbackground = '#000000'\ntext = '#ffffff'\nbackground-alpha = {alpha}\n"
            ),
            false,
        );
        let lock = r
            .findings
            .iter()
            .find(|f| f.rule_id == "readability.lock")
            .unwrap();
        let wallpaper = r
            .findings
            .iter()
            .find(|f| f.rule_id == "wallpaper.lock-model")
            .unwrap();
        let expected = Color::parse("#ffffff").unwrap().contrast(
            Color::parse("#000000")
                .unwrap()
                .over(alpha, Color::parse("#ffffff").unwrap()),
        );
        assert!((lock.evidence["contrast"].as_f64().unwrap() - expected).abs() < 1e-10);
        assert_eq!(lock.severity, severity);
        assert!(lock.scored);
        assert_eq!(lock.assessed_budget, 2.);
        assert_eq!(wallpaper.points_lost, 0.);
        if alpha < 1. {
            assert_eq!(lock.evidence["context"], "wallpaper-backed");
        }
    }
}
#[test]
fn unknown_media_stays_limited_but_application_scenarios_are_graded() {
    let r = audit(
        "[lock]\nbackground-alpha = 0.8\n[bar]\nbackground-alpha = 0.7\n[popups]\nbackground-alpha = 0.7\n",
        true,
    );
    for id in ["readability.lock", "wallpaper.lock-model"] {
        let f = r.findings.iter().find(|f| f.rule_id == id).unwrap();
        assert_eq!(f.assessed_budget, 0., "{id}");
        assert_eq!(f.points_lost, 0., "{id}");
        assert!(f.evidence["reason"].as_str().unwrap().len() > 30);
    }
    assert_eq!(r.grade.as_deref(), Some("A"));
    assert_eq!(r.state, "provisional");
    assert_eq!(r.categories[3].state, "ready");
    for id in [
        "readability.bar",
        "shell.selected-fill",
        "shell.popup-border",
    ] {
        let f = r.findings.iter().find(|f| f.rule_id == id).unwrap();
        assert!(f.scored);
        assert_eq!(f.assessed_budget, f.budget);
        assert_eq!(f.evidence["backgroundModel"], "application-gray17-v4");
        assert_eq!(f.evidence["backgroundSamples"], 17);
    }
}
#[test]
fn category_weights_and_foundation_f_are_preserved() {
    let mut r = audit("", false);
    for f in &mut r.findings {
        f.assessment_status = "assessed".into();
        f.assessed_budget = f.budget;
        f.points_lost = if f.category == "semantic" {
            f.budget
        } else {
            0.
        };
    }
    r.score();
    assert_eq!(r.score, Some(80.));
    assert_eq!(r.grade.as_deref(), Some("B"));
    assert_eq!(r.categories[2].grade.as_deref(), Some("F"));
    assert_eq!(
        r.categories.iter().map(|c| c.weight).collect::<Vec<_>>(),
        vec![20., 30., 20., 20., 10.]
    );
    for f in &mut r.findings {
        f.points_lost = f.budget;
        f.severity = "failure".into();
    }
    r.score();
    assert_eq!(r.grade.as_deref(), Some("F"));
    assert_eq!(r.categories[0].grade.as_deref(), Some("F"));
}
#[test]
fn wholly_inapplicable_categories_do_not_create_limitations() {
    let mut r = audit("", false);
    for f in &mut r.findings {
        f.points_lost = 0.;
        f.assessed_budget = f.budget;
        f.assessment_status = "assessed".into();
        if f.category == "wallpaper" {
            f.assessed_budget = 0.;
            f.assessment_status = "not-applicable".into();
        }
    }
    r.score();
    assert_eq!(r.state, "ready");
    assert_eq!(r.grade.as_deref(), Some("A"));
    assert_eq!(r.categories[4].state, "not-applicable");
    assert!(r.categories[4].grade.is_none());
}

#[test]
fn supported_translucent_surfaces_and_gradient_aliases_all_get_grades() {
    let mut shell = String::from("[hyprland]\nactive-border = '#88bbff #ff7777 35deg'\n");
    for section in [
        "bar",
        "popups",
        "notifications",
        "menu",
        "launcher",
        "polkit",
        "tooltip",
    ] {
        shell.push_str(&format!("[{section}]\nbackground-alpha = 0.96\n"));
        if section == "popups" {
            shell.push_str("border = 'hyprland.active-border'\n");
        }
    }
    let r = audit(&shell, false);
    assert!(
        r.categories
            .iter()
            .all(|c| c.grade.is_some() && c.state == "ready")
    );
    assert!(
        r.findings
            .iter()
            .filter(|f| f.scored)
            .all(|f| f.assessed_budget == f.budget)
    );
}
#[test]
fn stress_backgrounds_and_gradient_midpoints_can_expose_real_model_failures() {
    let r = audit(
        "[bar]\nbackground-alpha = 0\ntext = '#ffffff'\n[popups]\nbackground = '#808080'\nborder = '#000000 #ffffff 45deg'\nbackground-alpha = 1\n",
        false,
    );
    for id in ["readability.bar", "shell.popup-border"] {
        let f = r.findings.iter().find(|f| f.rule_id == id).unwrap();
        assert_eq!(f.severity, "failure");
        assert!(f.scored);
        assert_eq!(f.points_lost, f.budget);
        assert!(f.evidence["contrast"].as_f64().unwrap() < 1.5);
    }
    assert_eq!(
        r.findings
            .iter()
            .find(|f| f.rule_id == "readability.bar")
            .unwrap()
            .evidence["backgroundSamples"],
        17
    );
}

#[test]
fn restored_border_and_fill_budgets_contribute_to_the_grade() {
    let strong = audit(
        "[popups]\nborder = '#ffffff'\n[controls]\nselected-fill-alpha = 0.8\nfocus-fill-alpha = 0.8\n",
        false,
    );
    let subtle = audit(
        "[popups]\nborder = '#101315'\n[controls]\nnormal-fill-alpha = 0\nselected-fill-alpha = 0\nfocus-fill-alpha = 0\n",
        false,
    );
    assert!(strong.score.unwrap() > subtle.score.unwrap());
    for id in [
        "shell.popup-border",
        "shell.selected-fill",
        "shell.focus-fill",
    ] {
        let f = subtle.findings.iter().find(|f| f.rule_id == id).unwrap();
        assert_eq!(f.assessment_status, "assessed");
        assert!(f.budget > 0.);
        assert!(f.points_lost > 0.);
        assert!(f.scored);
    }
}

#[test]
fn restored_semantic_rules_retain_their_deductions() {
    let palette = fs::read_to_string("fixtures/themes/minimal/colors.toml")
        .unwrap()
        .replace("accent = \"#88bbff\"", "accent = \"#101315\"")
        .replace("red = \"#ff7777\"", "red = \"#101315\"")
        .replace("#34475b", "#101315");
    let r = audit_palette(
        "[menu]\nselected-text = '#ffffff'\n[launcher]\nselected-text = '#ffffff'\n",
        false,
        &palette,
    );
    assert!(r.score.unwrap() < 100.);
    for id in [
        "semantic.accent",
        "semantic.error-indicator",
        "semantic.selection",
    ] {
        let f = r.findings.iter().find(|f| f.rule_id == id).unwrap();
        assert!(f.scored);
        assert_eq!(f.severity, "failure");
        assert_eq!(f.points_lost, f.budget);
    }
}

#[test]
fn dim_ordinary_text_still_requires_four_point_five() {
    let r = audit(
        "[bar]\nbackground-alpha = 1\ntext = '#404040'\n[tooltip]\nbackground-alpha = 1\ntext = '#404040'\n",
        false,
    );
    for id in ["readability.bar", "readability.tooltip"] {
        let f = r.findings.iter().find(|f| f.rule_id == id).unwrap();
        assert!(f.scored);
        assert_eq!(f.evidence["required"], 4.5);
        assert_eq!(f.severity, "failure");
        assert_eq!(f.points_lost, f.budget);
    }
    assert!(r.score.unwrap() < 100.);
}
