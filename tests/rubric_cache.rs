use std::{fs, path::Path};
use theme_report::{
    cache,
    inputs::{Config, PROFILE, Snapshot},
    rules,
};

#[test]
fn restored_rubric_cache_rejects_ruleset_six_and_unscored_registered_checks() {
    let work = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
    fs::create_dir_all(&work).unwrap();
    let t = tempfile::tempdir_in(work).unwrap();
    let theme = t.path().join("theme");
    fs::create_dir_all(theme.join("backgrounds")).unwrap();
    fs::copy(
        "fixtures/themes/minimal/colors.toml",
        theme.join("colors.toml"),
    )
    .unwrap();
    fs::copy(
        "fixtures/themes/minimal/backgrounds/night.png",
        theme.join("backgrounds/night.png"),
    )
    .unwrap();
    fs::write(theme.join("shell.toml"), "[bar]\nbackground-alpha = 0.7\n").unwrap();
    let config = Config {
        theme,
        current: false,
        home: t.path().into(),
        omarchy: t.path().into(),
        cache: t.path().join("cache"),
        profile: Some(PROFILE.into()),
    };
    fs::create_dir_all(&config.cache).unwrap();
    let snapshot = Snapshot::capture(&config).unwrap();
    let mut report = rules::audit(&snapshot);
    // Synthetic current-cache envelope; all IO stays inside this test directory.
    report.target = "current".into();
    cache::publish(&config.cache, &report).unwrap();
    assert!(cache::load(&config, &snapshot).is_some());
    let text = std::process::Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .args([
            "audit",
            config.theme.to_str().unwrap(),
            "--profile",
            PROFILE,
            "--cache-dir",
            config.cache.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(text.status.success());
    let text = String::from_utf8(text.stdout).unwrap();
    assert!(!text.contains("Guidance"));
    assert!(text.contains("rules 5-experimental"));
    assert!(
        report
            .findings
            .iter()
            .filter(|f| f.rule_id != "advisory.lua-syntax")
            .all(|f| f.scored && f.budget > 0.)
    );
    report.ruleset_version = "6-experimental".into();
    cache::publish(&config.cache, &report).unwrap();
    assert!(cache::load(&config, &snapshot).is_none());
    report.ruleset_version = rules::RULESET_VERSION.into();
    let text = report
        .findings
        .iter_mut()
        .find(|f| f.rule_id == "readability.foundation")
        .unwrap();
    text.scored = false;
    text.budget = 0.;
    text.assessed_budget = 0.;
    report.score();
    cache::publish(&config.cache, &report).unwrap();
    assert!(
        cache::load(&config, &snapshot).is_none(),
        "Registered checks cannot be silently removed from grading"
    );
}
