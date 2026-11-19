//! The plan's artifact checks before a gate. A port of Go's
//! `internal/plan/check.go` and `internal/core/state/check.go`'s `Problem`.

use std::collections::HashMap;
use std::path::Path;

use super::SPECS_DIR;
use super::diagrams::{DIAGRAMS_SECTION, diagrams};
use super::model::{Model, load, read_if_exists};
use super::problems::problems;
use super::spec::{SPEC_SECTION, Severity, parse_spec};
use crate::state::store::shown;
use crate::ticket::{self, Ticket};
use crate::{gofmt, placeholder, repos};

/// One thing an artifact check found wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Relative to the workspace root; "" when not about one file.
    pub path: String,
    /// 1-based; 0 when not about one line.
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for Problem {
    /// `path:line: message`, leaving out the parts that are empty.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.path.is_empty(), self.line) {
            (false, n) if n > 0 => write!(f, "{}:{n}: {}", self.path, self.message),
            (false, _) => write!(f, "{}: {}", self.path, self.message),
            _ => f.write_str(&self.message),
        }
    }
}

fn problem(path: &str, message: String) -> Problem {
    Problem {
        path: path.to_string(),
        line: 0,
        message,
    }
}

/// Go's `CheckArtifacts`: the checks for a plan change's transition.
pub fn check_artifacts(
    root: &Path,
    change: &str,
    phase: &str,
    status: &str,
    adr_dir: &str,
) -> Vec<Problem> {
    match (phase, status) {
        ("spec", "approved") => check_spec(root, change),
        ("architecture", "approved") => check_architecture(root, change),
        ("breakdown", "approved") => check_breakdown(root, change, adr_dir),
        ("publish", "complete") => check_publish(root, change, adr_dir),
        _ => Vec::new(),
    }
}

fn artifact(change: &str, name: &str) -> String {
    format!("{SPECS_DIR}/{change}/{name}")
}

/// Go's `rel`: a root-joined path as relative to the root.
fn rel(root: &Path, path: &str) -> String {
    let prefix = gofmt::to_slash(&root.to_string_lossy());
    if prefix == "." {
        return path.to_string();
    }
    let prefix = format!("{}/", prefix.trim_end_matches('/'));
    path.strip_prefix(&prefix).unwrap_or(path).to_string()
}

fn cant_check(path: &str, err: &crate::Error) -> Vec<Problem> {
    vec![problem(path, format!("can't check: {err}"))]
}

fn read_artifact(root: &Path, path: &str) -> Result<String, Vec<Problem>> {
    match read_if_exists(root, path) {
        Err(e) => Err(cant_check(path, &e)),
        Ok(None) => Err(vec![problem(path, "doesn't exist".into())]),
        Ok(Some(text)) => Ok(text),
    }
}

fn check_spec(root: &Path, change: &str) -> Vec<Problem> {
    let path = artifact(change, "functional-spec.md");
    let text = match read_artifact(root, &path) {
        Ok(t) => t,
        Err(p) => return p,
    };
    let (reqs, findings) = parse_spec(&text);
    let mut out = Vec::new();
    if !reqs.iter().any(|r| r.kind == 'R') {
        out.push(problem(
            &path,
            format!("no R requirements under {SPEC_SECTION}"),
        ));
    }
    for f in findings {
        if f.severity == Severity::Error {
            out.push(problem(&path, f.message));
        }
    }
    out
}

/// A required diagram: its name in messages, and which headings match it.
type Required = (&'static str, fn(&str) -> bool);

fn is_flow(heading: &str) -> bool {
    heading
        .strip_prefix("Flow:")
        .is_some_and(|n| !n.trim().is_empty())
}

fn check_architecture(root: &Path, change: &str) -> Vec<Problem> {
    let path = artifact(change, "architecture.md");
    let text = match read_artifact(root, &path) {
        Ok(t) => t,
        Err(p) => return p,
    };
    let mut out = Vec::new();
    let (section, ds) = diagrams(&text);
    if !section {
        out.push(problem(&path, format!("no {DIAGRAMS_SECTION} section")));
    } else {
        let required: [Required; 3] = [
            ("### Context", |h| h == "Context"),
            ("### Layers", |h| h == "Layers"),
            ("### Flow: <name>", is_flow),
        ];
        for (name, matches) in &required {
            if !ds.iter().any(|d| matches(&d.heading)) {
                out.push(problem(&path, format!("{DIAGRAMS_SECTION} has no {name}")));
            }
        }
        for d in &ds {
            if d.source.is_empty() && required.iter().any(|(_, m)| m(&d.heading)) {
                out.push(Problem {
                    path: path.clone(),
                    line: d.line,
                    message: format!("### {} needs a non-empty mermaid block", d.heading),
                });
            }
        }
    }
    for m in placeholder::find(&text) {
        out.push(Problem {
            path: path.clone(),
            line: m.line,
            message: format!("unfilled template placeholder {}", m.text),
        });
    }
    out
}

/// Go's `changeTickets`: the model, and the change's own tickets and unparsable
/// ticket files.
fn change_tickets(
    root: &Path,
    change: &str,
    adr_dir: &str,
) -> Result<(Model, Vec<usize>, Vec<usize>), crate::Error> {
    let m = load(root, change, adr_dir)?;
    let dir = shown(root, &artifact(change, "tickets"));
    let parent = |p: &str| {
        p.rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default()
    };
    let mine = (0..m.tickets.len())
        .filter(|&i| parent(&m.tickets[i].path) == dir)
        .collect();
    let bad = (0..m.ticket_problems.len())
        .filter(|&i| parent(&m.ticket_problems[i].id) == dir)
        .collect();
    Ok((m, mine, bad))
}

fn check_breakdown(root: &Path, change: &str, adr_dir: &str) -> Vec<Problem> {
    let folder = artifact(change, "tickets");
    let (m, mine, bad) = match change_tickets(root, change, adr_dir) {
        Ok(x) => x,
        Err(e) => return cant_check(&folder, &e),
    };
    let reg = match repos::load(root) {
        Ok(r) => r,
        Err(e) => return cant_check("", &e),
    };
    let mut out: Vec<Problem> = bad
        .iter()
        .map(|&i| {
            let p = &m.ticket_problems[i];
            problem(&rel(root, &p.id), p.message.clone())
        })
        .collect();
    if mine.is_empty() && bad.is_empty() {
        return vec![problem(&folder, "no tickets".into())];
    }
    // The change's tickets by path and by id.
    let mut by_key: HashMap<&str, usize> = HashMap::new();
    for &i in &mine {
        let t = &m.tickets[i];
        by_key.insert(&t.path, i);
        if !t.id.is_empty() {
            by_key.insert(&t.id, i);
        }
    }
    // A ready ticket of this change is checked as a draft: readiness is checked
    // when it's readied, and breakdown comes first.
    let drafts: Vec<Ticket> = m
        .tickets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut t = t.clone();
            if mine.contains(&i) && t.status == ticket::STATUS_READY {
                t.status = ticket::STATUS_DRAFT.into();
            }
            t
        })
        .collect();
    let refs: Vec<&Ticket> = drafts.iter().collect();
    for p in ticket::validate_workspace(&refs, &reg.names()) {
        let Some(&i) = by_key.get(p.id.as_str()) else {
            continue;
        };
        let t = &m.tickets[i];
        let msg = if p.id == t.id {
            format!("{}: {}", t.id, p.message)
        } else {
            p.message
        };
        out.push(problem(&rel(root, &t.path), msg));
    }
    for f in problems(&m) {
        if f.change != change || f.severity != Severity::Error {
            continue;
        }
        let path = match by_key.get(f.id.as_str()) {
            Some(&i) => rel(root, &m.tickets[i].path),
            None => artifact(change, "functional-spec.md"),
        };
        out.push(problem(&path, format!("{}: {}", f.id, f.message)));
    }
    out
}

fn check_publish(root: &Path, change: &str, adr_dir: &str) -> Vec<Problem> {
    let (m, mine, _) = match change_tickets(root, change, adr_dir) {
        Ok(x) => x,
        Err(e) => return cant_check(&artifact(change, "tickets"), &e),
    };
    mine.iter()
        .map(|&i| &m.tickets[i])
        .filter(|t| {
            (t.status == ticket::STATUS_READY || t.status == ticket::STATUS_PUBLISHED)
                && t.published_hash.is_empty()
        })
        .map(|t| {
            problem(
                &rel(root, &t.path),
                format!("{}: {} but not frozen (no published_hash)", t.id, t.status),
            )
        })
        .collect()
}
