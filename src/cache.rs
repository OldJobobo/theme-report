use crate::{
    inputs::{Config, Snapshot},
    model::Report,
};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

pub struct Lock(File);
impl Lock {
    pub fn acquire(cache: &Path) -> Result<Self> {
        fs::create_dir_all(cache)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let file = options.open(cache.join("current.lock"))?;
        if !file.metadata()?.is_file() {
            bail!("Cache lock is not a regular file")
        }
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            let deadline = Instant::now() + Duration::from_secs(35);
            loop {
                let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if rc == 0 {
                    break;
                }
                let e = std::io::Error::last_os_error();
                if e.kind() != std::io::ErrorKind::WouldBlock {
                    return Err(e.into());
                }
                if Instant::now() >= deadline {
                    bail!("Another audit is still running; lock deadline exceeded")
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        Ok(Self(file))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            unsafe {
                libc::flock(self.0.as_raw_fd(), libc::LOCK_UN);
            }
        }
    }
}
pub fn publish(cache: &Path, r: &Report) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(cache)?;
    serde_json::to_writer_pretty(temp.as_file_mut(), r)?;
    temp.write_all(b"\n")?;
    temp.as_file().sync_all()?;
    temp.persist(cache.join("current.json"))?;
    File::open(cache)?.sync_all()?;
    Ok(())
}
fn consistent(report: &Report) -> bool {
    for (id, category, budget, mandatory, blocking) in crate::rules::REGISTRY {
        let matches: Vec<_> = report.findings.iter().filter(|f| f.rule_id == id).collect();
        if matches.len() != 1 {
            return false;
        }
        let f = matches[0];
        if f.category != category
            || f.budget != budget
            || f.mandatory != mandatory
            || f.blocking != blocking
            || !f.scored
            || !(0. <= f.points_lost
                && f.points_lost <= f.assessed_budget
                && f.assessed_budget <= f.budget)
        {
            return false;
        }
    }
    if report
        .findings
        .iter()
        .any(|f| f.scored && !crate::rules::REGISTRY.iter().any(|r| r.0 == f.rule_id))
    {
        return false;
    }
    let mut recomputed = report.clone();
    recomputed.score();
    let Ok(original) = serde_json::to_value(report) else {
        return false;
    };
    let Ok(checked) = serde_json::to_value(recomputed) else {
        return false;
    };
    [
        "score",
        "grade",
        "state",
        "categories",
        "summary",
        "capReasons",
    ]
    .iter()
    .all(|key| original[key] == checked[key])
}

fn current_report(config: &Config) -> Option<Report> {
    let path = config.cache.join("current.json");
    let bytes = crate::inputs::bounded(&path, 4 * 1024 * 1024).ok()?;
    let r: Report = serde_json::from_slice(&bytes).ok()?;
    (r.schema_version == 2
        && r.analyzer_version == env!("CARGO_PKG_VERSION")
        && r.ruleset_version == crate::rules::RULESET_VERSION
        && r.target == "current"
        && r.profile == crate::inputs::PROFILE
        && r.theme_path == config.theme.to_string_lossy()
        && consistent(&r))
    .then_some(r)
}
pub fn load(config: &Config, snapshot: &Snapshot) -> Option<Report> {
    current_report(config)
        .filter(|r| r.input_fingerprint == snapshot.fingerprint && r.theme == snapshot.theme)
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Sidecar {
    manifest: String,
    input_fingerprint: String,
}
const SIDECAR: &str = "current.manifest";
/// Cache hit from metadata alone: the inputs' manifest matches the one recorded with the report.
fn load_fast(config: &Config, manifest: &str) -> Option<Report> {
    let bytes = crate::inputs::bounded(&config.cache.join(SIDECAR), 64 * 1024).ok()?;
    let sidecar: Sidecar = serde_json::from_slice(&bytes).ok()?;
    if sidecar.manifest != manifest {
        return None;
    }
    current_report(config).filter(|r| r.input_fingerprint == sidecar.input_fingerprint)
}
fn record(cache: &Path, manifest: &str, r: &Report) -> Result<()> {
    let mut temp = tempfile::NamedTempFile::new_in(cache)?;
    serde_json::to_writer(
        temp.as_file_mut(),
        &Sidecar {
            manifest: manifest.into(),
            input_fingerprint: r.input_fingerprint.clone(),
        },
    )?;
    temp.persist(cache.join(SIDECAR))?;
    Ok(())
}
pub fn audit(config: &Config, force: bool) -> Result<Report> {
    let _lock = if config.current {
        Some(Lock::acquire(&config.cache)?)
    } else {
        None
    };
    sweep(&config.cache);
    let mut delay = Duration::from_millis(150);
    for attempt in 0..3 {
        if attempt > 0 {
            // Let a theme switch finish writing before trying again.
            std::thread::sleep(delay);
            delay *= 3;
        }
        let manifest = crate::inputs::manifest(config)?;
        if config.current
            && !force
            && let Some(r) = load_fast(config, &manifest)
        {
            return Ok(r);
        }
        let snapshot = Snapshot::capture(config)?;
        if snapshot.manifest != manifest || !snapshot.stable()? {
            continue;
        }
        if config.current
            && !force
            && let Some(r) = load(config, &snapshot)
        {
            record(&config.cache, &manifest, &r)?;
            return Ok(r);
        }
        let r = crate::rules::audit(&snapshot);
        if !snapshot.stable()? {
            continue;
        }
        if config.current {
            publish(&config.cache, &r)?;
            record(&config.cache, &manifest, &r)?;
        }
        return Ok(r);
    }
    bail!("unstable-input: theme changed during capture or audit; previous report retained")
}

static STAGING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
/// This process's private directory for media copies and decoder scratch files.
pub fn staging(cache: &Path) -> Result<PathBuf> {
    let dir = cache.join(format!("run-{}", std::process::id()));
    if !dir.is_dir() {
        fs::create_dir_all(cache)?;
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        if let Ok(mut dirs) = STAGING.lock()
            && !dirs.contains(&dir)
        {
            dirs.push(dir.clone());
        }
    }
    Ok(dir)
}
/// Remove this process's staging directories (normal exit or termination signal).
pub fn cleanup_staging() {
    if let Ok(dirs) = STAGING.lock() {
        for dir in dirs.iter() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}
fn alive(pid: i32) -> bool {
    #[cfg(unix)]
    {
        let rc = unsafe { libc::kill(pid, 0) };
        rc == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}
/// Remove scratch files left by audits that were killed (closing the panel cancels audits).
pub fn sweep(cache: &Path) {
    let Ok(entries) = fs::read_dir(cache) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = entry.path().symlink_metadata() else {
            continue;
        };
        if let Some(pid) = name
            .strip_prefix("run-")
            .and_then(|p| p.parse::<i32>().ok())
        {
            if meta.is_dir() && pid != std::process::id() as i32 && !alive(pid) {
                let _ = fs::remove_dir_all(entry.path());
            }
        } else if name.starts_with(".tmp")
            && meta.is_file()
            // Older releases staged at the top level; publication temps live for milliseconds.
            && meta.modified().ok().and_then(|m| m.elapsed().ok()) > Some(Duration::from_secs(120))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}
