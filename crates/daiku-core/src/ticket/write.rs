//! The write side of Go's `internal/core/ticket`: creating a ticket, the
//! readiness and publish marks, unfreezing, and ids (S-0007). Every change goes
//! through the ticket's `Document`, so fields that don't change keep their exact
//! text, and the body keeps its bytes (D2).

use std::sync::LazyLock;

use regex::Regex;

use super::{
    NATIVE_PREFIX, PLANNED_RE, PULLED_PREFIX, SCHEMA, STATUS_DRAFT, STATUS_PUBLISHED, STATUS_READY,
    Ticket, content_hash, drift_message, prefix_of,
};
use crate::frontmatter::Document;
use crate::{Error, gofmt};

/// The fields `Ticket::new` writes: Go's `New` takes a `Ticket` and reads these.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NewTicket {
    pub id: String,
    pub typ: String,
    pub title: String,
    pub parent: String,
    pub repo: String,
    pub depends_on: Vec<String>,
    pub supersedes: String,
    pub change: String,
    pub spec: String,
    pub adrs: Vec<String>,
    pub requirements: Vec<String>,
    pub labels: Vec<String>,
    pub estimate: String,
    pub status: String,
    pub body: String,
}

fn refused(id: &str, what: String) -> Error {
    Error::Ticket {
        message: format!("ticket {id}: {what}"),
    }
}

impl Ticket {
    /// Go's `New`: a ticket in the standard field order, read back as a parsed
    /// ticket so its bytes are what a later load sees.
    pub fn new(f: NewTicket) -> Result<Ticket, Error> {
        let mut doc = Document::new(f.body);
        let schema = SCHEMA.to_string();
        for (key, value) in [
            ("schema", schema.as_str()),
            ("id", &f.id),
            ("type", &f.typ),
            ("title", &f.title),
            ("parent", &f.parent),
            ("repo", &f.repo),
        ] {
            if !value.is_empty() {
                doc.set(key, value)?;
            }
        }
        doc.set_list("depends_on", &f.depends_on)?;
        if !f.supersedes.is_empty() {
            doc.set("supersedes", &f.supersedes)?;
        }
        for (key, value) in [("change", &f.change), ("spec", &f.spec)] {
            if !value.is_empty() {
                doc.set(key, value)?;
            }
        }
        doc.set_list("adrs", &f.adrs)?;
        // Optional, so it's written only when there's something to cite.
        if !f.requirements.is_empty() {
            doc.set_list("requirements", &f.requirements)?;
        }
        doc.set_list("labels", &f.labels)?;
        if !f.estimate.is_empty() {
            doc.set("estimate", &f.estimate)?;
        }
        let status = if f.status.is_empty() {
            STATUS_DRAFT
        } else {
            &f.status
        };
        doc.set("status", status)?;
        Ticket::parse(&doc.to_bytes())
    }

    /// Go's `Bytes`: the rendered ticket. A parsed ticket's body is written back
    /// as the bytes it was read with.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.doc.to_bytes();
        if let Some(raw) = &self.raw_body {
            out.truncate(out.len() - self.doc.body.len());
            out.extend_from_slice(raw);
        }
        out
    }

    /// Go's inbox `Pull` edit of a draft: the id set, `origin` and `change` dropped,
    /// everything else as the agent wrote it. Returns the rendered file and the
    /// ticket it parses to. A field that wasn't valid UTF-8 in the draft stays
    /// reported, so validation refuses it as Go's does, rather than writing U+FFFD.
    pub fn pulled_as(&self, id: &str) -> Result<(Vec<u8>, Ticket), Error> {
        let mut edited = self.clone();
        edited.doc.set("id", id)?;
        edited.doc.delete("origin");
        edited.doc.delete("change");
        let data = edited.to_bytes();
        let mut t = Ticket::parse(&data)?;
        if self.lossy_frontmatter {
            t.invalid_utf8 = self.invalid_utf8.clone();
            t.lossy_frontmatter = true;
        }
        Ok((data, t))
    }

    /// Refuses a write that would change a frontmatter byte that isn't UTF-8.
    pub fn check_writable(&self) -> Result<(), Error> {
        if self.lossy_frontmatter {
            return Err(Error::TicketNotUtf8 {
                id: self.id.clone(),
            });
        }
        Ok(())
    }

    /// Go's `MarkReady`: a draft or ready ticket gets `status: ready` and a
    /// readiness mark over its content.
    pub fn mark_ready(&mut self) -> Result<(), Error> {
        if self.status != STATUS_DRAFT && self.status != STATUS_READY {
            return Err(refused(
                &self.id,
                format!(
                    "only a draft or ready ticket can be readied, it's {}",
                    self.status
                ),
            ));
        }
        self.doc.set("status", STATUS_READY)?;
        let hash = content_hash(self);
        self.doc.set("ready_hash", &hash)?;
        self.status = STATUS_READY.to_string();
        self.ready_hash = hash;
        Ok(())
    }

    /// Go's `MarkPublished`: records the PM tool's id and freezes the ticket.
    pub fn mark_published(&mut self, tool: &str, item: &str) -> Result<(), Error> {
        if tool.is_empty() || item.is_empty() {
            return Err(refused(
                &self.id,
                "a tool and an item id are required".into(),
            ));
        }
        if self.malformed_freeze() {
            return Err(refused(
                &self.id,
                format!(
                    "published_hash {} isn't a valid v2 hash — unfreeze it to re-freeze",
                    gofmt::quote(&self.published_hash)
                ),
            ));
        }
        let (body, fm) = self.drift();
        if body || fm {
            return Err(refused(
                &self.id,
                format!(
                    "{} since it was published — unfreeze it or write a superseding ticket",
                    drift_message(body, fm)
                ),
            ));
        }
        if let Some(existing) = self.external.get(tool)
            && self.frozen()
            && !existing.is_empty()
            && existing != item
        {
            return Err(refused(
                &self.id,
                format!("frozen with {tool} id {existing}, can't change it to {item}"),
            ));
        }
        self.doc.set_sub("external", tool, item)?;
        let hash = content_hash(self);
        self.doc.set("published_hash", &hash)?;
        self.doc.set("status", STATUS_PUBLISHED)?;
        self.doc.delete("ready_hash");
        self.external = self.doc.map("external").unwrap_or_default();
        self.published_hash = hash;
        self.ready_hash = String::new();
        self.status = STATUS_PUBLISHED.to_string();
        Ok(())
    }

    /// Go's `Unfreeze`: clears the freeze, so the next sync updates the item.
    pub fn unfreeze(&mut self) -> Result<(), Error> {
        if !self.frozen() {
            return Err(refused(&self.id, "isn't frozen".into()));
        }
        self.check_writable()?;
        self.doc.delete("published_hash");
        self.published_hash = String::new();
        Ok(())
    }
}

/// Go's `sequence`: a planned or native id's number.
fn sequence(id: &str) -> Option<u32> {
    PLANNED_RE.captures(id).and_then(|m| m[2].parse().ok())
}

fn next_sequence(tickets: &[&Ticket]) -> u32 {
    tickets
        .iter()
        .filter_map(|t| sequence(&t.id))
        .max()
        .unwrap_or(0)
        + 1
}

/// Go's `NextID`: the next planned id for a type, from one sequence shared by
/// every ticket given.
pub fn next_id(tickets: &[&Ticket], typ: &str) -> Result<String, Error> {
    let prefix = prefix_of(typ).ok_or_else(|| Error::Ticket {
        message: format!("ticket: unknown type {}", gofmt::quote(typ)),
    })?;
    Ok(format!("{prefix}-{:04}", next_sequence(tickets)))
}

/// Go's `NextNativeID`.
pub fn next_native_id(tickets: &[&Ticket]) -> String {
    format!("{NATIVE_PREFIX}-{:04}", next_sequence(tickets))
}

static PULLED_DROP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[^A-Za-z0-9-]+").expect("valid pattern"));
static PULLED_HYPHENS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-{2,}").expect("valid pattern"));

/// Go's `PulledID`: a PM tool's item id as a pulled ticket id. It drops a leading
/// `X-` in either case, keeps letters, digits and hyphens, collapses and trims
/// hyphens, and prefixes `X-`.
pub fn pulled_id(pm: &str) -> Result<String, Error> {
    // Go's `strings.TrimSpace` trims Unicode White_Space, as `str::trim` does.
    let mut rest = pm.trim();
    if rest.len() >= 2
        && rest.as_bytes()[..2].eq_ignore_ascii_case(format!("{PULLED_PREFIX}-").as_bytes())
    {
        rest = &rest[2..];
    }
    let dropped = PULLED_DROP.replace_all(rest, "");
    let collapsed = PULLED_HYPHENS.replace_all(&dropped, "-");
    let id = collapsed.trim_matches('-');
    if id.is_empty() {
        return Err(Error::Ticket {
            message: format!(
                "ticket: PM id {} has no letters or digits",
                gofmt::quote(pm)
            ),
        });
    }
    Ok(format!("{PULLED_PREFIX}-{id}"))
}

/// Go's inbox `PMID`: the PM tool's item id in an external value, the text after
/// its last `#`, or the whole value.
pub fn pm_id(value: &str) -> &str {
    match value.rfind('#') {
        Some(i) => &value[i + 1..],
        None => value,
    }
}
