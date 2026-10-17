//! Runtime contract verification.
//!
//! The analyzer mirrors a handful of Omarchy runtime behaviors: how `colors.toml`
//! roles load, which surface/control defaults apply when a key is absent, and how
//! terminal selection colors fall back. Whole-file hashes broke on every unrelated
//! runtime edit, so each behavior is verified separately from a normalized extract
//! of only the code and defaults it depends on. Checks whose behavior cannot be
//! verified stay unassessed; the rest still run.
use crate::inputs::hash;
use serde_json::{Value, json};
use std::collections::BTreeMap;

// Exact reviewed file tuples (color, style, template, palette fallbacks).
const ORIGINAL_TEMPLATE: &str = "bdc8e76a5a9a700adaca0d2fa44a2a340584892fd507e3fcbd4f9a4843765242";
const PALETTE_FALLBACKS: &str = "a429ed1b18114ff3e784fc4479c6b829e9effb64c0f9d22d3b15eda4c62d86b5";
pub(crate) const REVIEWED: &[(&str, &str, &str, &str)] = &[
    (
        "b7b0f7df6e820851a50ddc641e1a87e33b8a6adbc14149fddf2d1e4f879dd163",
        "3d09947d917633b167f08d250696302e126e608c47991c604dca969d692f6221",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "cc9b62e2eaf8c307aa02206d9320c3d0ca349cf8feed452a6eddf387adaa5320",
        "f5901092617ae62645597199270afd4dfb0644d596bdfff16e1834fca6ad7243",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "b7b0f7df6e820851a50ddc641e1a87e33b8a6adbc14149fddf2d1e4f879dd163",
        "0b2aec0b87ee0c895cedd93c4357b46c1009b7302ddc75aa6d99ad5b259e1b02",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "b7b0f7df6e820851a50ddc641e1a87e33b8a6adbc14149fddf2d1e4f879dd163",
        "d0a7ab5222931313b4b3831d3cc3a1d5c6c6ad9e8fcbb41b96e8af0402c05a92",
        ORIGINAL_TEMPLATE,
        PALETTE_FALLBACKS,
    ),
    (
        "08af401286831759b63279cf6d6732d666256c751864a03bd5594ea29a29f0e5",
        "f84c801f2457a93130c99cb284de2615baafea6bd15de47cf190f0b11e5598a0",
        "38461c50c432b7332a3f59673536e8a784745bf678c58dfa3dcae7e4a91196b3",
        PALETTE_FALLBACKS,
    ),
];

// Extract hashes from the reviewed runtimes above (Omarchy eeff52bc and r6866/77bfaca).
const KNOWN_PALETTE_LOADING: &[&str] =
    &["87a1b8b9f8a940e04b57b09ea0b23d234adbb43d96895b4ae6b8fab427f26b6d"];
const KNOWN_SURFACE_DEFAULTS: &[&str] = &[
    "4b696117f768e417ace633004267f7df0b215e4d5448f442130aee7b213de954",
    "490c7a43c68ad2b01063efdd29f52769a4d4715adc6440bcfe0f1038659bce5c",
];
const KNOWN_CONTROL_STATES: &[&str] = &[
    "4e628446872c318b82c2d68da128ad287d77c50d7b12df5492c0f4fc71880406",
    "39ed9f7a79589f8dc8e1ef964ba3af4c14de0c9b0ee6cc39260d872a7fdb25c1",
];
const KNOWN_TEMPLATE_DEFAULTS: &[&str] = &[
    "f83d9515c72a7b62a75db355119442a2ffce34db667fc1b5645e9e43ffe05f28",
    "766cf5c2de9076ff67653cb3a0b787f138f7694fbd1ffae1980930d51ef077b7",
];
const KNOWN_SELECTION_FALLBACKS: &[&str] =
    &["8a560bf5171211b9de07951e689cb349a50bb925aa502128ee12739069c9b17c"];

type Extract = fn(&str) -> String;

/// Which mirrored runtime behaviors are verified for this audit.
#[derive(Clone, Debug, PartialEq)]
pub struct Contract {
    /// `reviewed` (exact files), `compatible` (all relevant extracts known),
    /// `partial`, `unsupported`, or `profile` (explicit path-audit profile).
    pub status: &'static str,
    pub palette: bool,
    pub selection: bool,
    pub shell: bool,
    /// Human-readable runtime pieces that changed in an unreviewed way.
    pub unverified: Vec<String>,
}

/// Behavior group each scored rule depends on.
pub fn group(rule_id: &str) -> &'static str {
    match rule_id {
        "validity.palette"
        | "validity.roles"
        | "validity.backgrounds"
        | "readability.foundation"
        | "semantic.accent"
        | "semantic.error-indicator" => "palette",
        "readability.selection" | "semantic.selection" => "selection",
        _ => "shell",
    }
}

impl Contract {
    pub fn profile() -> Self {
        Self {
            status: "profile",
            palette: true,
            selection: true,
            shell: true,
            unverified: vec![],
        }
    }
    pub fn allows(&self, rule_id: &str) -> bool {
        match group(rule_id) {
            "palette" => self.palette,
            "selection" => self.selection,
            _ => self.shell,
        }
    }
    pub fn to_json(&self) -> Value {
        json!({"status":self.status,"palette":self.palette,"selection":self.selection,"shell":self.shell,"unverified":self.unverified})
    }
    /// Evaluate the runtime files captured under their snapshot roles.
    pub fn evaluate(files: &BTreeMap<String, Vec<u8>>) -> Self {
        let text = |role: &str| {
            files
                .get(role)
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
        };
        let digest = |role: &str| files.get(role).map(|v| hash(v)).unwrap_or_default();
        let tuple = (
            digest("runtime.color"),
            digest("runtime.style"),
            digest("runtime.template"),
            digest("runtime.palette-fallbacks"),
        );
        if REVIEWED.contains(&(
            tuple.0.as_str(),
            tuple.1.as_str(),
            tuple.2.as_str(),
            tuple.3.as_str(),
        )) {
            return Self {
                status: "reviewed",
                ..Self::profile()
            };
        }
        let mut unverified = Vec::new();
        let mut check = |role: &str, label: &str, extract: Extract, known: &[&str]| {
            let ok =
                text(role).is_some_and(|t| known.contains(&hash(extract(&t).as_bytes()).as_str()));
            if !ok {
                unverified.push(label.to_string());
            }
            ok
        };
        let loading = check(
            "runtime.color",
            "Color.qml palette loading",
            palette_loading,
            KNOWN_PALETTE_LOADING,
        );
        let surfaces = check(
            "runtime.color",
            "Color.qml surface defaults",
            surface_defaults,
            KNOWN_SURFACE_DEFAULTS,
        );
        let controls = check(
            "runtime.style",
            "Style.qml control states",
            control_states,
            KNOWN_CONTROL_STATES,
        );
        let template = check(
            "runtime.template",
            "shell.toml.tpl defaults",
            template_defaults,
            KNOWN_TEMPLATE_DEFAULTS,
        );
        let fallbacks = check(
            "runtime.palette-fallbacks",
            "omarchy-theme-color selection fallbacks",
            selection_fallbacks,
            KNOWN_SELECTION_FALLBACKS,
        );
        let palette = loading;
        let selection = palette && fallbacks;
        let shell = palette && surfaces && controls && template;
        Self {
            status: if palette && selection && shell {
                "compatible"
            } else if palette {
                "partial"
            } else {
                "unsupported"
            },
            palette,
            selection,
            shell,
            unverified,
        }
    }
}

/// Extract hashes for each behavior, for reviewing a new runtime (`theme-report runtime`).
pub fn extract_hashes(files: &BTreeMap<String, Vec<u8>>) -> Vec<(&'static str, String)> {
    let text = |role: &str| {
        files
            .get(role)
            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
    };
    let parts: [(&str, &str, Extract); 5] = [
        ("palette-loading", "runtime.color", palette_loading),
        ("surface-defaults", "runtime.color", surface_defaults),
        ("control-states", "runtime.style", control_states),
        ("template-defaults", "runtime.template", template_defaults),
        (
            "selection-fallbacks",
            "runtime.palette-fallbacks",
            selection_fallbacks,
        ),
    ];
    parts
        .into_iter()
        .map(|(name, role, extract)| {
            (
                name,
                text(role)
                    .map(|t| hash(extract(&t).as_bytes()))
                    .unwrap_or_else(|| "missing".into()),
            )
        })
        .collect()
}

fn normalize(line: &str) -> Option<String> {
    let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
    (!line.is_empty() && !line.starts_with("//")).then_some(line)
}

/// Whole named function bodies (brace-counted) plus lines containing a marker.
fn qml_extract(text: &str, keep: impl Fn(&str) -> bool, markers: &[&str]) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut kept = vec![false; lines.len()];
    let mut i = 0;
    while i < lines.len() {
        let name = lines[i]
            .trim_start()
            .strip_prefix("function ")
            .and_then(|rest| rest.split('(').next())
            .map(str::trim);
        if name.is_some_and(&keep) {
            let (mut depth, mut opened, mut j) = (0i32, false, i);
            loop {
                for c in lines[j].chars() {
                    match c {
                        '{' => {
                            depth += 1;
                            opened = true
                        }
                        '}' => depth -= 1,
                        _ => {}
                    }
                }
                kept[j] = true;
                if (opened && depth <= 0) || j + 1 >= lines.len() {
                    break;
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        if markers.iter().any(|m| lines[i].contains(m)) {
            kept[i] = true;
        }
        i += 1;
    }
    lines
        .iter()
        .zip(kept)
        .filter(|(_, k)| *k)
        .filter_map(|(line, _)| normalize(line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn palette_loading(text: &str) -> String {
    qml_extract(text, |name| name == "loadColors", &[])
}
fn surface_defaults(text: &str) -> String {
    qml_extract(
        text,
        |name| matches!(name, "pick" | "pickAlpha" | "flatColor" | "composed"),
        &["root.composed(", "root.pick("],
    )
}
fn control_states(text: &str) -> String {
    qml_extract(
        text,
        |name| {
            matches!(
                name,
                "styleRawNum" | "styleNum" | "styleAlpha" | "styleString" | "colorFromHex"
            ) || name.ends_with("StateColor")
                || name.ends_with("FillFor")
        },
        &["styleAlpha(\"", "styleString(\"", "styleNum(\""],
    )
}
fn template_defaults(text: &str) -> String {
    // Literal template values are the defaults materialized themes start from.
    crate::rules::Shell::parse(text)
        .values
        .into_iter()
        .filter(|(_, value)| !value.contains("{{"))
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
}
fn selection_fallbacks(text: &str) -> String {
    text.lines()
        .filter(|line| line.contains("]] ||"))
        .filter_map(normalize)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn files(color: &str, style: &str, template: &str, palette: &str) -> BTreeMap<String, Vec<u8>> {
        [
            ("runtime.color", color),
            ("runtime.style", style),
            ("runtime.template", template),
            ("runtime.palette-fallbacks", palette),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
        .collect()
    }
    #[test]
    fn unknown_runtime_verifies_nothing() {
        let c = Contract::evaluate(&files("a", "b", "c", "d"));
        assert_eq!(c.status, "unsupported");
        assert!(!c.palette && !c.selection && !c.shell);
        assert_eq!(c.unverified.len(), 5);
    }
    #[test]
    fn extracts_ignore_unrelated_edits_but_not_relevant_ones() {
        let style = "  function styleAlpha(key, fallback) {\n    return clamp(key)\n  }\n  readonly property real normalFillAlpha: styleAlpha(\"normal-fill-alpha\", 0.04)\n  readonly property int iconFont: root.barToken(\"icon-font\", 13)\n";
        let edited = style.replace("13)", "14)\n  readonly property int iconSize: 12");
        assert_eq!(control_states(style), control_states(&edited));
        let changed = style.replace("0.04", "0.05");
        assert_ne!(control_states(style), control_states(&changed));
        let body = style.replace("clamp(key)", "clamp(key) * 2");
        assert_ne!(control_states(style), control_states(&body));
    }
    #[test]
    fn reviewed_tuples_are_complete_generations() {
        // Mixing files from different reviewed generations is not itself a reviewed runtime.
        assert!(!REVIEWED.contains(&(
            REVIEWED[0].0,
            REVIEWED[1].1,
            ORIGINAL_TEMPLATE,
            PALETTE_FALLBACKS
        )));
        assert!(REVIEWED.iter().all(|t| t.3 == PALETTE_FALLBACKS));
    }
    #[test]
    fn rule_groups_cover_registry() {
        for (id, ..) in crate::rules::REGISTRY {
            assert!(matches!(group(id), "palette" | "selection" | "shell"));
        }
        assert_eq!(group("validity.palette"), "palette");
        assert_eq!(group("readability.bar"), "shell");
    }
}
