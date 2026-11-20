//! Changes and gates as workspace operations (D5, D6, D10): `change new`,
//! `change close`, `gate`, `reopen`, `switch` and `status`. Each follows its Go
//! handler in `internal/cli` step by step, with the rules in core and the
//! printing left to the client.

use std::collections::BTreeMap;

use crate::plan::check::check_artifacts;
use crate::plan::overview::BuildRow;
use crate::plan::source::{self, NewSource};
use crate::state::machine::SKIPPED;
use crate::state::overview::{self, build_rows, build_ticket, plan_of};
use crate::state::store::{SeedOptions, at};
use crate::state::{
    self, BUILD_LOOP, Change, Known, NO_REPO_REASON, PLAN_DIR, PLAN_LOOP, SINGLE_REPO_REASON,
    Store, machine_for,
};
use crate::{Error, Outcome, Warning, gofmt, lock, repos, settings};

/// What `change new` created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewChange {
    pub change: Change,
    /// The intake source's id.
    pub source_id: i64,
}

/// `change new`'s result, and whether it created the plan room on the way:
/// the room is reported even when a later step fails, as Go does.
#[derive(Debug)]
pub struct ChangeNewResult {
    pub room_created: bool,
    pub result: Result<Outcome<NewChange>, Error>,
}

/// What `change new` takes.
#[derive(Debug, Clone, Default)]
pub struct ChangeNewOptions {
    pub title: String,
    /// `--repo` values: registered names, or one path.
    pub repos: Vec<String>,
    pub rev: String,
    /// The intake's body, as bytes: a promoted idea's body is copied as it is.
    pub body: crate::text::Text,
    /// The idea it's promoted from, 0 for none; recorded in the intake.
    pub idea: i64,
    /// The version the CLI reports, for the room's manifest.
    pub version: String,
}

/// The active change, as `status` shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveStatus {
    pub change: Change,
    /// Its machine's phases, in order.
    pub phases: Vec<&'static str>,
    /// The plan it belongs to, or a note when that isn't known.
    pub initiative: String,
    /// Each repo and its local path, `None` when it isn't set.
    pub code: Vec<(String, Option<String>)>,
    /// A build's ticket: id, title, and the branch name for it.
    pub ticket: Option<(String, String, String)>,
}

/// One repo's builds on the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoBuilds {
    pub repo: String,
    /// The build holding the repo's lock, if any.
    pub holder: Option<String>,
    pub builds: Vec<Change>,
}

/// `status`: the active change, the `vcs` setting, and the board of open changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub vcs: String,
    pub active: Option<ActiveStatus>,
    /// No change is recorded at all.
    pub no_changes: bool,
    pub plans: Vec<Change>,
    pub builds: Vec<RepoBuilds>,
}

/// `status <plan>`: a plan's progress and its builds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStatus {
    pub plan: Change,
    pub rows: Vec<BuildRow>,
}

impl crate::Workspace {
    fn reader(&self) -> Store<'_> {
        Store::reader(self.root(), self.env())
    }

    /// Go's `gatePolicyAll`: under `gates: block` without `--force`, the
    /// violations refuse the transition together; otherwise each is a warning.
    pub(crate) fn policy(
        &self,
        violations: Vec<String>,
        force: bool,
    ) -> Result<Vec<Warning>, Error> {
        let loaded = settings::load(self.root(), &self.env().dirs)?;
        if loaded.effective.gates == "block" && !force {
            return Err(Error::GateRefused { violations });
        }
        Ok(violations
            .into_iter()
            .map(|message| Warning { message })
            .collect())
    }

    /// Go's `lockViolation`: the lock's refusal when moving `c` to `phase` takes
    /// a repo another build holds.
    fn lock_violation(
        &self,
        store: &Store,
        c: &Change,
        phase: &str,
    ) -> Result<Option<String>, Error> {
        if !lock::enters(c, phase) {
            return Ok(None);
        }
        let changes = store.read_changes()?;
        Ok(lock::holder(&changes, c).map(|h| {
            Error::RepoLocked {
                repo: c.repos[0].clone(),
                holder: h.name.clone(),
                phase: h.phase.clone(),
                status: h.status.clone(),
            }
            .to_string()
        }))
    }

    /// `daiku gate <phase> <status>`: Go's `runGate` after parsing.
    pub fn gate(
        &self,
        phase: &str,
        status: &str,
        reason: &str,
        force: bool,
    ) -> Result<Outcome<Change>, Error> {
        let reader = self.reader();
        let st = reader.load_state()?;
        let mut notes = Vec::new();
        let mut warnings = Vec::new();
        if !st.name.is_empty() {
            overview::check(&reader, &st)?;
            let machine = machine_for(&st.loop_);
            let mut probe = st.clone();
            machine.set_gate(&mut probe, phase, status, reason)?;
            let mut violations = Vec::new();
            if let Some(v) = machine.check_gate(&st, phase, status) {
                notes.push(format!("override: {v}"));
                violations.push(v.to_string());
            }
            if let Some(msg) = self.lock_violation(&reader, &st, phase)? {
                notes.push(format!("override: {msg}"));
                violations.push(msg);
            }
            if st.loop_ != BUILD_LOOP {
                let adr_dir = settings::load(self.root(), &self.env().dirs)?
                    .effective
                    .adr_dir;
                let problems = check_artifacts(self.root(), &st.name, phase, status, &adr_dir);
                if !problems.is_empty() {
                    let mut lines = vec![format!(
                        "daiku: {phase} {status}: {} artifact problem(s):",
                        problems.len()
                    )];
                    for p in &problems {
                        lines.push(format!("  {p}"));
                        notes.push(format!("override: {p}"));
                    }
                    violations.push(lines.join("\n"));
                }
            }
            if !violations.is_empty() {
                warnings = self.policy(violations, force)?;
            }
        }
        let store = Store::at(self.root(), self.env())?;
        let c = store.advance(phase, status, reason, &notes)?;
        warnings.extend(store.take_warnings());
        Ok(Outcome { value: c, warnings })
    }

    /// `daiku reopen <phase>`: Go's `runReopen` after parsing.
    pub fn reopen(&self, phase: &str, reason: &str, force: bool) -> Result<Outcome<Change>, Error> {
        let reader = self.reader();
        let active = reader.load_state()?;
        if !active.name.is_empty() {
            overview::check(&reader, &active)?;
        }
        let mut notes = Vec::new();
        let mut warnings = Vec::new();
        if let Some(msg) = self.lock_violation(&reader, &active, phase)? {
            warnings = self.policy(vec![msg.clone()], force)?;
            notes.push(format!("override: {msg}"));
        }
        let store = Store::at(self.root(), self.env())?;
        let c = store.reopen(phase, reason, &notes)?;
        warnings.extend(store.take_warnings());
        Ok(Outcome { value: c, warnings })
    }

    /// `daiku change close <change>`: Go's `runChangeClose` after parsing.
    pub fn change_close(&self, name: &str, reason: &str) -> Result<Outcome<Change>, Error> {
        overview::check_close(&self.reader(), name)?;
        let store = Store::at(self.root(), self.env())?;
        let c = store.close(name, reason)?;
        Ok(Outcome {
            value: c,
            warnings: store.take_warnings(),
        })
    }

    /// Go's `changeRepos`: which `--repo` values are registered. One value that
    /// isn't is a path; several must all be registered.
    fn change_repos(&self, args: &[String]) -> Result<Vec<String>, Error> {
        let reg = repos::load(self.root())?;
        let mut registered = Vec::new();
        for a in args {
            if reg.repos.contains_key(a) {
                registered.push(a.clone());
            } else if args.len() > 1 {
                return Err(Error::RepoNotRegistered { name: a.clone() });
            }
        }
        Ok(registered)
    }

    /// `daiku change new <title>`: Go's `newChange`. Everything is checked and
    /// allocated before the first write; the intake is removed if seeding fails.
    pub fn change_new(&self, opts: &ChangeNewOptions) -> ChangeNewResult {
        let mut room_created = false;
        let result = self.change_new_inner(opts, &mut room_created);
        ChangeNewResult {
            room_created,
            result,
        }
    }

    fn change_new_inner(
        &self,
        opts: &ChangeNewOptions,
        room: &mut bool,
    ) -> Result<Outcome<NewChange>, Error> {
        let registered = self.change_repos(&opts.repos)?;
        let repo = opts.repos.first().cloned().unwrap_or_default();
        let reader = self.reader();
        reader.load_state()?;
        let loaded = settings::load(self.root(), &self.env().dirs)?;
        let source_dir = loaded.effective.source_dir.clone();
        let recorded: Vec<String> = reader
            .read_changes()?
            .into_values()
            .flat_map(|c| c.sources)
            .collect();
        let floor = source::max_recorded_id(&source_dir, &recorded);
        let id = reader.next_plan_id()?;
        let mut slug = state::slugify(&opts.title);
        if slug.is_empty() {
            slug = "change".into();
        }
        let name = format!("{id}-{slug}");
        let kind = if repo.is_empty() {
            source::ISSUE
        } else {
            source::REPO
        };
        *room = self.ensure_room("plan", &opts.version)?.value.created;
        let created = source::write_new(
            self.root(),
            &source_dir,
            &NewSource {
                title: opts.title.clone(),
                source: kind.into(),
                repo: repo.clone(),
                rev: opts.rev.clone(),
                body: opts.body.clone(),
                idea: opts.idea,
                after: floor,
            },
        )?;
        let rel = gofmt::join(&[source_dir.as_str(), &format!("{}.md", created.id)]);
        let mut skip = Vec::new();
        if repo.is_empty() {
            skip.push(("explore", NO_REPO_REASON));
        }
        if registered.len() < 2 {
            skip.push(("synthesis", SINGLE_REPO_REASON));
        }
        let seeded = Store::at(self.root(), self.env()).and_then(|store| {
            let c = store.seed(
                &name,
                PLAN_LOOP,
                SeedOptions {
                    sources: vec![rel.clone()],
                    repos: registered,
                    skip,
                    paths: vec![rel.clone()],
                    refresh: false,
                },
            )?;
            Ok((c, store.take_warnings()))
        });
        match seeded {
            Ok((change, warnings)) => Ok(Outcome {
                value: NewChange {
                    change,
                    source_id: created.id,
                },
                warnings,
            }),
            Err(e) => {
                // Only a race or an I/O fault gets here; don't leave an intake that
                // no change records.
                let _ = std::fs::remove_file(at(self.root(), &rel));
                Err(e)
            }
        }
    }

    /// `daiku switch` with no change: every change, as Go's `KnownChanges`.
    pub fn known_changes(&self) -> Result<Outcome<Vec<Known>>, Error> {
        self.reader().known().map(Outcome::new)
    }

    /// `daiku switch <change>`.
    pub fn switch(&self, name: &str) -> Result<Outcome<Change>, Error> {
        self.reader().switch(name).map(Outcome::new)
    }

    /// `daiku status`: Go's `runStatus` without arguments.
    pub fn status(&self) -> Result<Outcome<Status>, Error> {
        let loaded = settings::load(self.root(), &self.env().dirs)?;
        let reader = self.reader();
        let st = reader.load_state()?;
        let changes = reader.read_changes()?;
        let mut warnings = Vec::new();
        let active = if st.name.is_empty() {
            None
        } else {
            warnings = self.skipped_explore(&st);
            Some(self.active_status(&reader, &changes, st.clone()))
        };
        let mut plans = Vec::new();
        let mut by_repo: BTreeMap<String, Vec<Change>> = BTreeMap::new();
        for c in changes.values() {
            if c.is_closed() {
                continue;
            }
            if c.loop_ == BUILD_LOOP {
                let repo = c
                    .repos
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "(no repo)".into());
                by_repo.entry(repo).or_default().push(c.clone());
            } else {
                plans.push(c.clone());
            }
        }
        let builds = by_repo
            .into_iter()
            .map(|(repo, builds)| RepoBuilds {
                holder: builds
                    .iter()
                    .find(|c| lock::holds(c))
                    .map(|c| c.name.clone()),
                repo,
                builds,
            })
            .collect();
        Ok(Outcome {
            value: Status {
                vcs: loaded.vcs,
                active,
                no_changes: changes.is_empty(),
                plans,
                builds,
            },
            warnings,
        })
    }

    fn active_status(
        &self,
        reader: &Store,
        changes: &BTreeMap<String, Change>,
        c: Change,
    ) -> ActiveStatus {
        let ticket = if c.loop_ == BUILD_LOOP {
            build_ticket(self.root(), &c)
        } else {
            None
        };
        let initiative = if c.loop_ != BUILD_LOOP {
            c.name.clone()
        } else {
            let p = plan_of(reader.root, changes, &c);
            if !p.is_empty() {
                p
            } else {
                match &ticket {
                    Some(t) if !t.change.is_empty() => {
                        format!("{} (not in this workspace)", t.change)
                    }
                    _ => "-".into(),
                }
            }
        };
        let reg = repos::load(self.root());
        let code = c
            .repos
            .iter()
            .map(|n| {
                let path = reg
                    .as_ref()
                    .ok()
                    .and_then(|r| r.resolve(n, &self.env().dirs).ok())
                    .map(|p| p.to_string_lossy().into_owned());
                (n.clone(), path)
            })
            .collect();
        let ticket = ticket.map(|t| {
            let mut branch = t.id.to_lowercase();
            let slug = state::slugify(&t.title);
            if !slug.is_empty() {
                branch = format!("{branch}-{slug}");
            }
            (t.id, t.title, branch)
        });
        ActiveStatus {
            phases: machine_for(&c.loop_).names().collect(),
            change: c,
            initiative,
            code,
            ticket,
        }
    }

    /// Go's `warnSkippedExplore`: a change whose intake is a repo but whose
    /// explore phase is skipped.
    fn skipped_explore(&self, c: &Change) -> Vec<Warning> {
        let mut out = Vec::new();
        for src in &c.sources {
            let (dir, base) = match src.rsplit_once('/') {
                Some((d, b)) => (d, b),
                None => (".", src.as_str()),
            };
            let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base);
            let Ok(id) = stem.parse::<i64>() else {
                continue;
            };
            let Ok(s) = source::read_source(self.root(), dir, id) else {
                continue;
            };
            if s.source == source::REPO && c.gate("explore") == SKIPPED {
                out.push(Warning {
                    message: format!(
                        "source #{id} is a repo ({}) but the explore phase is skipped",
                        s.title
                    ),
                });
            }
        }
        out
    }

    /// `daiku status <plan>`: Go's `runInitiative`.
    pub fn status_plan(&self, name: &str) -> Result<Outcome<PlanStatus>, Error> {
        let reader = self.reader();
        let changes = reader.read_changes()?;
        let is_plan = !name.is_empty()
            && gofmt::is_local(name)
            && !name.contains(['/', '\\'])
            && changes.get(name).is_some_and(|c| c.loop_ == PLAN_LOOP)
            && std::fs::metadata(at(self.root(), &format!("{PLAN_DIR}/{name}")))
                .is_ok_and(|m| m.is_dir());
        if !is_plan {
            return Err(Error::NotAPlan {
                name: name.to_string(),
                build: changes.get(name).is_some_and(|c| c.loop_ == BUILD_LOOP),
            });
        }
        let rows = build_rows(self.root(), name, &changes)?;
        Ok(Outcome::new(PlanStatus {
            plan: changes[name].clone(),
            rows,
        }))
    }
}
