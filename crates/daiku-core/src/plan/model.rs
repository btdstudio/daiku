//! The plan model: a change's spec, architecture diagrams, and the workspace's
//! tickets and ADRs. A port of Go's `internal/plan/plan.go`.

use std::collections::BTreeMap;
use std::path::Path;

use super::SPECS_DIR;
use super::adr::{self, Record};
use super::diagrams::{Diagram, diagrams};
use super::spec::{Finding, Requirement, parse_spec};
use crate::state::CHANGE_ID_RE;
use crate::state::store::{at, read_file, shown};
use crate::ticket::{self, Ticket};
use crate::{Error, markdown};

/// One numbered change's spec.
#[derive(Debug, Clone, Default)]
pub struct ChangeSpec {
    pub name: String,
    pub has_spec: bool,
    /// The H1 after `— `, or "" without an H1.
    pub title: String,
    pub requirements: Vec<Requirement>,
    pub findings: Vec<Finding>,
}

/// What a plan holds.
#[derive(Debug, Clone, Default)]
pub struct Model {
    pub change: String,
    /// The spec's H1 after `— `, or the change name.
    pub title: String,
    pub has_spec: bool,
    pub requirements: Vec<Requirement>,
    pub spec_findings: Vec<Finding>,
    pub has_architecture: bool,
    pub diagram_section: bool,
    pub diagrams: Vec<Diagram>,
    /// The whole workspace's tickets, sorted by id.
    pub tickets: Vec<Ticket>,
    /// Ticket files that couldn't be parsed.
    pub ticket_problems: Vec<ticket::Problem>,
    /// `ADR-0001` → record.
    pub adrs: BTreeMap<String, Record>,
    /// Every numbered change's spec, by number.
    pub changes: Vec<ChangeSpec>,
}

/// Go's `readIfExists`: the file's text, `None` when it doesn't exist.
pub(crate) fn read_if_exists(root: &Path, rel: &str) -> Result<Option<String>, Error> {
    match read_file(&at(root, rel)) {
        Ok(d) => Ok(d.map(|d| String::from_utf8_lossy(&d).into_owned())),
        Err(text) => Err(Error::PathError { text }),
    }
}

/// Go's `title`: the first prose H1, after `— ` if it has one.
pub fn title(text: &str) -> String {
    for l in markdown::scan(text) {
        if !l.prose() || !l.text.starts_with("# ") {
            continue;
        }
        let h = l.text[2..].trim();
        return match h.split_once("— ") {
            Some((_, after)) => after.trim().to_string(),
            None => h.to_string(),
        };
    }
    String::new()
}

fn load_spec(root: &Path, change: &str) -> Result<ChangeSpec, Error> {
    let mut cs = ChangeSpec {
        name: change.to_string(),
        ..ChangeSpec::default()
    };
    if let Some(text) = read_if_exists(root, &format!("{SPECS_DIR}/{change}/functional-spec.md"))? {
        cs.has_spec = true;
        cs.title = title(&text);
        (cs.requirements, cs.findings) = parse_spec(&text);
    }
    Ok(cs)
}

/// Go's `changeNames`: the numbered change folders, by number then name.
fn change_names(root: &Path) -> Result<Vec<String>, Error> {
    let entries = match std::fs::read_dir(at(root, SPECS_DIR)) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::Io {
                op: "open",
                path: shown(root, SPECS_DIR),
                source,
            });
        }
    };
    let mut changes: Vec<(u64, String)> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let id = CHANGE_ID_RE.captures(&name)?[1].parse().unwrap_or(0);
            Some((id, name))
        })
        .collect();
    changes.sort();
    Ok(changes.into_iter().map(|(_, n)| n).collect())
}

/// The base name of the folder two levels up: a ticket's change (Go's
/// `TicketChange`).
pub fn ticket_change(t: &Ticket) -> &str {
    let mut parts = t.path.rsplit('/');
    parts.next();
    parts.next();
    parts.next().unwrap_or("")
}

/// Go's `LoadAll`: every change's spec and tickets, and the ADRs.
pub fn load_all(root: &Path, adr_dir: &str) -> Result<Model, Error> {
    let mut m = Model::default();
    for name in change_names(root)? {
        m.changes.push(load_spec(root, &name)?);
        let rel = format!("{SPECS_DIR}/{name}/tickets");
        let (ts, bad) = ticket::load_dir(&at(root, &rel), &shown(root, &rel))?;
        m.tickets.extend(ts);
        m.ticket_problems.extend(bad);
    }
    m.tickets.sort_by(|a, b| a.id.cmp(&b.id));
    for r in adr::list(root, adr_dir) {
        let key = format!("ADR-{:04}", r.number);
        if m.adrs.get(&key).is_some_and(|prev| prev.path < r.path) {
            continue;
        }
        m.adrs.insert(key, r);
    }
    Ok(m)
}

/// Go's `Load`: [`load_all`] plus the change's spec and architecture.
pub fn load(root: &Path, change: &str, adr_dir: &str) -> Result<Model, Error> {
    let mut m = load_all(root, adr_dir)?;
    m.change = change.to_string();
    m.title = change.to_string();
    let cs = match m.changes.iter().find(|c| c.name == change) {
        Some(cs) => cs.clone(),
        None => load_spec(root, change)?,
    };
    if cs.has_spec {
        m.has_spec = true;
        m.requirements = cs.requirements;
        m.spec_findings = cs.findings;
        if !cs.title.is_empty() {
            m.title = cs.title;
        }
    }
    if let Some(arch) = read_if_exists(root, &format!("{SPECS_DIR}/{change}/architecture.md"))? {
        m.has_architecture = true;
        (m.diagram_section, m.diagrams) = diagrams(&arch);
    }
    Ok(m)
}
