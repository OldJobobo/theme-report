use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use theme_report::{
    inputs::{Config, PROFILE, Snapshot},
    model::Report,
    rules::REGISTRY,
};

fn tmp() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
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
fn run(path: &Path, cache: &Path) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            path.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--format",
            "json",
            "--cache-dir",
            cache.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
fn sample() -> Report {
    let t = tmp();
    let p = fixture(t.path());
    serde_json::from_value(run(&p, &t.path().join("cache"))).unwrap()
}
#[test]
fn minimal_theme_gets_a_without_overrides() {
    let r = sample();
    assert_eq!(r.grade.as_deref(), Some("A"));
    assert_eq!(r.state, "ready");
    assert_eq!(r.categories[1].grade.as_deref(), Some("A"));
    assert!(r.categories.iter().all(|c| if c.weight > 0. {
        c.state == "ready" && c.grade.is_some()
    } else {
        c.state == "not-applicable" && c.grade.is_none()
    }));
    assert_eq!(r.score, Some(100.));
    assert!(
        r.findings
            .iter()
            .all(|f| f.points_lost <= f.assessed_budget)
    );
}
#[test]
fn full_opaque_theme_is_fully_assessed() {
    let t = tmp();
    let p = fixture(t.path());
    fs::write(p.join("shell.toml"),"[launcher]\nbackground-alpha = 1\n[lock]\nbackground-alpha = 1\n[tooltip]\nbackground-alpha = 1\n").unwrap();
    let r = run(&p, &t.path().join("cache"));
    assert_eq!(r["state"], "ready");
    assert_eq!(r["grade"], "A");
}
#[test]
fn registry_budgets() {
    for (cat, weight) in theme_report::model::CATEGORIES {
        assert_eq!(
            REGISTRY
                .iter()
                .filter(|r| r.1 == cat)
                .map(|r| r.2)
                .sum::<f64>(),
            weight
        );
    }
    let ids: std::collections::BTreeSet<_> = REGISTRY.iter().map(|r| r.0).collect();
    assert_eq!(ids.len(), REGISTRY.len());
}
#[test]
fn determinism_and_path_cache_isolation() {
    let t = tmp();
    let p = fixture(t.path());
    let cache = t.path().join("cache");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("current.json"), "preserve").unwrap();
    let mut a = run(&p, &cache);
    let mut b = run(&p, &cache);
    a.as_object_mut().unwrap().remove("auditTimestamp");
    b.as_object_mut().unwrap().remove("auditTimestamp");
    assert_eq!(a, b);
    assert_eq!(
        fs::read_to_string(cache.join("current.json")).unwrap(),
        "preserve"
    );
}
#[test]
fn malformed_palette_is_failure_not_crash() {
    let t = tmp();
    let p = fixture(t.path());
    fs::write(p.join("colors.toml"), "not [valid").unwrap();
    let r = run(&p, &t.path().join("cache"));
    assert_eq!(r["state"], "provisional");
    assert_eq!(r["grade"], "F");
    assert!(!r["capReasons"].as_array().unwrap().is_empty());
    assert!(
        r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["ruleId"] == "validity.palette" && f["severity"] == "failure")
    );
}
#[test]
fn invalid_alpha_is_failure() {
    let t = tmp();
    let p = fixture(t.path());
    fs::write(p.join("shell.toml"), "[bar]\nbackground-alpha = 2\n").unwrap();
    let r = run(&p, &t.path().join("cache"));
    assert!(
        r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["ruleId"] == "validity.shell" && f["severity"] == "failure")
    );
}
#[test]
fn unknown_profile_cannot_grade() {
    let t = tmp();
    let p = fixture(t.path());
    let c = Config {
        theme: p,
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().join("cache"),
        profile: Some("future".into()),
    };
    let r = theme_report::rules::audit(&Snapshot::capture(&c).unwrap());
    assert!(r.grade.is_none());
    assert_eq!(r.state, "not-graded");
    assert!(r.categories.iter().all(|c| c.grade.is_none()));
    assert!(r.score.is_none());
}
#[test]
fn changing_inputs_invalidates_snapshot() {
    let t = tmp();
    let p = fixture(t.path());
    let c = Config {
        theme: p.clone(),
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().join("cache"),
        profile: Some(PROFILE.into()),
    };
    let s = Snapshot::capture(&c).unwrap();
    fs::write(p.join("shell.toml"), "[bar]\ntext = '#bbbbbb'\n").unwrap();
    assert!(!s.stable().unwrap());
}
#[test]
fn symlink_escape_is_rejected() {
    use std::os::unix::fs::symlink;
    let t = tmp();
    let p = fixture(t.path());
    fs::write(t.path().join("outside"), "secret").unwrap();
    fs::remove_file(p.join("colors.toml")).unwrap();
    symlink(t.path().join("outside"), p.join("colors.toml")).unwrap();
    let c = Config {
        theme: p,
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().into(),
        profile: Some(PROFILE.into()),
    };
    assert!(Snapshot::capture(&c).is_err());
}
#[test]
fn oversized_text_is_rejected() {
    let t = tmp();
    let p = fixture(t.path());
    fs::write(p.join("colors.toml"), vec![b'x'; 1_048_577]).unwrap();
    let c = Config {
        theme: p,
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().into(),
        profile: Some(PROFILE.into()),
    };
    assert!(Snapshot::capture(&c).is_err());
}
#[test]
fn corrupt_image_is_owned_by_validity() {
    let t = tmp();
    let p = fixture(t.path());
    fs::write(p.join("backgrounds/night.png"), b"\x89PNG\r\n\x1a\nbroken").unwrap();
    let r = run(&p, &t.path().join("cache"));
    assert!(
        r["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["ruleId"] == "validity.backgrounds" && f["severity"] == "failure")
    );
}
#[test]
fn wholly_unmeasured_category_does_not_withhold_other_grades() {
    let mut r = sample();
    for f in &mut r.findings {
        if f.category == "wallpaper" {
            f.assessed_budget = 0.;
            f.assessment_status = "unassessed".into();
        }
    }
    r.score();
    assert_eq!(r.grade.as_deref(), Some("A"));
    assert_eq!(r.state, "provisional");
    assert_eq!(r.categories[4].state, "not-checked");
    assert!(r.categories[4].grade.is_none());
}
#[test]
fn suppression_excludes_the_check_and_makes_grade_provisional() {
    let mut r = sample();
    let f = r
        .findings
        .iter_mut()
        .find(|f| f.rule_id == "readability.foundation")
        .unwrap();
    f.policy_suppressed = true;
    r.score();
    assert_eq!(r.grade.as_deref(), Some("A"));
    assert_eq!(r.state, "provisional");
    assert_eq!(r.categories[1].state, "limited");
}
#[test]
fn not_applicable_excludes_budget() {
    let mut r = sample();
    let before = r.categories[1].expected;
    let f = r
        .findings
        .iter_mut()
        .find(|f| f.rule_id == "readability.selection")
        .unwrap();
    f.assessment_status = "not-applicable".into();
    r.score();
    assert_eq!(r.categories[1].expected, before - 4.);
    assert_eq!(r.grade.as_deref(), Some("A"));
}
#[test]
fn zero_assessment_has_no_score() {
    let mut r = sample();
    for f in &mut r.findings {
        f.assessed_budget = 0.;
        f.points_lost = 0.;
        f.assessment_status = "unassessed".into();
    }
    r.score();
    assert!(r.score.is_none());
    assert!(r.grade.is_none());
    assert_eq!(r.state, "not-graded");
    assert!(r.categories.iter().all(|c| if c.weight > 0. {
        c.state == "not-checked"
    } else {
        c.state == "not-applicable"
    }));
}
#[test]
fn blocking_failure_caps_grade_without_overwriting_score() {
    let mut r = sample();
    let f = r
        .findings
        .iter_mut()
        .find(|f| f.rule_id == "validity.palette")
        .unwrap();
    f.severity = "failure".into();
    f.points_lost = 4.;
    r.score();
    assert_eq!(r.grade.as_deref(), Some("D"));
    assert!(r.score.unwrap() > 90.);
    assert_eq!(r.cap_reasons, vec!["validity.palette"]);
}
#[test]
fn numeric_grade_boundaries() {
    for (score, grade) in [
        (90., "A"),
        (89.999, "B"),
        (80., "B"),
        (79.999, "C"),
        (70., "C"),
        (69.999, "D"),
        (60., "D"),
        (59.999, "F"),
    ] {
        let mut r = sample();
        for f in &mut r.findings {
            f.assessment_status = "assessed".into();
            f.assessed_budget = f.budget;
            f.points_lost = f.budget * (100. - score) / 100.;
            f.blocking = false;
        }
        r.score();
        assert_eq!(r.grade.as_deref(), Some(grade), "score {score}");
        assert_eq!(r.state, "ready");
        for c in r.categories.iter().filter(|c| c.expected > 0.) {
            assert_eq!(
                c.grade.as_deref(),
                Some(grade),
                "category {} score {score}",
                c.id
            );
        }
    }
}
#[test]
fn operational_errors_are_json_and_nonzero() {
    let t = tmp();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            t.path().join("missing").to_str().unwrap(),
            "--profile",
            PROFILE,
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["state"], "error");
}
#[test]
fn shell_overlay_and_cycle() {
    let mut s = theme_report::rules::Shell::parse("[bar]\ntext = '#ffffff'\n");
    s.merge(theme_report::rules::Shell::parse(
        "[bar]\ntext = 'bar.text'\n",
    ));
    assert!(
        s.color(
            "bar.text",
            theme_report::color::Color::parse("#000000").unwrap(),
            &Default::default()
        )
        .is_none()
    );
}
