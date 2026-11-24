//! What is waiting on the person (S-0015): gates to approve, phases to start and
//! tickets ready to build, in the order to show them. The rule lives here so the
//! CLI, agents and the app agree. It writes nothing and reads no clock.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::Serialize;

use crate::build::planned_id;
use crate::outcome::{Outcome, Warning};
use crate::repos;
use crate::state::machine::{APPROVED, AWAIT_APPROVAL, COMPLETE, DRAFT, IN_PROGRESS, SKIPPED};
use crate::state::{BUILD_LOOP, Change, Store};
use crate::text::Text;
use crate::ticket::{self, STATUS_CLOSED, STATUS_PUBLISHED, STATUS_READY, Ticket};
use crate::tickets::Gathered;
use crate::{Error, Workspace};

/// One thing waiting on the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum NextItem {
    /// A gate at await-approval.
    Approve {
        change: String,
        r#loop: String,
        phase: String,
        active: bool,
        since: Option<String>,
    },
    /// The change's next phase hasn't started.
    Start {
        change: String,
        r#loop: String,
        phase: String,
        active: bool,
        since: Option<String>,
    },
    /// A ticket ready to build.
    Build {
        ticket: String,
        title: Text,
        repo: String,
        change: String,
        since: Option<String>,
    },
}

/// What is waiting, first to last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NextList {
    pub items: Vec<NextItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Approve,
    Start,
}

/// The first phase of an open change that needs the person, if any: an
/// await-approval gate to approve, or a phase that hasn't started. A draft or
/// in-progress phase, any other status, or running off the end gives `None`.
fn first_open(c: &Change) -> Option<(Kind, &'static str)> {
    for phase in c.machine().names() {
        match c.gate(phase) {
            APPROVED | SKIPPED | COMPLETE => continue,
            AWAIT_APPROVAL => return Some((Kind::Approve, phase)),
            "" => return Some((Kind::Start, phase)),
            DRAFT | IN_PROGRESS => return None,
            _ => return None,
        }
    }
    None
}

/// `Some` before `None`, then ascending by bytes.
fn since_order(a: &Option<String>, b: &Option<String>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.as_bytes().cmp(b.as_bytes()),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn stamp(c: &Change) -> Option<String> {
    (!c.updated_at.is_empty()).then(|| c.updated_at.clone())
}

/// Whether dependency `id` is done: its build shipped, or a ticket with that id is
/// closed. An unknown id isn't.
fn done(id: &str, changes: &BTreeMap<String, Change>, gathered: &[Gathered]) -> bool {
    changes
        .get(id)
        .is_some_and(|c| c.loop_ == BUILD_LOOP && c.gate("ship") == COMPLETE)
        || gathered
            .iter()
            .any(|g| g.ticket.id == id && g.ticket.status == STATUS_CLOSED)
}

/// A ticket's status lets it be built.
fn buildable_status(t: &Ticket) -> bool {
    if planned_id(&t.id) {
        t.status == STATUS_READY || t.status == STATUS_PUBLISHED
    } else {
        t.status != STATUS_CLOSED
    }
}

/// The latest `updated_at` among the dependencies' builds, else the plan's, else
/// none.
fn build_since(g: &Gathered, changes: &BTreeMap<String, Change>) -> Option<String> {
    g.ticket
        .depends_on
        .iter()
        .filter_map(|d| changes.get(d))
        .filter(|c| !c.updated_at.is_empty())
        .map(|c| c.updated_at.clone())
        .max_by(|a, b| a.as_bytes().cmp(b.as_bytes()))
        .or_else(|| changes.get(&g.change).and_then(stamp))
}

impl Workspace {
    /// What is waiting on the person: gates to approve, phases to start, and
    /// tickets ready to build. It writes nothing and fails only on an IO or state
    /// error; a bad ticket file or registry is a warning.
    pub fn next(&self) -> Result<Outcome<NextList>, Error> {
        let store = Store::reader(self.root(), self.env());
        let changes = store.read_changes()?;
        let active = store.read_active()?;
        let active = changes
            .get(&active)
            .filter(|c| !c.is_closed())
            .map(|c| c.name.clone());

        let mut current: Vec<NextItem> = Vec::new();
        let mut approve: Vec<(Option<String>, NextItem)> = Vec::new();
        let mut start: Vec<(Option<String>, NextItem)> = Vec::new();
        for c in changes.values().filter(|c| !c.is_closed()) {
            let Some((kind, phase)) = first_open(c) else {
                continue;
            };
            let since = stamp(c);
            let is_active = active.as_deref() == Some(c.name.as_str());
            let (change, r#loop, phase) = (c.name.clone(), c.loop_.clone(), phase.to_string());
            let item_since = since.clone();
            let item = match kind {
                Kind::Approve => NextItem::Approve {
                    change,
                    r#loop,
                    phase,
                    active: is_active,
                    since: item_since,
                },
                Kind::Start => NextItem::Start {
                    change,
                    r#loop,
                    phase,
                    active: is_active,
                    since: item_since,
                },
            };
            if is_active {
                current.push(item);
            } else if kind == Kind::Approve {
                approve.push((since, item));
            } else {
                start.push((since, item));
            }
        }
        // `changes` iterates by name, so the sort's tie-break by name holds.
        approve.sort_by(|a, b| since_order(&a.0, &b.0));
        start.sort_by(|a, b| since_order(&a.0, &b.0));

        let (gathered, problems) = self.gather_tickets(&store)?;
        let mut warnings = store.take_warnings();
        warnings.extend(problems.iter().map(|p| Warning {
            message: p.to_string(),
        }));

        let mut builds: Vec<NextItem> = Vec::new();
        match repos::load(self.root()) {
            Err(e) => warnings.push(Warning {
                message: format!("repos: {e}; no tickets offered to build"),
            }),
            Ok(reg) => {
                let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
                for g in &gathered {
                    *counts.entry(g.ticket.id.as_str()).or_default() += 1;
                }
                let kept: Vec<&Gathered> = gathered
                    .iter()
                    .filter(|g| {
                        let t = &g.ticket;
                        counts[t.id.as_str()] == 1
                            // `build new` finds a ticket by its file name.
                            && g.ticket.path.ends_with(&format!("/{}.md", t.id))
                            && !(g.inbox && planned_id(&t.id))
                            && !changes.contains_key(&t.id)
                            && ticket::validate_file(t).is_empty()
                            && buildable_status(t)
                            && !g.change_closed
                            && !t.repo.is_empty()
                            && reg.repos.contains_key(&t.repo)
                            && !reg.is_archived(&t.repo)
                            && t.depends_on.iter().all(|d| done(d, &changes, &gathered))
                    })
                    .collect();
                let refs: Vec<&Ticket> = kept.iter().map(|g| &g.ticket).collect();
                let order = match ticket::order_tickets(&refs) {
                    Ok(order) => order,
                    Err(msg) => {
                        warnings.push(Warning {
                            message: format!("tickets: {msg}; showing tickets to build by id"),
                        });
                        let mut order: Vec<usize> = (0..kept.len()).collect();
                        order.sort_by(|&a, &b| refs[a].id.as_bytes().cmp(refs[b].id.as_bytes()));
                        order
                    }
                };
                builds.extend(order.into_iter().map(|i| {
                    let g = kept[i];
                    NextItem::Build {
                        ticket: g.ticket.id.clone(),
                        title: Text::from(g.ticket.title.clone()),
                        repo: g.ticket.repo.clone(),
                        change: g.change.clone(),
                        since: build_since(g, &changes),
                    }
                }));
            }
        }

        let items = current
            .into_iter()
            .chain(approve.into_iter().map(|(_, i)| i))
            .chain(start.into_iter().map(|(_, i)| i))
            .chain(builds)
            .collect();
        Ok(Outcome {
            value: NextList { items },
            warnings,
        })
    }
}

#[cfg(test)]
mod tests;
