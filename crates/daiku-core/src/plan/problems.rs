//! Cross-checks between a plan's specs, tickets and ADRs. A port of Go's
//! `internal/plan/problems.go`.

use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

use regex::Regex;

use super::model::{Model, ticket_change};
use super::spec::{Finding, Severity};
use crate::ticket::{self, Ticket};

static ADR_CITATION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^ADR-([0-9]{4})$").expect("valid pattern"));

/// Go's `Problems`: per change, in order, its spec findings, tickets citing
/// requirements that don't exist, ready stories and tasks with none, must
/// requirements no ticket cites, and ADR citations that don't resolve.
pub fn problems(m: &Model) -> Vec<Finding> {
    let mut by_change: BTreeMap<&str, Vec<&Ticket>> = BTreeMap::new();
    for t in &m.tickets {
        by_change.entry(ticket_change(t)).or_default().push(t);
    }
    let mut out = Vec::new();
    for cs in &m.changes {
        let mut tickets = by_change.get(cs.name.as_str()).cloned().unwrap_or_default();
        tickets.sort_by(|a, b| a.id.cmp(&b.id));
        let mut add = |severity, id: &str, message: String| {
            out.push(Finding {
                severity,
                id: id.to_string(),
                message,
                change: cs.name.clone(),
            })
        };
        for f in &cs.findings {
            add(
                f.severity,
                &f.id,
                format!("spec of {}: {}", cs.name, f.message),
            );
        }
        let defined: HashSet<&str> = cs.requirements.iter().map(|r| r.id.as_str()).collect();
        let mut cited: HashSet<&str> = HashSet::new();
        for t in &tickets {
            for id in &t.requirements {
                cited.insert(id);
                if !cs.has_spec {
                    add(
                        Severity::Error,
                        &t.id,
                        format!("cites {id}, but {} has no spec", cs.name),
                    );
                } else if !defined.contains(id.as_str()) {
                    add(
                        Severity::Error,
                        &t.id,
                        format!("cites {id}, which isn't in {}'s spec", cs.name),
                    );
                }
            }
            if t.status == ticket::STATUS_READY
                && (t.typ == ticket::STORY || t.typ == ticket::TASK)
                && t.requirements.is_empty()
            {
                add(
                    Severity::Warning,
                    &t.id,
                    format!("ready {} with no requirements", t.typ),
                );
            }
        }
        if !tickets.is_empty() {
            for r in &cs.requirements {
                if r.kind == 'R' && r.priority == "must" && !cited.contains(r.id.as_str()) {
                    add(
                        Severity::Warning,
                        &r.id,
                        format!("must requirement in {} isn't cited by any ticket", cs.name),
                    );
                }
            }
        }
        for t in &tickets {
            for a in &t.adrs {
                match ADR_CITATION_RE.captures(a) {
                    None => add(
                        Severity::Warning,
                        &t.id,
                        format!(
                            "adrs entry {} isn't of the form ADR-<nnnn>",
                            crate::gofmt::quote(a)
                        ),
                    ),
                    Some(mm) if !m.adrs.contains_key(a) => add(
                        Severity::Warning,
                        &t.id,
                        format!("cites {a}, but there's no {}-*.md ADR file", &mm[1]),
                    ),
                    Some(_) => {}
                }
            }
        }
    }
    out
}
