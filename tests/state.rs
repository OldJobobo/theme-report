//! The panel's saved-state reader: every file the panel shows is read here, bounded,
//! without following symlinks and without blocking on special files.
use serde_json::Value;
use std::{
    fs,
    os::unix::fs::symlink,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

fn tmp() -> tempfile::TempDir {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(".testbed/tests");
    fs::create_dir_all(&root).unwrap();
    tempfile::tempdir_in(root).unwrap()
}

struct Home {
    home: std::path::PathBuf,
    cache: std::path::PathBuf,
}

fn home(root: &Path) -> Home {
    let home = root.join("home");
    let cache = home.join(".cache/theme-report");
    fs::create_dir_all(cache.join("reviews")).unwrap();
    fs::create_dir_all(home.join(".local/state/omarchy/current")).unwrap();
    fs::create_dir_all(home.join(".config/omarchy/defaults")).unwrap();
    fs::write(
        home.join(".local/state/omarchy/current/theme.name"),
        "demo\n",
    )
    .unwrap();
    fs::write(home.join(".config/omarchy/defaults/agent"), "pi\n").unwrap();
    fs::write(
        cache.join("current.json"),
        r#"{"schemaVersion":2,"target":"current","theme":"demo","findings":[],"categories":[]}"#,
    )
    .unwrap();
    fs::write(cache.join("reviews/demo.md"), "# Review\n\nLooks fine.\n").unwrap();
    fs::write(
        cache.join("reviews/demo.json"),
        r#"{"agent":"pi","requestedAt":1}"#,
    )
    .unwrap();
    Home { home, cache }
}

fn state(h: &Home, args: &[&str]) -> Value {
    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_theme-report"))
        .arg("state")
        .args(args)
        .env("HOME", &h.home)
        .env("XDG_CACHE_HOME", h.home.join(".cache"))
        .output()
        .unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "state must never block"
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn reads_everything_the_panel_shows() {
    let t = tmp();
    let h = home(t.path());
    let s = state(&h, &[]);
    assert_eq!(s["schemaVersion"], 1);
    assert_eq!(s["themeName"], "demo");
    assert_eq!(s["agent"], "pi");
    assert_eq!(s["report"]["theme"], "demo");
    assert_eq!(s["review"]["theme"], "demo");
    assert_eq!(s["review"]["text"], "# Review\n\nLooks fine.\n");
    assert_eq!(s["review"]["meta"]["agent"], "pi");
    assert!(s["review"]["problem"].is_null());
    // An explicit theme selects whose review is read.
    fs::write(h.cache.join("reviews/other.md"), "Other").unwrap();
    assert_eq!(state(&h, &["--theme", "other"])["review"]["text"], "Other");
}

#[test]
fn missing_files_are_empty_not_errors() {
    let t = tmp();
    let h = home(t.path());
    fs::remove_file(h.cache.join("reviews/demo.md")).unwrap();
    fs::remove_file(h.cache.join("reviews/demo.json")).unwrap();
    fs::remove_file(h.home.join(".config/omarchy/defaults/agent")).unwrap();
    let s = state(&h, &[]);
    assert!(s["agent"].is_null());
    assert!(s["review"]["text"].is_null() && s["review"]["meta"].is_null());
    assert!(
        s["review"]["problem"].is_null(),
        "no review yet is not a problem"
    );
}

#[test]
fn symlinks_are_not_followed() {
    let t = tmp();
    let h = home(t.path());
    let secret = t.path().join("secret");
    fs::write(&secret, "private key").unwrap();
    for path in [
        h.cache.join("reviews/demo.md"),
        h.cache.join("reviews/demo.json"),
        h.cache.join("current.json"),
        h.home.join(".local/state/omarchy/current/theme.name"),
        h.home.join(".config/omarchy/defaults/agent"),
    ] {
        fs::remove_file(&path).unwrap();
        symlink(&secret, &path).unwrap();
    }
    let s = state(&h, &["--theme", "demo"]);
    let text = s.to_string();
    assert!(!text.contains("private key"), "{text}");
    assert!(s["report"].is_null() && s["themeName"].is_null() && s["agent"].is_null());
    assert!(s["review"]["text"].is_null() && s["review"]["meta"].is_null());
    assert!(
        s["review"]["problem"]
            .as_str()
            .unwrap()
            .contains("regular file")
    );
}

#[test]
fn special_files_never_block() {
    let t = tmp();
    let h = home(t.path());
    for path in [
        h.cache.join("reviews/demo.md"),
        h.home.join(".local/state/omarchy/current/theme.name"),
    ] {
        fs::remove_file(&path).unwrap();
        let c = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    }
    let s = state(&h, &["--theme", "demo"]);
    assert!(s["themeName"].is_null());
    assert!(s["review"]["text"].is_null());
    assert!(s["review"]["problem"].is_string());
}

#[test]
fn sizes_are_bounded() {
    let t = tmp();
    let h = home(t.path());
    // Long but within the read limit: shown, cut to the panel's length.
    fs::write(h.cache.join("reviews/demo.md"), "é".repeat(50_000)).unwrap();
    let s = state(&h, &[]);
    assert_eq!(
        s["review"]["text"].as_str().unwrap().chars().count(),
        40_000
    );
    // Over the read limit: refused with a reason, never loaded.
    fs::write(h.cache.join("reviews/demo.md"), "x".repeat(300 * 1024)).unwrap();
    fs::write(
        h.home.join(".config/omarchy/defaults/agent"),
        "a".repeat(8192),
    )
    .unwrap();
    let s = state(&h, &[]);
    assert!(s["review"]["text"].is_null());
    assert!(s["review"]["problem"].as_str().unwrap().contains("256 KiB"));
    assert!(s["agent"].is_null());
}

#[test]
fn unsafe_theme_names_read_no_review() {
    let t = tmp();
    let h = home(t.path());
    for theme in ["../demo", "demo/../../x", ".hidden", "", "a b"] {
        assert!(
            state(&h, &["--theme", theme])["review"].is_null(),
            "{theme:?}"
        );
    }
    // The same rule applies to the published report's theme.
    fs::write(
        h.cache.join("current.json"),
        r#"{"schemaVersion":2,"target":"current","theme":"../escape","findings":[],"categories":[]}"#,
    )
    .unwrap();
    assert!(state(&h, &[])["review"].is_null());
}
