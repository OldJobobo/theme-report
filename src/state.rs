//! What the panel shows between audits: the last published report, the active theme
//! name, the default agent and the saved agent review. The panel only watches these
//! files for changes; every read happens here, through the same bounded reader the
//! audit uses (no symlinks, regular files only, nonblocking, size-capped).
use crate::{agent, inputs::bounded};
use serde_json::{Value, json};
use std::path::Path;

/// Published reports are written by the analyzer; anything larger is not one of ours.
pub const REPORT_LIMIT: u64 = 4 * 1024 * 1024;
/// Theme names and agent choices are one short line.
pub const LINE_LIMIT: u64 = 4096;
/// A saved review is read up to this size; larger files are reported, not shown.
pub const REVIEW_LIMIT: u64 = 256 * 1024;
/// The review record is a small JSON object written by `theme-report agent review`.
pub const RECORD_LIMIT: u64 = 64 * 1024;
/// The panel shows at most this many characters of a review.
pub const REVIEW_CHARS: usize = 40_000;

fn text(path: &Path, limit: u64) -> Option<String> {
    bounded(path, limit)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
}

/// The first line of a small text file, trimmed.
pub fn first_line(path: &Path) -> Option<String> {
    let line = text(path, LINE_LIMIT)?.lines().next()?.trim().to_string();
    (!line.is_empty()).then_some(line)
}

/// Theme names the panel and prompts accept as file-name components.
pub fn review_theme_ok(theme: &str) -> bool {
    let mut chars = theme.chars();
    theme.len() <= 64
        && chars.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn review(env: &agent::Env, theme: &str) -> Value {
    let (path, record) = agent::review_paths(env, theme);
    let meta = text(&record, RECORD_LIMIT)
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .filter(Value::is_object);
    let (body, problem) = match bounded(&path, REVIEW_LIMIT) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(t) => (Some(t.chars().take(REVIEW_CHARS).collect::<String>()), None),
            Err(_) => (None, Some("The saved review is not UTF-8 text.")),
        },
        Err(_) if path.symlink_metadata().is_err() => (None, None),
        Err(_) => (
            None,
            Some("The saved review could not be read: it must be a regular file under 256 KiB."),
        ),
    };
    json!({ "theme": theme, "text": body, "meta": meta, "problem": problem })
}

/// The panel state as JSON. `theme` picks whose review to read; without it, the
/// published report's theme is used.
pub fn panel_state(env: &agent::Env, theme: Option<&str>) -> Value {
    let report = bounded(&env.cache.join("current.json"), REPORT_LIMIT)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(Value::is_object);
    let theme_name = first_line(&env.home.join(".local/state/omarchy/current/theme.name"));
    let review_theme = theme
        .map(str::to_string)
        .or_else(|| {
            report
                .as_ref()
                .and_then(|r| r.get("theme"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .filter(|t| review_theme_ok(t));
    json!({
        "schemaVersion": 1,
        "themeName": theme_name,
        "agent": agent::default_agent(env),
        "report": report,
        "review": review_theme.map(|t| review(env, &t)),
    })
}
