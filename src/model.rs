use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub rule_id: String,
    pub rule_version: u32,
    pub category: String,
    pub severity: String,
    pub assessment_status: String,
    pub method: String,
    pub scored: bool,
    pub policy_suppressed: bool,
    pub budget: f64,
    pub assessed_budget: f64,
    pub points_lost: f64,
    pub blocking: bool,
    pub mandatory: bool,
    pub summary: String,
    pub evidence: Value,
    pub sources: Vec<Source>,
    pub remediation: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Source {
    pub file: String,
    pub section: String,
    pub key: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: String,
    pub weight: f64,
    pub expected: f64,
    pub assessed: f64,
    pub lost: f64,
    pub score: Option<f64>,
    pub grade: Option<String>,
    pub state: String,
    pub summary: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub schema_version: u32,
    pub analyzer_version: String,
    pub ruleset_version: String,
    pub profile: String,
    pub omarchy_version: Option<String>,
    pub target: String,
    pub theme: String,
    pub theme_path: String,
    pub input_fingerprint: String,
    pub state: String,
    pub score: Option<f64>,
    pub grade: Option<String>,
    pub summary: String,
    pub cap_reasons: Vec<String>,
    pub categories: Vec<Category>,
    pub findings: Vec<Finding>,
    pub assumptions: Vec<String>,
    pub audit_timestamp: u64,
    pub watched_paths: Vec<String>,
    /// Which mirrored Omarchy runtime behaviors were verified (current audits).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_contract: Option<Value>,
}
pub const CATEGORIES: [(&str, f64); 5] = [
    ("validity", 20.),
    ("readability", 30.),
    ("semantic", 20.),
    ("shell", 20.),
    ("wallpaper", 10.),
];
fn grade(score: f64, capped: bool) -> String {
    let letter = if score >= 90. {
        "A"
    } else if score >= 80. {
        "B"
    } else if score >= 70. {
        "C"
    } else if score >= 60. {
        "D"
    } else {
        "F"
    };
    if capped && score >= 70. {
        "D".into()
    } else {
        letter.into()
    }
}
fn count(n: usize, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}
fn issues(problems: usize, cautions: usize) -> String {
    [(problems, "problem"), (cautions, "caution")]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, noun)| count(n, noun))
        .collect::<Vec<_>>()
        .join(" · ")
}
impl Report {
    pub fn score(&mut self) {
        self.cap_reasons = self
            .findings
            .iter()
            .filter(|f| f.scored && f.blocking && f.severity == "failure" && !f.policy_suppressed)
            .map(|f| f.rule_id.clone())
            .collect();
        self.categories = CATEGORIES
            .iter()
            .map(|(id, weight)| {
                let rules: Vec<_> = self
                    .findings
                    .iter()
                    .filter(|f| {
                        f.scored && f.category == *id && f.assessment_status != "not-applicable"
                    })
                    .collect();
                let expected: f64 = rules.iter().map(|f| f.budget).sum();
                let assessed: f64 = rules
                    .iter()
                    .filter(|f| !f.policy_suppressed)
                    .map(|f| f.assessed_budget)
                    .sum();
                let lost: f64 = rules
                    .iter()
                    .filter(|f| !f.policy_suppressed)
                    .map(|f| f.points_lost)
                    .sum();
                let problems = rules
                    .iter()
                    .filter(|f| !f.policy_suppressed && f.severity == "failure")
                    .count();
                let cautions = rules
                    .iter()
                    .filter(|f| !f.policy_suppressed && f.severity == "caution")
                    .count();
                let limitations = rules
                    .iter()
                    .filter(|f| f.policy_suppressed || f.assessed_budget < f.budget)
                    .count();
                let state = if expected == 0. {
                    "not-applicable"
                } else if assessed == 0. {
                    "not-checked"
                } else if limitations > 0 {
                    "limited"
                } else {
                    "ready"
                };
                let score = ratio(assessed - lost, assessed);
                let capped = rules
                    .iter()
                    .any(|f| f.blocking && f.severity == "failure" && !f.policy_suppressed);
                let summary = if assessed == 0. {
                    if expected == 0. {
                        "No applicable checks".into()
                    } else {
                        "No supported measurements".into()
                    }
                } else if problems + cautions > 0 {
                    issues(problems, cautions)
                } else {
                    "No problems found".into()
                };
                Category {
                    id: (*id).into(),
                    weight: *weight,
                    expected,
                    assessed,
                    lost,
                    score,
                    grade: score.map(|s| grade(s, capped)),
                    state: state.into(),
                    summary,
                }
            })
            .collect();
        let assessed = self.categories.iter().map(|c| c.assessed).sum();
        let lost: f64 = self.categories.iter().map(|c| c.lost).sum();
        self.score = ratio(assessed - lost, assessed);
        self.grade = self.score.map(|s| grade(s, !self.cap_reasons.is_empty()));
        let limited = self
            .categories
            .iter()
            .any(|c| matches!(c.state.as_str(), "limited" | "not-checked"));
        self.state = if self.grade.is_none() {
            "not-graded"
        } else if limited {
            "provisional"
        } else {
            "ready"
        }
        .into();
        let problems = self
            .findings
            .iter()
            .filter(|f| !f.policy_suppressed && f.severity == "failure")
            .count();
        let cautions = self
            .findings
            .iter()
            .filter(|f| !f.policy_suppressed && f.severity == "caution")
            .count();
        self.summary = if self.grade.is_none() {
            "No supported checks could be completed.".into()
        } else if !self.cap_reasons.is_empty() {
            "Broken foundational inputs need fixing first.".into()
        } else if problems + cautions > 0 {
            format!("{} to review.", issues(problems, cautions))
        } else if limited {
            "The checks performed look good; some contexts could not be checked.".into()
        } else {
            "All checked areas look healthy.".into()
        };
    }
}
fn ratio(n: f64, d: f64) -> Option<f64> {
    if d > 0. { Some(100. * n / d) } else { None }
}
