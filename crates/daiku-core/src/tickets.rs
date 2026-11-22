//! The `tickets` commands: Go's `internal/cli/tickets.go` without the argument
//! parsing and printing (S-0007). Each verb is a `Workspace` method that returns
//! data. A list of problems Go prints as `✗` lines with exit 1 comes back as a
//! value (`Refused`, `CheckReport::errors`); only what Go returns as an `error` is
//! an `Err` (D4).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::outcome::{Outcome, Warning};
use crate::plan::{SPECS_DIR, model, problems::problems as plan_problems, spec::Severity};
use crate::state::store::{Store, at, shown};
use crate::state::{BUILD_LOOP, overview};
use crate::ticket::{
    self, NewTicket, Problem, STATUS_DRAFT, STATUS_PUBLISHED, STATUS_READY, STORY, TASK, Ticket,
};
use crate::{Error, gofmt, inbox, repos, scaffold, settings, write};

/// `tickets new`'s inputs, parsed as Go's handler parses them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketNewOptions {
    pub typ: String,
    pub title: String,
    pub parent: String,
    pub repo: String,
    pub depends_on: Vec<String>,
    pub requirements: Vec<String>,
    pub inbox: bool,
}

/// A ticket `tickets new` wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Created {
    pub id: String,
    /// Shown as Go prints it: joined to the root, slash-separated.
    pub path: String,
    /// Written to the build inbox rather than a plan change.
    pub inbox: bool,
}

/// `tickets check`'s findings, in Go's order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckReport {
    /// `✗` lines; any makes the check fail.
    pub errors: Vec<Problem>,
    /// `!` lines.
    pub warnings: Vec<String>,
    /// Tickets read: the plan's and the inbox's.
    pub count: usize,
}

/// `tickets status`: each bucket in Go's order with its sorted ids, and the
/// files that couldn't be read.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub buckets: Vec<Bucket>,
    pub unreadable: Vec<Problem>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Bucket {
    pub name: String,
    pub ids: Vec<String>,
}

/// One line of `tickets order`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Ordered {
    pub id: String,
    pub title: String,
}

/// What `tickets ready` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyTargets {
    /// The active plan change's drafts.
    All,
    Ids(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum ReadyResult {
    /// The tickets readied, in the order they were written.
    Readied(Vec<String>),
    /// `--all` found no draft in the named change.
    NoDrafts(String),
    /// Nothing was written.
    Refused(Vec<Problem>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Published {
    pub id: String,
    pub tool: String,
    pub item: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub enum PublishResult {
    Published(Published),
    /// Nothing was written.
    Refused(Vec<Problem>),
}

/// A ticket `tickets pull` recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Pulled {
    pub id: String,
    pub rel: String,
    pub tool: String,
}

/// Go's `checkGate`: a gate not at the wanted status, through the gates policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct GateCheck {
    /// The log line for a violation let through.
    override_line: Option<String>,
    /// The refusal.
    problem: Option<Problem>,
    warnings: Vec<Warning>,
}

static REQUIREMENT_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[RN][0-9]+$").expect("valid pattern"));

const TICKET_TEMPLATE: &str = "plan/05_breakdown/templates/ticket.md";

fn ticket_dir_rel(change: &str) -> String {
    format!("{SPECS_DIR}/{change}/tickets")
}

/// Go's `loadTickets`: every plan change's tickets, and the files that couldn't
/// be read.
fn load_all_tickets(store: &Store) -> Result<(Vec<Ticket>, Vec<Problem>), Error> {
    let (mut all, mut problems) = (Vec::new(), Vec::new());
    for c in store.plan_changes()? {
        let rel = ticket_dir_rel(&c.name);
        let (ts, bad) = ticket::load_dir(&at(store.root, &rel), &shown(store.root, &rel))?;
        all.extend(ts);
        problems.extend(bad);
    }
    Ok((all, problems))
}

fn by_id(tickets: &[Ticket]) -> BTreeMap<&str, Vec<&Ticket>> {
    let mut m: BTreeMap<&str, Vec<&Ticket>> = BTreeMap::new();
    for t in tickets {
        m.entry(t.id.as_str()).or_default().push(t);
    }
    m
}

/// Go's `templateBody`: the body of the embedded ticket template.
fn template_body() -> Result<String, Error> {
    let text = scaffold::engine::template(TICKET_TEMPLATE)
        .unwrap_or_else(|| panic!("{TICKET_TEMPLATE} is embedded"));
    Ok(Ticket::parse(text.as_bytes())?.body)
}

/// Go's `gateStatus`.
fn gate_status<'a>(gates: Option<&'a BTreeMap<String, String>>, phase: &str) -> &'a str {
    match gates.and_then(|g| g.get(phase)) {
        Some(s) if !s.is_empty() => s,
        _ => "unset",
    }
}

/// Go's `refreshOverviews`: regenerates the changes' READMEs and returns the paths
/// written. A failure is a warning, and that README is left alone.
fn refresh_overviews(store: &Store, changes: &[&str]) -> (Vec<String>, Vec<Warning>) {
    let loaded = match settings::load(store.root, &store.env.dirs) {
        Ok(l) => l,
        Err(e) => {
            return (
                Vec::new(),
                vec![Warning {
                    message: e.to_string(),
                }],
            );
        }
    };
    let names: BTreeSet<&str> = changes.iter().copied().filter(|c| !c.is_empty()).collect();
    let (mut written, mut errs) = (Vec::new(), Vec::new());
    for c in names {
        match overview::refresh(store, c, &loaded) {
            Ok(w) => written.extend(w),
            Err(e) => errs.push(e.to_string()),
        }
    }
    let warnings = if errs.is_empty() {
        Vec::new()
    } else {
        vec![Warning {
            message: errs.join("\n"),
        }]
    };
    (written, warnings)
}

fn rel_of(root: &Path, t: &Ticket) -> String {
    let base = gofmt::to_slash(&root.to_string_lossy());
    let p = t.path.as_str();
    if base == "." {
        return p.to_string();
    }
    p.strip_prefix(&format!("{base}/")).unwrap_or(p).to_string()
}

impl crate::Workspace {
    fn store(&self) -> Store<'_> {
        Store::reader(self.root(), self.env())
    }

    /// Go's `checkGate`: `change` must have `phase` at `want` for `cmd`.
    fn check_gate(
        &self,
        store: &Store,
        cmd: &str,
        change: &str,
        phase: &str,
        want: &str,
        force: bool,
    ) -> Result<GateCheck, Error> {
        let changes = store.read_changes()?;
        let status = gate_status(changes.get(change).map(|c| &c.gates), phase);
        if status == want {
            return Ok(GateCheck::default());
        }
        let violation = format!(
            "daiku: {cmd} requires {phase} {want} for {change} (it's {})",
            gofmt::quote(status)
        );
        match self.policy(vec![violation.clone()], force) {
            Ok(warnings) => Ok(GateCheck {
                override_line: Some(format!("override: {violation}")),
                problem: None,
                warnings,
            }),
            Err(e) => Ok(GateCheck {
                problem: Some(Problem::new(change, e.to_string())),
                ..GateCheck::default()
            }),
        }
    }

    /// Go's `saveTicket`: writes the ticket, logs the lines under its change and
    /// records them with the ticket and any README refreshed.
    fn save_ticket(&self, t: &Ticket, lines: &[String]) -> Result<Vec<Warning>, Error> {
        let store = Store::at(self.root(), self.env())?;
        store.check_state_files()?;
        let rel = rel_of(self.root(), t);
        write::write_file(self.root(), &at(self.root(), &rel), &t.to_bytes())?;
        let logged: Vec<String> = lines.iter().map(|l| format!("{}: {l}", t.change)).collect();
        let (written, mut warnings) = refresh_overviews(&store, &[model::ticket_change(t)]);
        let mut paths = vec![rel];
        paths.extend(written);
        store.record(&logged, &paths)?;
        warnings.extend(store.take_warnings());
        Ok(warnings)
    }

    /// `daiku tickets new`: Go's `ticketsNew` after parsing.
    pub fn tickets_new(&self, o: &TicketNewOptions) -> Result<Outcome<Created>, Error> {
        let store = self.store();
        let st = store.load_state()?;
        // Without an open plan change to plan it in, a ticket goes to the inbox.
        if o.inbox || st.name.is_empty() || st.loop_ == BUILD_LOOP || st.is_closed() {
            return self.inbox_ticket(o);
        }
        let mut repo = o.repo.clone();
        if repo.is_empty() && (o.typ == STORY || o.typ == TASK) {
            if st.repos.len() != 1 {
                return Err(Error::TicketNeedsRepo { typ: o.typ.clone() });
            }
            repo = st.repos[0].clone();
        }
        if !repo.is_empty() && !repos::load(self.root())?.repos.contains_key(&repo) {
            return Err(Error::TicketRepoNotRegistered { repo });
        }
        self.check_requirements(&st.name, &o.requirements)?;
        let (all, problems) = load_all_tickets(&store)?;
        if let Some(p) = problems.first() {
            return Err(Error::IdUnallocatable {
                problem: p.to_string(),
            });
        }
        let refs: Vec<&Ticket> = all.iter().collect();
        let id = ticket::next_id(&refs, &o.typ)?;
        let t = Ticket::new(NewTicket {
            id: id.clone(),
            typ: o.typ.clone(),
            title: o.title.clone(),
            parent: o.parent.clone(),
            repo,
            depends_on: o.depends_on.clone(),
            requirements: o.requirements.clone(),
            change: st.name.clone(),
            spec: format!("{SPECS_DIR}/{}/functional-spec.md", st.name),
            body: template_body()?,
            ..NewTicket::default()
        })?;
        let rel = format!("{}/{id}.md", ticket_dir_rel(&st.name));
        write::write_file(self.root(), &at(self.root(), &rel), &t.to_bytes())?;
        // Regenerated, but not recorded: the next recorded transition commits it.
        let (_, warnings) = refresh_overviews(&store, &[st.name.as_str()]);
        Ok(Outcome {
            value: Created {
                id,
                path: shown(self.root(), &rel),
                inbox: false,
            },
            warnings,
        })
    }

    /// Go's `inboxTicket`: a native task in `build/tickets/`.
    fn inbox_ticket(&self, o: &TicketNewOptions) -> Result<Outcome<Created>, Error> {
        if !o.parent.is_empty() || !o.depends_on.is_empty() || !o.requirements.is_empty() {
            return Err(Error::InboxPlanFields);
        }
        if o.typ != TASK {
            return Err(Error::InboxNotTask { typ: o.typ.clone() });
        }
        if o.repo.is_empty() {
            return Err(Error::InboxNeedsRepo);
        }
        if !repos::load(self.root())?.repos.contains_key(&o.repo) {
            return Err(Error::InboxRepoNotRegistered {
                repo: o.repo.clone(),
            });
        }
        let t = inbox::create(
            self.root(),
            &inbox::NewInbox {
                title: o.title.clone(),
                repo: o.repo.clone(),
                labels: Vec::new(),
                body: template_body()?,
            },
        )?;
        Ok(Outcome::new(Created {
            id: t.id.clone(),
            path: t.path,
            inbox: true,
        }))
    }

    /// Go's `checkRequirements`: every id must be well formed and in the change's
    /// spec; all the bad ones are named together.
    fn check_requirements(&self, change: &str, ids: &[String]) -> Result<(), Error> {
        if ids.is_empty() {
            return Ok(());
        }
        let adr_dir = settings::load(self.root(), &self.env().dirs)?
            .effective
            .adr_dir;
        let m = model::load(self.root(), change, &adr_dir)?;
        let defined: BTreeSet<&str> = m.requirements.iter().map(|r| r.id.as_str()).collect();
        let bad: Vec<String> = ids
            .iter()
            .filter_map(|id| {
                if !REQUIREMENT_ID_RE.is_match(id) {
                    Some(format!("{id} isn't a requirement id (R<n> or N<n>)"))
                } else if !m.has_spec {
                    Some(format!("{id} can't be checked: {change} has no spec"))
                } else if !defined.contains(id.as_str()) {
                    Some(format!("{id} isn't in {change}'s spec"))
                } else {
                    None
                }
            })
            .collect();
        if bad.is_empty() {
            Ok(())
        } else {
            Err(Error::UnknownRequirements { problems: bad })
        }
    }

    /// `daiku tickets check`: Go's `ticketsCheck`.
    pub fn tickets_check(&self) -> Result<Outcome<CheckReport>, Error> {
        let store = self.store();
        let (all, mut errors) = load_all_tickets(&store)?;
        // The inbox is checked with the plan tickets; an X- ticket outside it is
        // an error.
        let (inboxed, located) = inbox::list(self.root())?;
        errors.extend(located);
        for t in &all {
            if t.id.split_once('-').map(|(p, _)| p) == Some(ticket::PULLED_PREFIX) {
                errors.push(Problem::new(
                    &t.id,
                    format!(
                        "pulled tickets live in build/tickets/, not {}",
                        rel_of(self.root(), t)
                    ),
                ));
            }
        }
        let names = repos::load(self.root())?.names();
        let in_inbox: BTreeSet<&str> = inboxed.iter().map(|t| t.id.as_str()).collect();
        let every: Vec<&Ticket> = all.iter().chain(inboxed.iter()).collect();
        let mut warnings = Vec::new();
        for p in ticket::validate_workspace(&every, &names) {
            if !p.reference.is_empty() && in_inbox.contains(p.id.as_str()) {
                warnings.push(format!(
                    "{}: {} isn't in this workspace (planned elsewhere)",
                    p.id,
                    p.message.trim_end_matches(" doesn't exist")
                ));
                continue;
            }
            errors.push(p);
        }
        // A ready ticket whose breakdown was reopened is warned about.
        let changes = store.read_changes()?;
        for t in &all {
            if t.status != STATUS_READY {
                continue;
            }
            let s = gate_status(changes.get(&t.change).map(|c| &c.gates), "breakdown");
            if s != "approved" {
                warnings.push(format!(
                    "{}: ready, but breakdown for {} is {}",
                    t.id,
                    t.change,
                    gofmt::quote(s)
                ));
            }
        }
        let adr_dir = settings::load(self.root(), &self.env().dirs)?
            .effective
            .adr_dir;
        let m = model::load_all(self.root(), &adr_dir)?;
        for f in plan_problems(&m) {
            match f.severity {
                Severity::Error => errors.push(Problem::new(&f.id, f.message)),
                Severity::Warning => warnings.push(format!("{}: {}", f.id, f.message)),
            }
        }
        Ok(Outcome::new(CheckReport {
            errors,
            warnings,
            count: all.len() + inboxed.len(),
        }))
    }

    /// `daiku tickets status`: Go's `ticketsStatus`.
    pub fn tickets_status(&self) -> Result<Outcome<StatusReport>, Error> {
        let (all, unreadable) = load_all_tickets(&self.store())?;
        let mut buckets: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        for t in &all {
            let bucket = if t.drifted() {
                "drifted"
            } else if t.status == STATUS_PUBLISHED && !t.frozen() {
                "unfrozen"
            } else {
                t.status.as_str()
            };
            buckets.entry(bucket).or_default().push(t.id.clone());
        }
        let buckets = [
            STATUS_DRAFT,
            STATUS_READY,
            STATUS_PUBLISHED,
            "unfrozen",
            "drifted",
        ]
        .iter()
        .map(|name| {
            let mut ids = buckets.remove(name).unwrap_or_default();
            ids.sort();
            Bucket {
                name: name.to_string(),
                ids,
            }
        })
        .collect();
        Ok(Outcome::new(StatusReport {
            buckets,
            unreadable,
        }))
    }

    /// `daiku tickets order`: Go's `ticketsOrder`.
    pub fn tickets_order(&self) -> Result<Outcome<Vec<Ordered>>, Error> {
        let (all, problems) = load_all_tickets(&self.store())?;
        if let Some(p) = problems.first() {
            return Err(Error::UnreadableTicket {
                problem: p.to_string(),
            });
        }
        let refs: Vec<&Ticket> = all.iter().collect();
        let order =
            ticket::order_tickets(&refs).map_err(|message| Error::TicketOrder { message })?;
        Ok(Outcome::new(
            order
                .into_iter()
                .map(|i| Ordered {
                    id: all[i].id.clone(),
                    title: all[i].title.clone(),
                })
                .collect(),
        ))
    }

    /// Go's `findTicket`.
    fn find_ticket(&self, id: &str) -> Result<Ticket, Error> {
        let (all, _) = load_all_tickets(&self.store())?;
        let mut found = all.into_iter().filter(|t| t.id == id);
        match (found.next(), found.next()) {
            (None, _) => Err(Error::TicketNotFound { id: id.into() }),
            (Some(t), None) => Ok(t),
            (Some(_), Some(_)) => Err(Error::TicketAmbiguous { id: id.into() }),
        }
    }

    /// `daiku tickets ready`: Go's `ticketsReady`. It's all or nothing: it checks a
    /// readied copy of every target, and writes only when none has a problem.
    pub fn tickets_ready(
        &self,
        targets: &ReadyTargets,
        force: bool,
    ) -> Result<Outcome<ReadyResult>, Error> {
        let store = self.store();
        let (loaded, unreadable) = load_all_tickets(&store)?;
        let mut problems = unreadable;
        let index = by_id(&loaded);
        let mut chosen: Vec<&Ticket> = Vec::new();
        match targets {
            ReadyTargets::All => {
                let st = store.load_state()?;
                if st.name.is_empty() {
                    return Err(Error::NoActiveChange);
                }
                if st.loop_ == BUILD_LOOP {
                    return Err(Error::BuildChange {
                        name: st.name,
                        verb: "tickets ready --all",
                    });
                }
                chosen = loaded
                    .iter()
                    .filter(|t| t.change == st.name && t.status == STATUS_DRAFT)
                    .collect();
                if chosen.is_empty() && problems.is_empty() {
                    return Ok(Outcome::new(ReadyResult::NoDrafts(st.name)));
                }
            }
            ReadyTargets::Ids(ids) => {
                let mut seen = BTreeSet::new();
                for id in ids {
                    if !seen.insert(id.as_str()) {
                        continue;
                    }
                    match index.get(id.as_str()).map(Vec::as_slice) {
                        None | Some([]) => problems.push(Problem::new(id, "no such ticket".into())),
                        Some([t]) => chosen.push(t),
                        Some(_) => problems
                            .push(Problem::new(id, "more than one ticket has this id".into())),
                    }
                }
            }
        }
        let targeted: BTreeSet<&str> = chosen.iter().map(|t| t.id.as_str()).collect();

        // The breakdown gate, once per change.
        let (mut lines, mut warnings) = (Vec::new(), Vec::new());
        let mut checked = BTreeSet::new();
        for t in &chosen {
            if !checked.insert(t.change.as_str()) {
                continue;
            }
            let g = self.check_gate(
                &store,
                "tickets ready",
                &t.change,
                "breakdown",
                "approved",
                force,
            )?;
            problems.extend(g.problem);
            if let Some(o) = g.override_line {
                lines.push(format!("{}: {o}", t.change));
            }
            warnings.extend(g.warnings);
        }

        // Check a readied copy of each target.
        let mut copies = Vec::new();
        for t in &chosen {
            let mut c = (*t).clone();
            if let Err(e) = c.mark_ready() {
                problems.push(Problem::new(&t.id, e.to_string()));
                continue;
            }
            problems.extend(ticket::validate_file(&c));
            if !c.parent.is_empty() && !targeted.contains(c.parent.as_str()) {
                match index.get(c.parent.as_str()).and_then(|f| f.first()) {
                    None => problems.push(Problem::new(
                        &t.id,
                        format!("parent {} not found", c.parent),
                    )),
                    Some(p) if p.status != STATUS_READY && p.status != STATUS_PUBLISHED => {
                        problems.push(Problem::new(
                            &t.id,
                            format!(
                                "parent {} is {}; ready it first or in the same run",
                                c.parent, p.status
                            ),
                        ));
                    }
                    Some(_) => {}
                }
            }
            copies.push(c);
        }
        if !problems.is_empty() {
            return Ok(Outcome {
                value: ReadyResult::Refused(problems),
                warnings,
            });
        }

        // Check every file before writing any, so a refusal leaves them as they were.
        let store = Store::at(self.root(), self.env())?;
        store.check_state_files()?;
        let rels: Vec<String> = copies.iter().map(|c| rel_of(self.root(), c)).collect();
        for (c, rel) in copies.iter().zip(&rels) {
            c.check_writable()?;
            write::check_path(self.root(), &at(self.root(), rel))?;
        }
        let mut paths = Vec::new();
        for (c, rel) in copies.iter().zip(rels) {
            write::write_file(self.root(), &at(self.root(), &rel), &c.to_bytes())?;
            paths.push(rel);
            lines.push(format!("{}: {} readied", c.change, c.id));
        }
        let changes: Vec<&str> = copies.iter().map(model::ticket_change).collect();
        let (written, w) = refresh_overviews(&store, &changes);
        warnings.extend(w);
        paths.extend(written);
        store.record(&lines, &paths)?;
        warnings.extend(store.take_warnings());
        Ok(Outcome {
            value: ReadyResult::Readied(copies.iter().map(|c| c.id.clone()).collect()),
            warnings,
        })
    }

    /// `daiku tickets mark-published`: Go's `ticketsMarkPublished`. It checks first
    /// and changes nothing if any check fails.
    pub fn tickets_mark_published(
        &self,
        id: &str,
        tool: &str,
        item: &str,
        force: bool,
    ) -> Result<Outcome<PublishResult>, Error> {
        let store = self.store();
        let (loaded, unreadable) = load_all_tickets(&store)?;
        let index = by_id(&loaded);
        let t = match index.get(id).map(Vec::as_slice) {
            None | Some([]) => return Err(Error::TicketNotFound { id: id.into() }),
            Some([t]) => (*t).clone(),
            Some(_) => return Err(Error::TicketAmbiguous { id: id.into() }),
        };
        // An unreadable ticket may be a predecessor, so it can't be verified.
        let mut problems = unreadable;
        let mut lines = Vec::new();
        let g = self.check_gate(
            &store,
            "mark-published",
            &t.change,
            "publish",
            "in-progress",
            force,
        )?;
        problems.extend(g.problem);
        lines.extend(g.override_line);
        let mut warnings = g.warnings;
        problems.extend(publish_problems(&t, tool, &index));
        if !problems.is_empty() {
            return Ok(Outcome {
                value: PublishResult::Refused(problems),
                warnings,
            });
        }
        let mut t = t;
        t.mark_published(tool, item)
            .map_err(|e| Error::TicketRefused {
                message: e.to_string(),
            })?;
        t.check_writable()?;
        lines.push(format!("{id} published to {tool} ({item})"));
        warnings.extend(self.save_ticket(&t, &lines)?);
        Ok(Outcome {
            value: PublishResult::Published(Published {
                id: id.into(),
                tool: tool.into(),
                item: item.into(),
            }),
            warnings,
        })
    }

    /// `daiku tickets unfreeze`: Go's `ticketsUnfreeze` after parsing.
    pub fn tickets_unfreeze(&self, id: &str, reason: &str) -> Result<Outcome<String>, Error> {
        // One line in LOG.md, and a reason that was only line breaks is missing
        // (N-0006).
        let reason = crate::text::fold_reason(reason);
        if reason.is_empty() {
            return Err(Error::ReasonRequired);
        }
        let mut t = self.find_ticket(id)?;
        t.unfreeze().map_err(|e| match e {
            Error::Ticket { message } => Error::TicketRefused { message },
            e => e,
        })?;
        let warnings = self.save_ticket(&t, &[format!("{id} unfrozen ({reason})")])?;
        Ok(Outcome {
            value: id.into(),
            warnings,
        })
    }

    /// `daiku tickets pull <file>`: Go's `ticketsPull` after parsing. The file is
    /// read as given, relative to the current folder.
    pub fn tickets_pull(&self, file: &str) -> Result<Outcome<Pulled>, Error> {
        let draft = std::fs::read(file).map_err(|e| Error::PullUnreadable {
            file: file.into(),
            cause: format!("open {file}: {}", gofmt::io_error_text(&e)),
        })?;
        let store = Store::at(self.root(), self.env())?;
        store.check_state_files()?;
        let t = inbox::pull(self.root(), &draft)?;
        let tool = t.external.keys().next().cloned().unwrap_or_default();
        let rel = inbox::rel(&t.id);
        store.record(
            &[format!("{}: pulled from {tool}", t.id)],
            std::slice::from_ref(&rel),
        )?;
        Ok(Outcome {
            value: Pulled {
                id: t.id,
                rel,
                tool,
            },
            warnings: store.take_warnings(),
        })
    }
}

/// Go's `publishProblems`: the ticket's status, and that its parent, each
/// `depends_on` ticket and its `supersedes` target have an id in `tool`. None of
/// these can be forced.
fn publish_problems(t: &Ticket, tool: &str, index: &BTreeMap<&str, Vec<&Ticket>>) -> Vec<Problem> {
    let mut problems = Vec::new();
    let add = |problems: &mut Vec<Problem>, m: String| problems.push(Problem::new(&t.id, m));
    match t.status.as_str() {
        STATUS_DRAFT => add(
            &mut problems,
            format!("is a draft — run `daiku tickets ready {}` first", t.id),
        ),
        STATUS_READY => problems.extend(ticket::validate_file(t)),
        STATUS_PUBLISHED => {}
        other => add(&mut problems, format!("can't publish a {other} ticket")),
    }
    let mut refs = vec![("parent", t.parent.as_str())];
    refs.extend(t.depends_on.iter().map(|d| ("depends_on", d.as_str())));
    refs.push(("supersedes", t.supersedes.as_str()));
    let mut seen = BTreeSet::new();
    for (role, id) in refs {
        if id.is_empty() || !seen.insert(id) {
            continue;
        }
        let message = match index.get(id).map(Vec::as_slice) {
            None | Some([]) => format!("{role} {id} not found"),
            Some([_, _, ..]) => {
                format!("{role} {id} is ambiguous (more than one ticket has this id)")
            }
            Some([found]) if found.external.get(tool).is_none_or(String::is_empty) => {
                format!("{role} {id} has no {tool} id; publish it first")
            }
            Some(_) => continue,
        };
        add(&mut problems, message);
    }
    problems
}

#[cfg(test)]
mod tests;
