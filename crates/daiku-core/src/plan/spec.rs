//! Spec parsing: the requirements in a functional spec. A port of Go's
//! `internal/plan/spec.go`.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use crate::{markdown, placeholder};

/// One requirement line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Requirement {
    /// `R3`.
    pub id: String,
    /// `R` or `N`.
    pub kind: char,
    /// `must`, `should`, `could`, or "" when missing.
    pub priority: String,
    /// 1-based.
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        })
    }
}

/// Something wrong in a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub id: String,
    pub message: String,
    /// The change it's about; [`parse_spec`] leaves it "".
    pub change: String,
}

pub const SPEC_SECTION: &str = "## Functional requirements";

static REQUIREMENT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[\t\n\x0C\r ]*[-*][\t\n\x0C\r ]+([RN])([0-9]+)\.[\t\n\x0C\r ]")
        .expect("valid pattern")
});
static PRIORITY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"— \*\*(must|should|could)\*\*").expect("valid pattern"));

/// Go's `ParseSpec`: the requirements under `## Functional requirements`, with
/// duplicates (errors) and missing priorities (warnings). Lines in fences,
/// comments or holding a placeholder don't count.
pub fn parse_spec(markdown: &str) -> (Vec<Requirement>, Vec<Finding>) {
    let placeholders: HashSet<usize> = placeholder::find(markdown)
        .into_iter()
        .map(|m| m.line)
        .collect();
    let (mut reqs, mut findings) = (Vec::new(), Vec::new());
    let mut first: HashMap<String, usize> = HashMap::new();
    let lines = markdown::scan(markdown);
    for l in markdown::section_lines(&lines, SPEC_SECTION) {
        if !l.prose() || placeholders.contains(&l.n) {
            continue;
        }
        let Some(m) = REQUIREMENT_RE.captures(&l.text) else {
            continue;
        };
        let id = format!("{}{}", &m[1], &m[2]);
        if let Some(prev) = first.get(&id) {
            findings.push(Finding {
                severity: Severity::Error,
                message: format!("line {}: {id} is already defined on line {prev}", l.n),
                id,
                change: String::new(),
            });
            continue;
        }
        first.insert(id.clone(), l.n);
        let mut r = Requirement {
            id: id.clone(),
            kind: m[1].chars().next().unwrap_or('R'),
            priority: String::new(),
            line: l.n,
        };
        match PRIORITY_RE.captures(&l.text) {
            Some(p) => r.priority = p[1].to_string(),
            None => findings.push(Finding {
                severity: Severity::Warning,
                message: format!(
                    "line {}: {id} has no priority (expected — **must**, — **should** or — **could**)",
                    l.n
                ),
                id,
                change: String::new(),
            }),
        }
        reqs.push(r);
    }
    (reqs, findings)
}
