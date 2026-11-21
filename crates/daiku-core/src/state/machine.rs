//! The phase machine: a loop's ordered phases and the rules for moving through
//! them. A port of Go's `internal/core/state/machine.go`.

use std::collections::BTreeMap;

use super::Change;
use crate::Error;

/// How a phase progresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// draft → await-approval → approved, or skipped.
    Gated,
    /// "" → in-progress → complete.
    Run,
}

/// Statuses a phase can hold; "" means not started.
pub const DRAFT: &str = "draft";
pub const AWAIT_APPROVAL: &str = "await-approval";
pub const APPROVED: &str = "approved";
pub const SKIPPED: &str = "skipped";
pub const IN_PROGRESS: &str = "in-progress";
pub const COMPLETE: &str = "complete";
/// A change's status once it's finished with; it isn't a gate.
pub const CLOSED: &str = "closed";

/// One step of a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase {
    pub name: &'static str,
    pub kind: Kind,
    /// May be set to skipped, with a reason.
    pub skippable: bool,
}

const fn gated(name: &'static str, skippable: bool) -> Phase {
    Phase {
        name,
        kind: Kind::Gated,
        skippable,
    }
}

const fn run(name: &'static str) -> Phase {
    Phase {
        name,
        kind: Kind::Run,
        skippable: false,
    }
}

/// A loop's ordered phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    pub phases: &'static [Phase],
}

/// The plan loop: intake → research → explore → synthesis → spec → architecture
/// → breakdown → publish.
pub const PLAN: Machine = Machine {
    phases: &[
        gated("intake", false),
        gated("research", true),
        gated("explore", true),
        gated("synthesis", true),
        gated("spec", false),
        gated("architecture", false),
        gated("breakdown", false),
        run("publish"),
    ],
};

/// The build loop: ticket → requirements → design → tasks → implement → test →
/// review → security → ship.
pub const BUILD: Machine = Machine {
    phases: &[
        gated("ticket", false),
        gated("requirements", false),
        gated("design", false),
        gated("tasks", false),
        run("implement"),
        gated("test", false),
        gated("review", false),
        gated("security", true),
        run("ship"),
    ],
};

fn valid_status(p: &Phase, status: &str) -> bool {
    match status {
        "" => true,
        DRAFT | AWAIT_APPROVAL | APPROVED => p.kind == Kind::Gated,
        SKIPPED => p.kind == Kind::Gated && p.skippable,
        IN_PROGRESS | COMPLETE => p.kind == Kind::Run,
        _ => false,
    }
}

/// Sets or, for "", removes `key`.
fn set(map: &mut BTreeMap<String, String>, key: &str, value: &str) {
    if value.is_empty() {
        map.remove(key);
    } else {
        map.insert(key.to_string(), value.to_string());
    }
}

impl Machine {
    fn find(&self, name: &str) -> Option<(usize, &'static Phase)> {
        self.phases.iter().enumerate().find(|(_, p)| p.name == name)
    }

    /// The phase names, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> {
        self.phases.iter().map(|p| p.name)
    }

    /// Go's `CheckGate`: the order violation of setting `phase` to `status`, if
    /// any. Every earlier phase must be approved or skipped when gated, complete
    /// when run. Clearing a gate is always allowed. Whether a violation blocks is
    /// the caller's policy.
    pub fn check_gate(&self, c: &Change, phase: &str, status: &str) -> Option<Error> {
        if status.is_empty() {
            return None;
        }
        for p in self.phases {
            if p.name == phase {
                return None;
            }
            let got = c.gates.get(p.name).map(String::as_str).unwrap_or("");
            let need = match p.kind {
                Kind::Run if got != COMPLETE => "complete",
                Kind::Gated if got != APPROVED && got != SKIPPED => "approved",
                _ => continue,
            };
            return Some(Error::OutOfOrder {
                phase: phase.to_string(),
                earlier: p.name,
                need,
                got: got.to_string(),
            });
        }
        None
    }

    /// Go's `SetGate`: starts `phase` with `status`, after checking the names, the
    /// reason a skip needs, and that the change isn't closed. The order is
    /// [`Machine::check_gate`]'s.
    pub fn set_gate(
        &self,
        c: &mut Change,
        phase: &str,
        status: &str,
        reason: &str,
    ) -> Result<(), Error> {
        if c.is_closed() {
            return Err(Error::ChangeClosed {
                name: c.name.clone(),
            });
        }
        let Some((_, p)) = self.find(phase) else {
            return Err(Error::UnknownPhase {
                phase: phase.to_string(),
            });
        };
        if !valid_status(p, status) {
            if status == SKIPPED && p.kind == Kind::Gated {
                return Err(Error::CantSkip {
                    phase: phase.to_string(),
                });
            }
            return Err(Error::UnknownStatus {
                status: status.to_string(),
            });
        }
        if status == SKIPPED && reason.is_empty() {
            return Err(Error::SkipNeedsReason {
                phase: phase.to_string(),
            });
        }
        c.phase = phase.to_string();
        c.status = status.to_string();
        set(&mut c.gates, phase, status);
        set(
            &mut c.reasons,
            phase,
            if status == SKIPPED { reason } else { "" },
        );
        Ok(())
    }

    /// Go's `Reopen`: back to a started phase, as draft (in-progress for a run
    /// phase). Later phases are cleared, except those skipped. A reason is needed.
    pub fn reopen(&self, c: &mut Change, phase: &str, reason: &str) -> Result<(), Error> {
        if c.is_closed() {
            return Err(Error::ChangeClosed {
                name: c.name.clone(),
            });
        }
        let Some((idx, p)) = self.find(phase) else {
            return Err(Error::UnknownPhase {
                phase: phase.to_string(),
            });
        };
        if reason.is_empty() {
            return Err(Error::ReopenNeedsReason {
                phase: phase.to_string(),
            });
        }
        if c.gates.get(phase).is_none_or(String::is_empty) {
            return Err(Error::NotStarted {
                phase: phase.to_string(),
            });
        }
        let status = if p.kind == Kind::Run {
            IN_PROGRESS
        } else {
            DRAFT
        };
        set(&mut c.gates, phase, status);
        for later in &self.phases[idx + 1..] {
            if c.gates.get(later.name).map(String::as_str) != Some(SKIPPED) {
                c.gates.remove(later.name);
            }
        }
        c.phase = phase.to_string();
        c.status = status.to_string();
        set(&mut c.reasons, phase, "");
        Ok(())
    }

    /// Go's `Close`: marks the change finished with. A reason is needed.
    pub fn close(&self, c: &mut Change, reason: &str) -> Result<(), Error> {
        if c.is_closed() {
            return Err(Error::AlreadyClosed {
                name: c.name.clone(),
            });
        }
        if reason.is_empty() {
            return Err(Error::CloseNeedsReason {
                name: c.name.clone(),
            });
        }
        c.status = CLOSED.to_string();
        Ok(())
    }
}
