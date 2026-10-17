use crate::{
    color::Color,
    contract::Contract,
    inputs::{PROFILE, Snapshot},
    model::{Finding, Report, Source},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

pub const RULESET_VERSION: &str = "5-experimental";

pub const REGISTRY: [(&str, &str, f64, bool, bool); 22] = [
    ("validity.palette", "validity", 4., true, true),
    ("validity.roles", "validity", 8., true, true),
    ("validity.shell", "validity", 4., true, true),
    ("validity.backgrounds", "validity", 4., false, false),
    ("readability.foundation", "readability", 5., false, false),
    ("readability.selection", "readability", 4., false, false),
    ("readability.bar", "readability", 3., false, false),
    ("readability.popups", "readability", 3., false, false),
    ("readability.notifications", "readability", 3., false, false),
    ("readability.menu.selected", "readability", 4., false, false),
    (
        "readability.launcher.selected",
        "readability",
        2.,
        false,
        false,
    ),
    ("readability.polkit", "readability", 3., false, false),
    ("readability.lock", "readability", 2., false, false),
    ("readability.tooltip", "readability", 1., false, false),
    ("semantic.accent", "semantic", 8., false, false),
    ("semantic.error-indicator", "semantic", 6., false, false),
    ("semantic.selection", "semantic", 6., false, false),
    ("shell.selected-fill", "shell", 8., false, false),
    ("shell.focus-fill", "shell", 6., false, false),
    ("shell.popup-border", "shell", 6., false, false),
    ("wallpaper.lock-model", "wallpaper", 5., false, false),
    ("wallpaper.launcher-model", "wallpaper", 5., false, false),
];
fn finding(id: &str, summary: &str) -> Finding {
    let (_, cat, budget, mandatory, blocking) = REGISTRY
        .iter()
        .find(|r| r.0 == id)
        .expect("registered rule");
    Finding {
        rule_id: id.into(),
        rule_version: 1,
        category: (*cat).into(),
        severity: "unassessed".into(),
        assessment_status: "unassessed".into(),
        method: "resolved-token".into(),
        scored: true,
        policy_suppressed: false,
        budget: *budget,
        assessed_budget: 0.,
        points_lost: 0.,
        blocking: *blocking,
        mandatory: *mandatory,
        summary: summary.into(),
        evidence: json!({"reason":"Required colors or opacity could not be resolved from this theme and its defaults."}),
        sources: vec![source(
            match id {
                "validity.shell" => "shell.toml",
                "validity.backgrounds" => "backgrounds/",
                _ if *cat == "validity"
                    || *cat == "semantic"
                    || id == "readability.foundation"
                    || id == "readability.selection" =>
                {
                    "colors.toml"
                }
                _ => "shell.toml",
            },
            "",
            "",
        )],
        remediation: "Inspect the referenced colors and opacity settings. A limitation is not a theme failure.".into(),
    }
}
fn paused(id: &str, contract: &Contract) -> Finding {
    let mut f = finding(id, "Paused: Omarchy changed how this is rendered");
    f.evidence = json!({"reason":format!("This Omarchy build changed {} in a way this analyzer version has not reviewed, so the check is paused instead of guessed.", contract.unverified.join(", ")),"runtimeContract":contract.status,"unverified":contract.unverified});
    f.remediation =
        "Update theme-report for this Omarchy build. `theme-report runtime` shows what changed."
            .into();
    f
}
fn boolean(mut f: Finding, ok: bool, evidence: Value) -> Finding {
    f.assessment_status = "assessed".into();
    f.assessed_budget = f.budget;
    f.severity = if ok { "pass" } else { "failure" }.into();
    f.points_lost = if ok { 0. } else { f.budget };
    f.evidence = evidence;
    f.remediation = if ok {
        "No change needed."
    } else {
        match f.rule_id.as_str() {
            "validity.palette" => "Fix the TOML syntax in colors.toml, or restore the missing palette file.",
            "validity.roles" => "Define supported solid background, foreground, accent, and urgent colors in colors.toml.",
            "validity.shell" => "Correct the invalid keys listed below in shell.toml or the user shell override.",
            "validity.backgrounds" => "Restore the missing selected background or replace the malformed media named below.",
            _ => "Correct the referenced input.",
        }
    }.into();
    f
}
fn metric(mut f: Finding, value: f64, pass: f64, floor: f64, evidence: Value) -> Finding {
    if !value.is_finite() {
        return f;
    }
    f.assessment_status = "assessed".into();
    f.assessed_budget = f.budget;
    f.severity = if value >= pass {
        "pass"
    } else if value >= floor {
        "caution"
    } else {
        "failure"
    }
    .into();
    f.points_lost = if value >= pass {
        0.
    } else if value >= floor {
        f.budget / 2.
    } else {
        f.budget
    };
    f.evidence = evidence;
    f.remediation = if value >= pass {
        "No change needed."
    } else {
        "Increase separation between these functional roles; preserve text readability."
    }
    .into();
    f
}
fn source(file: &str, section: &str, key: &str) -> Source {
    Source {
        file: file.into(),
        section: section.into(),
        key: key.into(),
    }
}
fn contrast(
    mut f: Finding,
    fg: Color,
    bg: Color,
    required: f64,
    file: &str,
    section: &str,
    key: &str,
) -> Finding {
    f.sources.push(source(file, section, key));
    if file == "colors.toml" {
        f.sources.push(source(
            file,
            "",
            if key == "selection_foreground" {
                "selection_background"
            } else {
                "background"
            },
        ));
    }
    let value = fg.contrast(bg);
    let mut result = metric(
        f,
        value,
        required,
        if required >= 4.5 { 3. } else { 1.5 },
        json!({"foreground":fg.hex(),"effectiveBackground":bg.hex(),"contrast":value,"required":required,"floor":if required>=4.5{3.}else{1.5}}),
    );
    if result.points_lost > 0. {
        result.remediation = if file == "colors.toml" {
            format!(
                "Adjust {key} or its background in colors.toml until contrast reaches {required}:1."
            )
        } else {
            format!(
                "Adjust [{section}] {key} or background/background-alpha in shell.toml until contrast reaches {required}:1."
            )
        };
    }
    result
}

// Matches the runtime's intentionally limited line-based shell parser, not a general TOML interpreter.
#[derive(Default)]
pub struct Shell {
    pub values: BTreeMap<String, String>,
    pub ignored: Vec<String>,
    pub invalid: Vec<String>,
}
impl Shell {
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        let mut section = String::new();
        for (line_no, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[')
                && let Some(end) = line.find(']')
            {
                let name = &line[1..end];
                if name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    section = name.into();
                    continue;
                }
            }
            let Some((key, value)) = line.split_once('=') else {
                s.ignored.push(format!("line {}", line_no + 1));
                continue;
            };
            let key = key.trim();
            let value = value.trim();
            if section.is_empty()
                || !key
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                s.ignored.push(format!("line {}", line_no + 1));
                continue;
            }
            let v = if value.starts_with('"') || value.starts_with('\'') {
                let quote = value.chars().next().unwrap();
                if let Some(end) = value[1..].find(quote) {
                    let tail = value[end + 2..].trim();
                    if !tail.is_empty() && !tail.starts_with('#') {
                        s.ignored.push(format!("line {}", line_no + 1));
                        continue;
                    }
                    value[1..end + 1].to_string()
                } else {
                    s.ignored.push(format!("line {}", line_no + 1));
                    continue;
                }
            } else {
                value.split('#').next().unwrap_or("").trim().into()
            };
            let role = format!("{section}.{key}");
            let known_section = matches!(
                section.as_str(),
                "bar"
                    | "hyprland"
                    | "controls"
                    | "spacing"
                    | "font"
                    | "popups"
                    | "tooltip"
                    | "notifications"
                    | "launcher"
                    | "menu"
                    | "polkit"
                    | "lock"
                    | "image-picker"
            );
            let known_alpha = matches!(
                key,
                "background-alpha"
                    | "border-alpha"
                    | "scrim-alpha"
                    | "selected-background-alpha"
                    | "selected-border-alpha"
                    | "unselected-border-alpha"
                    | "selection-alpha"
                    | "normal-fill-alpha"
                    | "normal-border-alpha"
                    | "hover-cursor-fill-alpha"
                    | "hover-cursor-border-alpha"
                    | "focus-fill-alpha"
                    | "focus-border-alpha"
                    | "selected-fill-alpha"
                    | "pressed-fill-alpha"
                    | "selection-fill-alpha"
            );
            if !known_section {
                s.ignored.push(format!("unsupported key {role}"));
                s.values.insert(role, v);
                continue;
            }
            if key.ends_with("alpha") && !known_alpha {
                s.ignored.push(format!("unsupported key {role}"));
            }
            if known_alpha
                && v.parse::<f64>()
                    .ok()
                    .is_none_or(|n| !n.is_finite() || !(0. ..=1.).contains(&n))
            {
                s.invalid.push(role.clone());
            }
            if matches!(
                key,
                "border-width"
                    | "normal-border-width"
                    | "hover-cursor-border-width"
                    | "focus-border-width"
                    | "selected-border-width"
                    | "base-size"
                    | "scale"
                    | "size-horizontal"
                    | "size-vertical"
            ) {
                let numbers: Vec<_> = v.split_whitespace().map(str::parse::<f64>).collect();
                if numbers.is_empty()
                    || numbers.iter().any(|n| {
                        n.as_ref().is_err() || n.as_ref().is_ok_and(|n| !n.is_finite() || *n < 0.)
                    })
                {
                    s.invalid.push(role.clone());
                }
            }
            if v.contains("{{") {
                s.invalid.push(role.clone());
            }
            s.values.insert(role, v);
        }
        s
    }
    pub fn merge(&mut self, other: Self) {
        self.values.extend(other.values);
        self.ignored.extend(other.ignored);
        self.invalid.extend(other.invalid);
    }
    pub fn alpha(&self, key: &str, fallback: f64) -> Option<f64> {
        match self.values.get(key) {
            Some(v) => v
                .parse()
                .ok()
                .filter(|n: &f64| n.is_finite() && (0. ..=1.).contains(n)),
            None => Some(fallback),
        }
    }
    pub fn color(
        &self,
        key: &str,
        fallback: Color,
        palette: &BTreeMap<String, Color>,
    ) -> Option<Color> {
        let Some(v) = self.values.get(key) else {
            return Some(fallback);
        };
        let mut token = v.as_str();
        let mut seen = std::collections::BTreeSet::new();
        while let Some(next) = self.values.get(token) {
            if !seen.insert(token) {
                return None;
            }
            token = next;
        }
        Color::parse(token)
            .ok()
            .or_else(|| palette.get(token).copied())
    }
}
fn palette(s: &Snapshot) -> Option<BTreeMap<String, Color>> {
    let raw: sup::Table = s.text("colors.toml")?.parse().ok()?;
    let mut colors = BTreeMap::new();
    for (k, v) in raw {
        if let Some(c) = v.as_str().and_then(|s| Color::parse(s).ok()) {
            colors.insert(k, c);
        }
    }
    for (role, alias) in [
        ("background", "color0"),
        ("foreground", "color7"),
        ("accent", "color4"),
        ("urgent", "red"),
        ("muted", "color8"),
    ] {
        if !colors.contains_key(role)
            && let Some(c) = colors.get(alias).copied()
        {
            colors.insert(role.into(), c);
        }
    }
    // Terminal selection fallbacks mirror the pinned theme-color contract; do not execute it.
    for (role, candidates) in [
        ("bright_foreground", vec!["color15", "foreground"]),
        (
            "selection",
            vec!["selection_background", "color8", "color0", "background"],
        ),
        ("selection_background", vec!["selection"]),
        ("selection_foreground", vec!["bright_foreground"]),
    ] {
        if !colors.contains_key(role)
            && let Some(c) = candidates.iter().find_map(|key| colors.get(*key).copied())
        {
            colors.insert(role.into(), c);
        }
    }
    // Color.loadColors assigns urgent in file order from red/color1.
    for line in s.text("colors.toml")?.lines() {
        if let Some((key, value)) = line.split_once('=')
            && matches!(key.trim(), "red" | "color1")
        {
            let token = value.trim().trim_start_matches(['\"', '\'']);
            if token.len() >= 7
                && token.is_ascii()
                && let Ok(c) = Color::parse(&token[..7])
            {
                colors.insert("urgent".into(), c);
            }
        }
    }
    // Runtime accent/muted may retain state if omitted; never infer previous-theme colors.
    Some(colors)
}
mod sup {
    pub type Table = toml::Table;
}
fn opaque_surface(shell: &Shell, p: &BTreeMap<String, Color>, section: &str) -> Option<Color> {
    let bg = *p.get("background")?;
    let a = shell.alpha(
        &format!("{section}.background-alpha"),
        default_alpha(section),
    )?;
    if a != 1. {
        return None;
    }
    shell.color(&format!("{section}.background"), bg, p)
}
fn default_alpha(section: &str) -> f64 {
    match section {
        "tooltip" => 0.97,
        "launcher" => 0.95,
        "lock" => 0.8,
        _ => 1.,
    }
}

// One spatial/temporal stress calculation, reused for known lock wallpaper and the launcher scenario.
fn wallpaper_pair(
    shell: &Shell,
    p: &BTreeMap<String, Color>,
    section: &str,
    selected: bool,
    samples: &[(String, Vec<Color>)],
) -> Option<(f64, String, Color, Color)> {
    let bg = *p.get("background")?;
    let fg = *p.get("foreground")?;
    let base = shell.color(&format!("{section}.background"), bg, p)?;
    let alpha = shell.alpha(
        &format!("{section}.background-alpha"),
        default_alpha(section),
    )?;
    let text = shell.color(
        &format!(
            "{section}.{}",
            if selected { "selected-text" } else { "text" }
        ),
        if selected { *p.get("accent")? } else { fg },
        p,
    )?;
    let mut worst = (f64::INFINITY, String::new(), bg, text);
    for (name, colors) in samples {
        for &wall in colors {
            let under = if section == "launcher" {
                shell
                    .color("launcher.scrim", bg, p)?
                    .over(shell.alpha("launcher.scrim-alpha", 0.5)?, wall)
            } else {
                wall
            };
            let mut surface = base.over(alpha, under);
            if selected {
                surface = shell
                    .color(&format!("{section}.selected-background"), fg, p)?
                    .over(
                        shell.alpha(&format!("{section}.selected-background-alpha"), 0.08)?,
                        surface,
                    );
            }
            let ratio = text.contrast(surface);
            if ratio < worst.0 {
                worst = (ratio, name.clone(), surface, text);
            }
        }
    }
    worst.0.is_finite().then_some(worst)
}

const STRESS_MODEL: &str = "application-gray17-v4";
fn stress_backgrounds() -> Vec<(String, Vec<Color>)> {
    vec![(
        STRESS_MODEL.into(),
        (0..=16).map(|i| Color([i as f64 / 16.; 3])).collect(),
    )]
}
fn stress_surfaces(
    shell: &Shell,
    p: &BTreeMap<String, Color>,
    section: &str,
) -> Option<Vec<Color>> {
    if let Some(c) = opaque_surface(shell, p, section) {
        return Some(vec![c]);
    }
    let base = shell.color(&format!("{section}.background"), *p.get("background")?, p)?;
    let alpha = shell.alpha(
        &format!("{section}.background-alpha"),
        default_alpha(section),
    )?;
    Some(
        stress_backgrounds()[0]
            .1
            .iter()
            .map(|&under| base.over(alpha, under))
            .collect(),
    )
}
// Grade native solid/gradient borders across stops and intermediate encoded-sRGB colors, not a flattened token.
fn border_colors(
    shell: &Shell,
    p: &BTreeMap<String, Color>,
    opacity: f64,
) -> Option<Vec<crate::border::Sample>> {
    let mut token = shell
        .values
        .get("popups.border")
        .map(String::as_str)
        .unwrap_or("accent");
    let mut seen = std::collections::BTreeSet::new();
    while let Some(next) = shell.values.get(token) {
        if !seen.insert(token) {
            return None;
        }
        token = next;
    }
    let stop = |token: &str| {
        crate::border::parse(token, opacity).or_else(|| {
            p.get(token).copied().map(|color| crate::border::Sample {
                color,
                alpha: opacity,
            })
        })
    };
    if let Some(c) = stop(token) {
        return Some(vec![c]);
    }
    let mut words: Vec<_> = token.split_whitespace().collect();
    if let Some(angle) = words.last().and_then(|s| s.strip_suffix("deg")) {
        if !angle.parse::<f64>().ok()?.is_finite() {
            return None;
        }
        words.pop();
    }
    if !(2..=8).contains(&words.len()) {
        return None;
    }
    let stops: Vec<_> = words.iter().map(|s| stop(s)).collect::<Option<_>>()?;
    Some(
        stops
            .windows(2)
            .flat_map(|pair| (0..=16).map(move |i| pair[0].mix(pair[1], i as f64 / 16.)))
            .collect(),
    )
}
fn installed_version() -> Option<String> {
    for package in ["omarchy-dev", "omarchy"] {
        if let Ok(output) = std::process::Command::new("/usr/bin/pacman")
            .args(["-Q", package])
            .output()
            && output.status.success()
        {
            return String::from_utf8(output.stdout)
                .ok()?
                .split_whitespace()
                .nth(1)
                .map(str::to_owned);
        }
    }
    None
}

pub fn audit(s: &Snapshot) -> Report {
    let mut r=Report{schema_version:2,analyzer_version:env!("CARGO_PKG_VERSION").into(),ruleset_version:RULESET_VERSION.into(),profile:PROFILE.into(),omarchy_version:if s.config.current { installed_version() } else { None },target:if s.config.current{"current"}else{"materialized-path"}.into(),theme:s.theme.clone(),theme_path:s.config.theme.to_string_lossy().into(),input_fingerprint:s.fingerprint.clone(),state:"not-graded".into(),score:None,grade:None,summary:String::new(),cap_reasons:vec![],categories:vec![],findings:vec![],assumptions:vec!["Resolved-token checks do not reproduce runtime pixels or transient palette state.".into(),"Translucent application-backed surfaces use 17 reference gray backgrounds; grades describe sampled stress resilience, not live window pixels.".into(),"Wallpaper rules are fixed unblurred wallpaper stress models, not lock/launcher runtime accessibility claims.".into(),"Video uses five deterministic temporal samples of 8-bit SDR media, not every frame; decoder identity participates in the fingerprint.".into(),"Experimental semantic/state thresholds are published policy, not universal accessibility standards.".into()],watched_paths:s.paths.values().cloned().chain([s.config.theme.to_string_lossy().into_owned(),s.config.theme.join("backgrounds").to_string_lossy().into_owned()]).collect(),audit_timestamp:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),runtime_contract:s.config.current.then(|| s.contract.to_json())};
    if !s.contract.palette {
        r.findings = REGISTRY
            .iter()
            .map(|(id, _, _, _, _)| paused(id, &s.contract))
            .collect();
        r.score();
        return r;
    }
    let raw = s.text("colors.toml");
    let parsed = raw.and_then(|x| x.parse::<toml::Table>().ok());
    let f = boolean(
        finding("validity.palette", "Palette parses as TOML"),
        parsed.is_some(),
        json!({"file":"colors.toml","parseable":parsed.is_some()}),
    );
    r.findings.push(f);
    let p = palette(s).unwrap_or_default();
    let missing: Vec<_> = ["background", "foreground", "accent", "urgent"]
        .into_iter()
        .filter(|k| !p.contains_key(*k))
        .collect();
    r.findings.push(boolean(
        finding(
            "validity.roles",
            "Foundational roles resolve without transient state",
        ),
        missing.is_empty(),
        json!({"missingOrUnsupported":missing,"resolvedColors":(["background","foreground","accent","urgent"].iter().filter_map(|key|p.get(*key).map(|c|((*key).to_string(),c.hex()))).collect::<BTreeMap<_,_>>()),"paletteColors":p.iter().map(|(key,color)|(key.clone(),color.hex())).collect::<BTreeMap<_,_>>()}),
    ));
    let mut shell = Shell::parse(s.text("shell.toml").unwrap_or(""));
    let user = Shell::parse(s.text("user.shell.toml").unwrap_or(""));
    // The theme owns its invalid values; a machine-level override is reported separately.
    let theme_invalid = std::mem::take(&mut shell.invalid);
    let overridden: Vec<_> = theme_invalid
        .iter()
        .filter(|key| user.values.contains_key(*key))
        .cloned()
        .collect();
    shell.merge(Shell {
        values: user.values.clone(),
        ignored: user
            .ignored
            .iter()
            .map(|line| format!("user.shell.toml {line}"))
            .collect(),
        invalid: vec![],
    });
    r.findings.push(boolean(finding("validity.shell","Known shell domains and generated tokens are valid"),theme_invalid.is_empty(),json!({"invalidKeys":theme_invalid,"overriddenOnThisMachine":overridden,"ignoredLines":shell.ignored,"parser":"runtime-line-subset"})));
    let images: Vec<_> = s
        .files
        .iter()
        .filter(|(k, _)| k.starts_with("backgrounds/") || *k == "current.background")
        .collect();
    let mut samples = Vec::new();
    let mut errors = Vec::new();
    let mut unsupported: Vec<_> = s
        .unsupported_backgrounds
        .iter()
        .map(|(role, reason)| format!("{role}: {reason}"))
        .collect();
    let mut decoded_hashes = std::collections::BTreeSet::new();
    let start = std::time::Instant::now();
    let mut image_evidence = serde_json::Map::new();
    for (name, bytes) in images {
        if !decoded_hashes.insert(crate::inputs::hash(bytes)) {
            continue;
        }
        if start.elapsed().as_secs() > 25 {
            unsupported.push(format!("{name}: total decoding deadline"));
            continue;
        }
        match crate::wallpaper::sample(bytes, &s.config.cache) {
            Ok(v) => {
                let mut evidence = v.evidence;
                evidence["sampleColors"] =
                    json!(v.colors.iter().map(|c| c.hex()).collect::<Vec<_>>());
                image_evidence.insert(name.clone(), evidence);
                samples.push((name.clone(), v.colors));
            }
            Err(e) => {
                let text = e.to_string();
                if text.starts_with("invalid-image:") {
                    errors.push(format!("{name}: {text}"))
                } else {
                    unsupported.push(format!("{name}: {text}"))
                }
            }
        }
    }
    for (name, image) in &s.large_images {
        if !decoded_hashes.insert(image.hash.clone()) {
            continue;
        }
        if start.elapsed().as_secs() > 25 {
            unsupported.push(format!("{name}: total decoding deadline"));
            continue;
        }
        match crate::wallpaper::sample_path(image.file.path(), &s.config.cache) {
            Ok(v) => {
                let mut evidence = v.evidence;
                evidence["sampleColors"] =
                    json!(v.colors.iter().map(|c| c.hex()).collect::<Vec<_>>());
                evidence["sourceSha256"] = json!(image.hash);
                evidence["sourceBytes"] = json!(image.size);
                evidence["snapshot"] = json!("disk-backed immutable copy");
                image_evidence.insert(name.clone(), evidence);
                samples.push((name.clone(), v.colors));
            }
            Err(e) => {
                let text = e.to_string();
                if text.starts_with("invalid-image:") {
                    errors.push(format!("{name}: {text}"));
                } else {
                    unsupported.push(format!("{name}: {text}"));
                }
            }
        }
    }
    let mut media_evidence = serde_json::Map::new();
    for (name, video) in &s.videos {
        if !decoded_hashes.insert(video.hash.clone()) {
            continue;
        }
        if start.elapsed().as_secs() > 25 {
            unsupported.push(format!("{name}: total decoding deadline"));
            continue;
        }
        match crate::video::sample(video) {
            Ok(v) => {
                media_evidence.insert(name.clone(), v.evidence);
                samples.push((name.clone(), v.colors));
            }
            Err(e) => {
                let text = e.to_string();
                if text.starts_with("invalid-video:") {
                    errors.push(format!("{name}: {text}"));
                } else {
                    unsupported.push(format!("{name}: {text}"));
                }
            }
        }
    }
    let mut assets = finding(
        "validity.backgrounds",
        "Wallpaper media parses and sampled frames decode",
    );
    assets.rule_version = 4;
    assets.method = "bounded-media-decode".into();
    let media_complete = !samples.is_empty() && unsupported.is_empty() && errors.is_empty();
    let selected_missing = s.config.current
        && !s.files.contains_key("current.background")
        && !s.unsupported_backgrounds.contains_key("current.background")
        && !s.videos.contains_key("current.background")
        && !s.large_images.contains_key("current.background");
    if selected_missing || !errors.is_empty() {
        assets = boolean(
            assets,
            false,
            json!({"missingCurrent":selected_missing,"errors":errors}),
        );
    } else if !samples.is_empty() && unsupported.is_empty() {
        assets = boolean(
            assets,
            true,
            json!({"decoded":samples.len(),"videos":media_evidence,"thumbnails":image_evidence}),
        );
    } else {
        assets.evidence = json!({"reason":"no supported background context or bounded decoding incomplete","unsupported":unsupported,"videos":media_evidence,"thumbnails":image_evidence});
    }
    for name in s
        .paths
        .keys()
        .filter(|name| name.starts_with("backgrounds/") || name.as_str() == "current.background")
    {
        assets.sources.push(source(name, "", ""));
    }
    if !s.ignored_backgrounds.is_empty() {
        assets.evidence["ignored"] = json!(s.ignored_backgrounds);
    }
    r.findings.push(assets);
    if let (Some(&bg), Some(&fg)) = (p.get("background"), p.get("foreground")) {
        r.findings.push(contrast(
            finding("readability.foundation", "Foundational text contrast"),
            fg,
            bg,
            4.5,
            "colors.toml",
            "",
            "foreground",
        ));
        let mut sel = finding("readability.selection", "Terminal selected text contrast");
        if let (Some(&sf), Some(&sb)) =
            (p.get("selection_foreground"), p.get("selection_background"))
        {
            sel = contrast(sel, sf, sb, 4.5, "colors.toml", "", "selection_foreground");
        }
        r.findings.push(sel);
        for (id, section, selected) in [
            ("readability.bar", "bar", false),
            ("readability.popups", "popups", false),
            ("readability.notifications", "notifications", false),
            ("readability.menu.selected", "menu", true),
            ("readability.launcher.selected", "launcher", true),
            ("readability.polkit", "polkit", false),
            ("readability.lock", "lock", false),
            ("readability.tooltip", "tooltip", false),
        ] {
            let mut f = finding(id, &format!("{section} text contrast"));
            for key in [
                "background",
                "background-alpha",
                if selected { "selected-text" } else { "text" },
            ] {
                f.sources.push(source("shell.toml", section, key));
            }
            if selected {
                for key in ["selected-background", "selected-background-alpha"] {
                    f.sources.push(source("shell.toml", section, key));
                }
            }
            if let Some(alpha) = shell.alpha(
                &format!("{section}.background-alpha"),
                default_alpha(section),
            ) && alpha < 1.
            {
                let reason = if section == "lock" {
                    "The lock is wallpaper-backed, but its media or colors could not be completely sampled.".to_string()
                } else {
                    format!(
                        "The {section} is translucent over application content. Its background varies with the windows beneath it."
                    )
                };
                f.evidence = json!({"reason":reason,"context":if section=="lock" {"wallpaper-backed"} else {"application-backed"}});
                f.remediation = if section == "lock" {
                    "Inspect wallpaper decoding and the lock color settings; no live screen capture is needed.".into()
                } else {
                    "Check this surface over light and dark windows. Increasing background-alpha makes its text contrast less dependent on those windows.".into()
                };
            }
            if section == "lock"
                && opaque_surface(&shell, &p, section).is_none()
                && media_complete
                && !selected_missing
                && let Some(worst) = wallpaper_pair(&shell, &p, section, false, &samples)
            {
                f.rule_version = 3;
                f.method = "modeled-appearance".into();
                f.sources.push(source(&worst.1, "", ""));
                f = metric(
                    f,
                    worst.0,
                    4.5,
                    3.,
                    json!({"contrast":worst.0,"required":4.5,"background":worst.1,"effectiveBackground":worst.2.hex(),"foreground":worst.3.hex(),"context":"wallpaper-backed","sampleModel":"center-crop-160x90-grid8-video5-v2","excludes":"native blur, lock video dimming, monitor transforms"}),
                );
                if f.points_lost > 0. {
                    f.remediation = "Increase [lock] background-alpha or change lock text/background colors to reach 4.5:1 over the sampled wallpaper frames.".into();
                }
            }
            if section != "lock"
                && opaque_surface(&shell, &p, section).is_none()
                && let Some(worst) =
                    wallpaper_pair(&shell, &p, section, selected, &stress_backgrounds())
            {
                f.rule_version = 4;
                f.method = "modeled-appearance".into();
                f = metric(
                    f,
                    worst.0,
                    4.5,
                    3.,
                    json!({"contrast":worst.0,"required":4.5,"effectiveBackground":worst.2.hex(),"foreground":worst.3.hex(),"backgroundModel":STRESS_MODEL,"backgroundSamples":17,"scope":"text contrast over reference dark-to-light application backgrounds, not captured live windows"}),
                );
                if f.points_lost > 0. {
                    f.remediation="Increase this surface's background-alpha or adjust its text/background colors to reach 4.5:1 across the reference background scenarios.".into();
                }
            }
            if let Some(mut surface) = opaque_surface(&shell, &p, section) {
                let key = if selected { "selected-text" } else { "text" };
                if selected {
                    if let (Some(fill), Some(a)) = (
                        shell.color(&format!("{section}.selected-background"), fg, &p),
                        shell.alpha(&format!("{section}.selected-background-alpha"), 0.08),
                    ) {
                        surface = fill.over(a, surface);
                    } else {
                        r.findings.push(f);
                        continue;
                    }
                }
                if let Some(text) = shell.color(
                    &format!("{section}.{key}"),
                    if selected {
                        *p.get("accent").unwrap_or(&fg)
                    } else {
                        fg
                    },
                    &p,
                ) {
                    f = contrast(f, text, surface, 4.5, "shell.toml", section, key);
                }
            }
            r.findings.push(f);
        }
        let mut accent = finding(
            "semantic.accent",
            "Accent indicator contrasts with the foundation",
        );
        if let Some(&a) = p.get("accent") {
            accent = contrast(accent, a, bg, 3., "colors.toml", "", "accent");
        }
        r.findings.push(accent);
        let mut error = finding(
            "semantic.error-indicator",
            "Error indicator contrasts with the foundation",
        );
        if let Some(&e) = p.get("urgent") {
            error = contrast(error, e, bg, 3., "colors.toml", "", "red");
        }
        r.findings.push(error);
        let mut select = finding(
            "semantic.selection",
            "Terminal selection differs from ordinary surface",
        );
        if let Some(&sb) = p.get("selection_background") {
            let d = sb.distance(bg);
            select = metric(
                select,
                d,
                0.05,
                0.02,
                json!({"oklabDistance":d,"required":0.05,"floor":0.02,"normalColor":bg.hex(),"stateColor":sb.hex()}),
            );
            select
                .sources
                .push(source("colors.toml", "", "selection_background"));
        }
        r.findings.push(select);
        for (id, state, alpha) in [
            ("shell.selected-fill", "selected", 0.18),
            ("shell.focus-fill", "focus", 0.08),
        ] {
            let mut f = finding(id, &format!("Shared {state} fill provides a modeled cue"));
            for key in [
                "normal-color".to_string(),
                "normal-fill-alpha".to_string(),
                format!("{state}-color"),
                format!("{state}-fill-alpha"),
            ] {
                f.sources.push(source("shell.toml", "controls", &key));
            }
            if shell
                .alpha("popups.background-alpha", 1.)
                .is_some_and(|a| a < 1.)
            {
                f.evidence = json!({"reason":"Shared controls sit on translucent popups over changing application content. The selected/focused fill cannot be compared against a fixed surface."});
                f.remediation = "Inspect selection and focus over light and dark windows; increasing [popups] background-alpha makes the shared fill more predictable.".into();
            }
            if let Some(surfaces) = stress_surfaces(&shell, &p, "popups")
                && let (Some(color), Some(a), Some(normal), Some(na)) = (
                    shell.color(&format!("controls.{state}-color"), fg, &p),
                    shell.alpha(&format!("controls.{state}-fill-alpha"), alpha),
                    shell.color("controls.normal-color", fg, &p),
                    shell.alpha("controls.normal-fill-alpha", 0.04),
                )
            {
                let mut worst = (f64::INFINITY, bg, bg);
                for &surface in &surfaces {
                    let state = color.over(a, surface);
                    let ordinary = normal.over(na, surface);
                    let d = state.distance(ordinary);
                    if d < worst.0 {
                        worst = (d, ordinary, state);
                    }
                }
                let d = worst.0;
                f.rule_version = 4;
                f.method = "modeled-appearance".into();
                f = metric(
                    f,
                    d,
                    0.02,
                    0.,
                    json!({"oklabDistance":d,"required":0.02,"normalColor":worst.1.hex(),"stateColor":worst.2.hex(),"backgroundModel":if surfaces.len()>1 {STRESS_MODEL} else {"opaque-resolved-surface"},"backgroundSamples":surfaces.len(),"scope":"sampled shared fill visibility only; weak fill is a caution, not proof that redundant labels/icons/borders fail"}),
                );
                if f.points_lost > 0. {
                    f.remediation="Increase the difference between normal and selected/focused fills if a stronger color cue is needed. Labels, icons, and borders may supply additional cues.".into();
                }
                f.sources.push(source(
                    "shell.toml",
                    "controls",
                    &format!("{state}-fill-alpha"),
                ));
            }
            r.findings.push(f);
        }
        let mut border = finding(
            "shell.popup-border",
            "Popup border separates from its surface",
        );
        let border_geometry_known = shell
            .values
            .get("popups.border-width")
            .is_none_or(|v| v.parse::<f64>().is_ok_and(|n| n > 0.))
            && !shell
                .values
                .keys()
                .any(|k| k.starts_with("popups.border-width-"));
        let surfaces = stress_surfaces(&shell, &p, "popups");
        let colors = shell
            .alpha("popups.border-alpha", 1.)
            .and_then(|opacity| border_colors(&shell, &p, opacity));
        border.evidence = json!({"reason":if !border_geometry_known {
            "Popup border width is zero or its geometry is not supported by this check."
        } else if surfaces.is_none() {
            "Popup background color/alpha could not be resolved."
        } else {
            "Popup border color/gradient or alpha could not be resolved; unsupported formats and cyclic references are not measured."
        },"borderToken":shell.values.get("popups.border")});
        border.remediation =
            "Check the reported popup border/background tokens and geometry in shell.toml.".into();
        if border_geometry_known && let (Some(surfaces), Some(colors)) = (surfaces, colors) {
            let mut pair = (f64::INFINITY, bg, bg);
            for &surface in &surfaces {
                for &c in &colors {
                    let effective = c.color.over(c.alpha, surface);
                    let value = effective.contrast(surface);
                    if value < pair.0 {
                        pair = (value, effective, surface);
                    }
                }
            }
            let worst = pair.0;
            border.rule_version = 5;
            border.method = "modeled-appearance".into();
            border = metric(
                border,
                worst,
                3.,
                1.5,
                json!({"contrast":worst,"required":3.,"foreground":pair.1.hex(),"effectiveBackground":pair.2.hex(),"backgroundModel":if surfaces.len()>1 {STRESS_MODEL} else {"opaque-resolved-surface"},"backgroundSamples":surfaces.len(),"borderSamples":colors.len(),"scope":"sampled native border colors against modeled popup interiors; no live geometry/pixel certification"}),
            );
            border
                .sources
                .push(source("shell.toml", "popups", "border"));
            if border.points_lost > 0. {
                border.remediation="Adjust popup border colors/alpha or background opacity to reach 3:1 across the reference backgrounds.".into();
            }
        }
        r.findings.push(border);
        for (id, section, selected) in [
            ("wallpaper.lock-model", "lock", false),
            ("wallpaper.launcher-model", "launcher", true),
        ] {
            let mut f = finding(
                id,
                &format!("{section} text under the fixed wallpaper stress model"),
            );
            f.method = "modeled-appearance".into();
            for key in [
                "background",
                "background-alpha",
                if selected { "selected-text" } else { "text" },
            ] {
                f.sources.push(source("shell.toml", section, key));
            }
            if selected {
                for key in [
                    "selected-background",
                    "selected-background-alpha",
                    "scrim",
                    "scrim-alpha",
                ] {
                    f.sources.push(source("shell.toml", section, key));
                }
            }
            f.rule_version = 3;
            f.evidence = json!({"reason":"Wallpaper media could not be completely sampled, or the surface colors could not be resolved."});
            if media_complete
                && !selected_missing
                && let Some(worst) = wallpaper_pair(&shell, &p, section, selected, &samples)
            {
                // Readability owns an existing defect; wallpaper retains evidence without charging twice.
                let reference = if selected {
                    "readability.launcher.selected"
                } else {
                    "readability.lock"
                };
                let already_failing = r.findings.iter().any(|x| {
                    x.rule_id == reference && x.assessed_budget > 0. && x.points_lost > 0.
                });
                f.sources.push(source(&worst.1, "", ""));
                f = metric(
                    f,
                    if already_failing { 4.5 } else { worst.0 },
                    4.5,
                    3.,
                    json!({"contrast":worst.0,"required":4.5,"background":worst.1,"effectiveBackground":worst.2.hex(),"foreground":worst.3.hex(),"sampleModel":"center-crop-160x90-grid8-video5-v2","videoSamples":media_evidence,"referenceDefectOwnedByReadability":already_failing,"scope":if selected {"launcher-over-wallpaper stress scenario, not live application-backed contrast"} else {"sampled wallpaper-backed lock"},"excludes":"blur, application content, monitor transforms"}),
                );
                if f.points_lost > 0. {
                    f.remediation = format!(
                        "Increase [{section}] background-alpha or adjust its text/background colors to reach 4.5:1 in the sampled wallpaper scenario."
                    );
                }
            }
            r.findings.push(f);
        }
    }
    for (id, _, _, _, _) in REGISTRY {
        if !r.findings.iter().any(|f| f.rule_id == id) {
            r.findings
                .push(finding(id, "Required color context unavailable"));
        }
    }
    r.findings.sort_by_key(|f| {
        REGISTRY
            .iter()
            .position(|x| x.0 == f.rule_id)
            .unwrap_or(usize::MAX)
    });
    if s.files.contains_key("hyprland.lua") {
        let mut f = finding(
            "validity.shell",
            "Lua syntax is not assessed; theme code is never executed",
        );
        f.rule_id = "advisory.lua-syntax".into();
        f.category = "validity".into();
        f.scored = false;
        f.budget = 0.;
        f.mandatory = false;
        f.blocking = false;
        f.severity = "advisory".into();
        f.evidence = json!({"reason":"No dialect-matched non-executing Lua validator is bundled; theme code is never executed."});
        f.sources = vec![source("hyprland.lua", "", "")];
        f.remediation = "Validate with a trusted non-executing validator matching the installed Lua dialect; never run theme code for an audit.".into();
        r.findings.push(f);
    }
    if !user.invalid.is_empty() {
        let mut f = finding(
            "validity.shell",
            "Your shell.toml override has invalid values",
        );
        f.rule_id = "advisory.user-shell".into();
        f.scored = false;
        f.budget = 0.;
        f.mandatory = false;
        f.blocking = false;
        f.severity = "advisory".into();
        f.evidence = json!({"invalidKeys":user.invalid,"reason":"These values in ~/.config/omarchy/shell.toml are outside their valid range. They are not counted against the theme, but checks that depend on them cannot be measured."});
        f.sources = vec![source("user.shell.toml", "", "")];
        f.remediation = "Fix or remove these keys in ~/.config/omarchy/shell.toml.".into();
        r.findings.push(f);
    }
    for f in &mut r.findings {
        if f.rule_id == "validity.palette" || f.rule_id == "validity.roles" {
            f.sources.push(source("colors.toml", "", ""));
        }
        if f.rule_id == "validity.shell" {
            f.sources.push(source(
                if s.files.contains_key("shell.toml") {
                    "shell.toml"
                } else {
                    "profile.defaults"
                },
                "",
                "",
            ));
        }
        f.sources.retain(|src| src.key != "text+surface");
        for src in &mut f.sources {
            if src.file == "shell.toml"
                && user
                    .values
                    .contains_key(&format!("{}.{}", src.section, src.key))
            {
                src.file = "user.shell.toml".into();
            } else if src.file == "shell.toml"
                && !src.key.is_empty()
                && !shell
                    .values
                    .contains_key(&format!("{}.{}", src.section, src.key))
            {
                src.file = "profile.defaults".into();
            }
        }
        if f.rule_id == "validity.shell" && s.files.contains_key("user.shell.toml") {
            f.sources.push(source("user.shell.toml", "", ""));
        }
        // No file-level pointer at a shell.toml the theme does not have; no repeated sources.
        let theme_shell = s.files.contains_key("shell.toml");
        f.sources.retain(|src| {
            theme_shell
                || !(src.file == "shell.toml" && src.section.is_empty() && src.key.is_empty())
        });
        let mut seen = std::collections::BTreeSet::new();
        f.sources
            .retain(|src| seen.insert((src.file.clone(), src.section.clone(), src.key.clone())));
        // Checks mirroring runtime behavior this analyzer has not verified are paused.
        if f.scored && !s.contract.allows(&f.rule_id) {
            *f = paused(&f.rule_id, &s.contract);
        }
    }
    r.score();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_metric_thresholds() {
        for (value, severity, fraction) in [
            (4.5, "pass", 0.),
            (4.499999, "caution", 0.5),
            (3., "caution", 0.5),
            (2.999999, "failure", 1.),
        ] {
            let f = metric(
                finding("readability.foundation", "vector"),
                value,
                4.5,
                3.,
                json!({}),
            );
            assert_eq!(f.severity, severity);
            assert_eq!(f.points_lost, f.budget * fraction);
        }
    }
    #[test]
    fn indicator_thresholds() {
        for (value, severity) in [
            (3., "pass"),
            (2.999999, "caution"),
            (1.5, "caution"),
            (1.499999, "failure"),
        ] {
            let f = metric(
                finding("semantic.accent", "vector"),
                value,
                3.,
                1.5,
                json!({}),
            );
            assert_eq!(f.severity, severity);
        }
    }
}
