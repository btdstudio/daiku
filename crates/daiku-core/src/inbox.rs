//! The build inbox, `build/tickets/`: Go's `internal/build/inbox` (S-0007).
//! Native tickets (`N-`) are created here, and tickets pulled from a PM tool
//! (`X-`) are recorded here. Planned tickets stay under `plan/specs/`.

use std::path::{Path, PathBuf};

use crate::state::store::{at, shown};
use crate::text::one_line;
use crate::ticket::{self, NATIVE_PREFIX, NewTicket, PULLED_PREFIX, Problem, TASK, Ticket};
use crate::{Error, write};

/// The inbox folder, relative to the workspace root.
pub const TICKETS_DIR: &str = "build/tickets";

/// The inbox folder under `root`.
pub fn dir(root: &Path) -> PathBuf {
    at(root, TICKETS_DIR)
}

/// A ticket's file in the inbox, relative to the root.
pub fn rel(id: &str) -> String {
    format!("{TICKETS_DIR}/{id}.md")
}

/// Go's `List`: every inbox ticket, sorted by id, and the problems: files that
/// can't be parsed, and planned ids, which belong under `plan/specs/`.
pub fn list(root: &Path) -> Result<(Vec<Ticket>, Vec<Problem>), Error> {
    let (mut tickets, mut problems) = ticket::load_dir(&dir(root), &shown(root, TICKETS_DIR))?;
    tickets.sort_by(|a, b| a.id.cmp(&b.id));
    for t in &tickets {
        if planned(&t.id) {
            problems.push(Problem::new(
                &t.id,
                format!("planned tickets live under plan/specs/, not in {TICKETS_DIR}/"),
            ));
        }
    }
    Ok((tickets, problems))
}

/// Go's `planned`: an E-, F-, S- or T- id.
fn planned(id: &str) -> bool {
    match id.split_once('-') {
        Some((prefix, _)) => prefix != NATIVE_PREFIX && prefix != PULLED_PREFIX,
        None => false,
    }
}

/// What `create` writes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewInbox {
    pub title: String,
    pub repo: String,
    pub labels: Vec<String>,
    pub body: String,
}

/// Go's `New`: a draft native task with the next `N-` id, counted over the inbox
/// alone.
pub fn create(root: &Path, opts: &NewInbox) -> Result<Ticket, Error> {
    if opts.repo.is_empty() {
        return Err(Error::InboxNeedsRepo);
    }
    let (all, problems) = ticket::load_dir(&dir(root), &shown(root, TICKETS_DIR))?;
    if let Some(p) = problems.first() {
        return Err(Error::IdUnallocatable {
            problem: p.to_string(),
        });
    }
    let refs: Vec<&Ticket> = all.iter().collect();
    let id = ticket::next_native_id(&refs);
    let mut t = Ticket::new(NewTicket {
        id: id.clone(),
        typ: TASK.into(),
        title: one_line(&opts.title),
        repo: opts.repo.clone(),
        labels: opts.labels.clone(),
        body: format!("\n{}\n", opts.body.trim_end_matches('\n')),
        ..NewTicket::default()
    })?;
    let rel = rel(&id);
    write::write_file(root, &at(root, &rel), &t.to_bytes())?;
    t.path = shown(root, &rel);
    Ok(t)
}

/// Go's `Pull`: records a draft fetched from a PM tool as `X-<pm-id>`, replacing
/// an earlier pull of the same item. The draft needs exactly one `external` entry.
pub fn pull(root: &Path, draft: &[u8]) -> Result<Ticket, Error> {
    let parsed = Ticket::parse(draft).map_err(|e| Error::PullDraft {
        cause: e.to_string(),
    })?;
    if parsed.external.len() != 1 {
        return Err(Error::PullExternalCount {
            n: parsed.external.len(),
        });
    }
    let value = parsed.external.values().next().expect("one entry");
    let id = ticket::pulled_id(ticket::pm_id(value)).map_err(|e| Error::TicketRefused {
        message: e.to_string(),
    })?;
    let (data, mut t) = parsed.pulled_as(&id).map_err(|e| Error::PullParse {
        id: id.clone(),
        cause: e.to_string(),
    })?;
    let rel = rel(&id);
    t.path = shown(root, &rel);
    if let Some(p) = ticket::validate_file(&t).first() {
        return Err(Error::PullInvalid {
            problem: p.to_string(),
        });
    }
    t.check_writable()?;
    write::write_file(root, &at(root, &rel), &data)?;
    Ok(t)
}

#[cfg(test)]
mod tests;
