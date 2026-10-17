use crate::{
    inputs::{Config, Snapshot},
    model::Report,
};
use anyhow::{Result, bail};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::Path,
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

pub fn load(config: &Config, snapshot: &Snapshot) -> Option<Report> {
    let path = config.cache.join("current.json");
    let bytes = crate::inputs::bounded(&path, 4 * 1024 * 1024).ok()?;
    let r: Report = serde_json::from_slice(&bytes).ok()?;
    if r.schema_version == 2
        && r.analyzer_version == env!("CARGO_PKG_VERSION")
        && r.ruleset_version == crate::rules::RULESET_VERSION
        && r.target == "current"
        && r.profile == crate::inputs::PROFILE
        && r.input_fingerprint == snapshot.fingerprint
        && r.theme == snapshot.theme
        && r.theme_path == config.theme.to_string_lossy()
        && consistent(&r)
    {
        Some(r)
    } else {
        None
    }
}
pub fn audit(config: &Config, force: bool) -> Result<Report> {
    let _lock = if config.current {
        Some(Lock::acquire(&config.cache)?)
    } else {
        None
    };
    for _ in 0..3 {
        let snapshot = Snapshot::capture(config)?;
        if !snapshot.stable()? {
            continue;
        }
        if config.current
            && !force
            && let Some(r) = load(config, &snapshot)
        {
            return Ok(r);
        }
        let r = crate::rules::audit(&snapshot);
        if !snapshot.stable()? {
            continue;
        }
        if config.current {
            publish(&config.cache, &r)?;
        }
        return Ok(r);
    }
    bail!("unstable-input: theme changed during capture or audit; previous report retained")
}
