//! The repo registry's commands and the build room (S-0008): Go's
//! `internal/core/repos` `Run` and `RunEnv`, `internal/cli/build.go` and
//! `internal/cli/ship.go`, without argument parsing or printing. Each command is a
//! `Workspace` method that returns data; what Go prints in the middle of an
//! operation (clone results, the doctor's findings) comes back as steps in order.

use serde::Serialize;

use crate::env::{Cmd, spawn_failure};
use crate::outcome::Outcome;
use crate::repos::{self, RepoProblem};
use crate::state::store::{SeedOptions, Store, at, shown};
use crate::state::{BUILD_LOOP, Change, overview};
use crate::ticket::{self, Ticket};
use crate::{Error, Event, gofmt, inbox};

/// One line of `repo list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepoRow {
    pub name: String,
    pub remote: String,
    pub branch: String,
    /// The path as stored in `paths.json` (a `~/` path unexpanded), if any.
    pub path: Option<String>,
    /// Archived: kept in the registry, but not cloned or given new work.
    pub archived: bool,
    /// Why it was archived, or empty.
    pub archived_reason: String,
}

/// What `repo doctor --clone` did for one repo, in the order Go printed it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CloneStep {
    /// No local path and no `--into`: Go prints a hint to stderr and moves on.
    NoPath { repo: String },
    /// The clone failed: Go prints the error to stderr and moves on.
    Failed { repo: String, error: String },
    /// Cloned, and the registry saved: Go prints `cloned <repo> to <path>`.
    Cloned { repo: String, path: String },
    /// The clone worked but the registry couldn't be saved: Go prints the error
    /// and stops, exit 1, without running the doctor.
    SaveFailed { error: String },
}

/// `repo doctor`'s result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub clones: Vec<CloneStep>,
    pub problems: Vec<RepoProblem>,
    /// The number of registered repos, for `<n> repos ok`.
    pub count: usize,
    /// A [`CloneStep::SaveFailed`] ended the run before the doctor.
    pub aborted: bool,
}

/// What `build new` made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NewBuild {
    pub id: String,
    pub repo: String,
    /// The ticket's path, relative to the workspace, as recorded in the change.
    pub rel: String,
}

/// `build new`'s result, and whether it created the build room on the way: the
/// room is reported even when a later step fails, as Go does.
#[derive(Debug)]
pub struct BuildNewResult {
    pub room_created: bool,
    pub result: Result<Outcome<NewBuild>, Error>,
}

/// A program for the client to run with the terminal attached: `git commit …` or
/// `gh pr create …`, in the build's repo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShipCommand {
    pub dir: String,
    pub program: String,
    pub args: Vec<String>,
}

/// Go's `PRBody`: links a pull request to its ticket: id and title, each
/// external item, where it was planned, the plan change's folder for a planned
/// ticket read in place, the spec and the ADRs.
pub fn pr_body(t: &Ticket, plan_folder: &str) -> String {
    let mut lines = vec![format!("Implements {}: {}", t.id, t.title), String::new()];
    for (tool, item) in &t.external {
        lines.push(format!("- **{tool}:** {item}"));
    }
    if !t.origin.workspace.is_empty() {
        lines.push(format!(
            "- **Planned in:** {} (`{}`)",
            t.origin.workspace, t.origin.path
        ));
    }
    if !plan_folder.is_empty() {
        lines.push(format!(
            "- **Plan:** `{}/`",
            plan_folder.trim_end_matches('/')
        ));
    }
    if !t.spec.is_empty() {
        lines.push(format!("- **Spec:** `{}`", t.spec));
    }
    if !t.adrs.is_empty() {
        lines.push(format!("- **ADRs:** {}", t.adrs.join(", ")));
    }
    format!("{}\n", lines.join("\n").trim_end_matches('\n'))
}

/// Go's `planFolder`: the plan change folder of a build whose ticket is planned,
/// read in place from `plan/specs/<change>/tickets/`, or `""` for an inbox ticket.
fn plan_folder(c: &Change) -> String {
    let prefix = format!("{}/", crate::plan::SPECS_DIR);
    for src in &c.sources {
        let dir = gofmt::clean(src.rsplit_once('/').map_or("", |(d, _)| d));
        if src.starts_with(&prefix) && dir.rsplit('/').next() == Some("tickets") {
            return dir
                .rsplit_once('/')
                .map_or_else(|| ".".to_string(), |(d, _)| d.to_string());
        }
    }
    String::new()
}

/// Go's `plannedID`: an E-, F-, S- or T- id.
fn planned_id(id: &str) -> bool {
    let prefix = id.split('-').next().unwrap_or("");
    prefix != ticket::NATIVE_PREFIX && prefix != ticket::PULLED_PREFIX
}

fn is_file(path: &std::path::Path) -> bool {
    std::fs::metadata(path).is_ok_and(|m| m.is_file())
}

impl crate::Workspace {
    /// The registry, loaded as Go's `repos.Load` does. `repo` loads it before it
    /// reads the verb, so a corrupt file is reported first.
    pub fn registry(&self) -> Result<repos::Registry, Error> {
        repos::load(self.root())
    }

    /// `daiku repo add`: registers a repo, records its local path when one is
    /// given, and saves. Returns the name.
    pub fn repo_add(
        &self,
        name: &str,
        remote: &str,
        path: Option<&str>,
        branch: &str,
    ) -> Result<Outcome<String>, Error> {
        let mut reg = self.registry()?;
        reg.add(name, remote, branch)?;
        if let Some(path) = path.filter(|p| !p.is_empty()) {
            reg.set_path(name, path, &self.env().dirs)?;
        }
        reg.save(self.root())?;
        Ok(Outcome::new(name.to_string()))
    }

    /// `daiku repo list`: each repo, sorted by name.
    pub fn repo_list(&self) -> Result<Outcome<Vec<RepoRow>>, Error> {
        let reg = self.registry()?;
        Ok(Outcome::new(
            reg.names()
                .into_iter()
                .map(|name| {
                    let repo = &reg.repos[&name];
                    RepoRow {
                        remote: repos::redact_remote(&repo.remote),
                        branch: repo.default_branch.clone(),
                        path: reg.paths.get(&name).cloned(),
                        archived: repo.archived,
                        archived_reason: repo.archived_reason.clone(),
                        name,
                    }
                })
                .collect(),
        ))
    }

    /// `daiku repo path <name> <path>`: records the path and returns it as stored.
    /// `daiku repo archive <name> [--reason <r>]` (N-0008).
    pub fn repo_archive(&self, name: &str, reason: &str) -> Result<Outcome<String>, Error> {
        let mut reg = self.registry()?;
        reg.archive(name, reason)?;
        reg.save(self.root())?;
        Ok(Outcome::new(name.to_string()))
    }

    /// `daiku repo unarchive <name>` (N-0008).
    pub fn repo_unarchive(&self, name: &str) -> Result<Outcome<String>, Error> {
        let mut reg = self.registry()?;
        reg.unarchive(name)?;
        reg.save(self.root())?;
        Ok(Outcome::new(name.to_string()))
    }

    pub fn repo_path(&self, name: &str, path: &str) -> Result<Outcome<String>, Error> {
        let mut reg = self.registry()?;
        reg.set_path(name, path, &self.env().dirs)?;
        reg.save(self.root())?;
        Ok(Outcome::new(reg.paths[name].clone()))
    }

    /// `daiku repo doctor [--clone] [--into <dir>]`: Go's `runDoctor`. With
    /// `clone`, each repo that isn't a folder on disk is cloned first, and
    /// `Env.events` hears a start and an end for every clone attempted (R7).
    pub fn repo_doctor(
        &self,
        clone: bool,
        into: Option<&str>,
    ) -> Result<Outcome<DoctorReport>, Error> {
        let mut reg = self.registry()?;
        let env = self.env();
        let mut clones = Vec::new();
        if clone {
            for name in reg.names() {
                // An archived repo is never cloned (N-0008).
                if reg.is_archived(&name) {
                    continue;
                }
                let recorded = reg.paths.get(&name).cloned();
                if let Ok(path) = reg.resolve(&name, &env.dirs)
                    && std::fs::metadata(&path).is_ok_and(|m| m.is_dir())
                {
                    continue;
                }
                let dest = match (recorded, into.filter(|i| !i.is_empty())) {
                    (Some(dest), _) => dest,
                    (None, Some(into)) => gofmt::join(&[into, name.as_str()]),
                    (None, None) => {
                        clones.push(CloneStep::NoPath { repo: name });
                        continue;
                    }
                };
                env.events.emit(Event::CloneStarted {
                    repo: name.clone(),
                    dest: dest.clone(),
                });
                let result = reg.clone_repo(&name, &dest, &env.dirs, &*env.process);
                env.events.emit(Event::CloneFinished {
                    repo: name.clone(),
                    dest: dest.clone(),
                    error: result.as_ref().err().map(ToString::to_string),
                });
                if let Err(e) = result {
                    clones.push(CloneStep::Failed {
                        repo: name,
                        error: e.to_string(),
                    });
                    continue;
                }
                if let Err(e) = reg.save(self.root()) {
                    clones.push(CloneStep::SaveFailed {
                        error: e.to_string(),
                    });
                    return Ok(Outcome::new(DoctorReport {
                        clones,
                        problems: Vec::new(),
                        count: reg.live_count(),
                        aborted: true,
                    }));
                }
                let path = reg.paths[&name].clone();
                clones.push(CloneStep::Cloned { repo: name, path });
            }
        }
        let problems = reg.doctor(&env.dirs, &*env.process);
        Ok(Outcome::new(DoctorReport {
            clones,
            problems,
            count: reg.live_count(),
            aborted: false,
        }))
    }

    /// `daiku env`: the active change's repos as harness flags, or an empty string
    /// when it has none (Go's `runEnv` and `RunEnv`).
    pub fn env_flags(&self) -> Result<Outcome<String>, Error> {
        let st = Store::reader(self.root(), self.env()).load_state()?;
        if st.name.is_empty() {
            return Err(Error::NoActiveChange);
        }
        let reg = self.registry()?;
        reg.env_flags(&st.repos, &self.env().dirs).map(Outcome::new)
    }

    /// `daiku env --json`: the flags as a core view.
    pub fn env_view(&self) -> Result<Outcome<crate::json::EnvFlags>, Error> {
        self.env_flags().map(|o| Outcome {
            value: crate::json::EnvFlags { flags: o.value },
            warnings: o.warnings,
        })
    }
}

impl crate::Workspace {
    /// `daiku build new <ticket-id>`: Go's `buildNew` after parsing. It finds the
    /// ticket where it lives, checks it can be built, creates the build room the
    /// first time, and seeds the change named by the ticket's id in its repo. The
    /// ticket is never copied.
    pub fn build_new(&self, arg: &str, version: &str) -> BuildNewResult {
        let mut room_created = false;
        let result = self.build_new_steps(arg, version, &mut room_created);
        BuildNewResult {
            room_created,
            result,
        }
    }

    fn build_new_steps(
        &self,
        arg: &str,
        version: &str,
        room_created: &mut bool,
    ) -> Result<Outcome<NewBuild>, Error> {
        // Go's `NormalizeID`: a bare number is a native ticket's.
        let id = match ticket::go_atoi(arg) {
            Some(n) => format!("{}-{n:04}", ticket::NATIVE_PREFIX),
            None => arg.to_string(),
        };
        if !ticket::ID_RE.is_match(&id) {
            return Err(Error::NotATicketId { arg: arg.into() });
        }
        let reader = Store::reader(self.root(), self.env());
        if reader.read_changes()?.contains_key(&id) {
            return Err(Error::BuildExists { id });
        }
        let (t, rel) = self.find_build_ticket(&reader, &id)?;
        self.check_buildable(&t)?;
        // Refuse unsafe state files, and a plan overview that can't be written
        // safely, before the room or the change folder is written.
        reader.check_state_files()?;
        let probe = Change {
            name: id.clone(),
            loop_: BUILD_LOOP.to_string(),
            sources: vec![rel.clone()],
            ..Change::default()
        };
        overview::check(&reader, &probe)?;
        *room_created = self.ensure_room("build", version)?.value.created;
        let store = Store::at(self.root(), self.env())?;
        store.seed(
            &id,
            BUILD_LOOP,
            SeedOptions {
                sources: vec![rel.clone()],
                repos: vec![t.repo.clone()],
                paths: vec![rel.clone()],
                refresh: true,
                ..SeedOptions::default()
            },
        )?;
        Ok(Outcome {
            value: NewBuild {
                id,
                repo: t.repo,
                rel,
            },
            warnings: store.take_warnings(),
        })
    }

    /// Go's `findBuildTicket`: ticket `id` in a plan change's `tickets/` folder,
    /// then in the inbox, and its path relative to the root. An id found in both
    /// places, or a planned id in the inbox, is an error.
    fn find_build_ticket(&self, store: &Store, id: &str) -> Result<(Ticket, String), Error> {
        let root = self.root();
        let mut found: Vec<String> = Vec::new();
        for c in store.plan_changes()? {
            let rel = format!("{}/{}/tickets/{id}.md", crate::plan::SPECS_DIR, c.name);
            if is_file(&at(root, &rel)) {
                found.push(rel);
            }
        }
        let inbox_rel = inbox::rel(id);
        if is_file(&at(root, &inbox_rel)) {
            if planned_id(id) {
                return Err(Error::PlannedInInbox { id: id.into() });
            }
            found.push(inbox_rel);
        }
        let rel = match found.as_slice() {
            [] => return Err(Error::NoBuildTicket { id: id.into() }),
            [one] => one.clone(),
            many => {
                return Err(Error::TicketInManyPlaces {
                    id: id.into(),
                    places: many.join(", "),
                });
            }
        };
        let path = at(root, &rel);
        let data = std::fs::read(&path).map_err(|source| Error::Io {
            op: "open",
            path: shown(root, &rel),
            source,
        })?;
        let mut t = Ticket::parse(&data).map_err(|e| Error::BuildTicketParse {
            rel: rel.clone(),
            cause: e.to_string(),
        })?;
        t.path = shown(root, &rel);
        if t.id != id {
            return Err(Error::TicketIdMismatch {
                rel,
                found: t.id,
                id: id.into(),
            });
        }
        Ok((t, rel))
    }

    /// Go's `checkBuildable`: refuses a ticket with no repo or an unregistered one,
    /// one that fails the schema, and a planned ticket that hasn't been readied or
    /// published.
    fn check_buildable(&self, t: &Ticket) -> Result<(), Error> {
        if let Some(p) = ticket::validate_file(t).first() {
            return Err(Error::InvalidBuildTicket {
                problem: p.to_string(),
            });
        }
        if planned_id(&t.id)
            && t.status != ticket::STATUS_READY
            && t.status != ticket::STATUS_PUBLISHED
        {
            return Err(Error::NotBuildable {
                id: t.id.clone(),
                status: t.status.clone(),
            });
        }
        if t.repo.is_empty() {
            return Err(Error::NoBuildRepo { id: t.id.clone() });
        }
        let reg = repos::load(self.root())?;
        if !reg.repos.contains_key(&t.repo) {
            return Err(Error::BuildRepoNotRegistered {
                id: t.id.clone(),
                repo: t.repo.clone(),
            });
        }
        if reg.is_archived(&t.repo) {
            return Err(Error::RepoIsArchived {
                name: t.repo.clone(),
            });
        }
        Ok(())
    }
}

/// A run attached to the terminal, as Go words its result.
fn attached_result(program: &str, r: std::io::Result<i32>) -> Result<(), Error> {
    match r {
        Ok(0) => Ok(()),
        Ok(-1) => Err(Error::ShipRun {
            text: "signal: killed".into(),
        }),
        Ok(code) => Err(Error::ShipRun {
            text: format!("exit status {code}"),
        }),
        Err(e) => Err(Error::ShipRun {
            text: spawn_failure(program, &e),
        }),
    }
}

impl crate::Workspace {
    /// Go's `activeBuild` with `ship.RepoDir`: the active build and its repo's
    /// local path. `commit` and `pr` act only on a build, and only in its
    /// registered git checkout.
    fn active_build(&self) -> Result<(Change, std::path::PathBuf), Error> {
        let st = Store::reader(self.root(), self.env()).load_state()?;
        if st.name.is_empty() || st.loop_ != BUILD_LOOP || st.repos.is_empty() {
            return Err(Error::NoActiveBuild);
        }
        let dir = self.repo_dir(&st.repos[0])?;
        Ok((st, dir))
    }

    /// Go's `RepoDir`: the repo's local path, which must be a folder inside a git
    /// work tree, so `commit` and `pr` never run anywhere else.
    fn repo_dir(&self, repo: &str) -> Result<std::path::PathBuf, Error> {
        let env = self.env();
        let dir = repos::load(self.root())?.resolve(repo, &env.dirs)?;
        let shown = dir.to_string_lossy().into_owned();
        if !std::fs::metadata(&dir).is_ok_and(|m| m.is_dir()) {
            return Err(Error::NoUsablePath {
                repo: repo.into(),
                dir: shown,
            });
        }
        let probe = Cmd::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .dir(&dir);
        match env.process.output(&probe) {
            Ok(o) if o.code == Some(0) && String::from_utf8_lossy(&o.stdout).trim() == "true" => {
                Ok(dir)
            }
            _ => Err(Error::NotACheckout {
                repo: repo.into(),
                dir: shown,
            }),
        }
    }

    /// `daiku commit [git commit args]`: Go's `runCommit`. It commits what's
    /// staged in the active build's repo, and refuses when nothing is staged, so
    /// each commit is one reviewed group. `git commit` runs with the terminal
    /// attached.
    pub fn commit(&self, args: &[String]) -> Result<Outcome<()>, Error> {
        let (_, dir) = self.active_build()?;
        let env = self.env();
        // `git diff --cached --quiet` exits 0 when nothing is staged.
        let diff = Cmd::new("git")
            .args(["diff", "--cached", "--quiet"])
            .dir(&dir);
        if env.process.output(&diff).is_ok_and(|o| o.code == Some(0)) {
            return Err(Error::NothingStaged);
        }
        let commit = Cmd::new("git")
            .args(std::iter::once("commit").chain(args.iter().map(String::as_str)))
            .dir(&dir);
        attached_result("git", env.process.run_attached(&commit))?;
        Ok(Outcome::new(()))
    }

    /// `daiku pr [title]`, first half: Go's `runPR` up to running gh. It reads
    /// the build's ticket and builds `gh pr create`'s arguments. A branch that
    /// doesn't include the ticket's id is a warning, which a client shows before
    /// it runs the command with [`Workspace::run_ship`].
    pub fn pr_command(&self, args: &[String]) -> Result<Outcome<ShipCommand>, Error> {
        let (st, dir) = self.active_build()?;
        let env = self.env();
        let ticket = overview::build_ticket(self.root(), &st);
        let mut gh = vec!["pr".to_string(), "create".to_string()];
        let mut title = args
            .first()
            .filter(|a| !a.starts_with('-'))
            .cloned()
            .unwrap_or_default();
        let mut warnings = Vec::new();
        if let Some(t) = &ticket {
            if title.is_empty() {
                title = format!("[{}] {}", t.id, t.title);
            }
            gh.push("--body".into());
            gh.push(pr_body(t, &plan_folder(&st)));
            let branch = Cmd::new("git").args(["branch", "--show-current"]).dir(&dir);
            if let Ok(o) = env.process.output(&branch)
                && o.code == Some(0)
            {
                let branch = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if !branch.to_lowercase().contains(&t.id.to_lowercase()) {
                    warnings.push(crate::Warning {
                        message: format!(
                            "branch {branch} doesn't include {} — `daiku status` shows the suggested name",
                            t.id
                        ),
                    });
                }
            }
        }
        if !title.is_empty() {
            gh.push("--title".into());
            gh.push(title);
        }
        Ok(Outcome {
            value: ShipCommand {
                dir: dir.to_string_lossy().into_owned(),
                program: "gh".into(),
                args: gh,
            },
            warnings,
        })
    }

    /// Runs a [`ShipCommand`] with the terminal attached, as Go's `ship.Runner`
    /// does, with no shell.
    pub fn run_ship(&self, cmd: &ShipCommand) -> Result<(), Error> {
        let c = Cmd::new(cmd.program.as_str())
            .args(cmd.args.iter().map(String::as_str))
            .dir(&cmd.dir);
        attached_result(&cmd.program, self.env().process.run_attached(&c))
    }
}

#[cfg(test)]
mod tests;
