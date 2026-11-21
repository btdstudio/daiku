//! The store: every change's record in `STATE.json`, the active pointer, change
//! folders and ids, and transitions recorded in `LOG.md` and committed with git.
//! A port of Go's `internal/core/state/store.go` as `internal/state` configures it.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::file::{self, Written, WrittenFile};
use super::machine::{DRAFT, SKIPPED};
use super::{
    BUILD_LOOP, CHANGE_ID_RE, Change, PLAN_CHANGE_DIRS, PLAN_DIR, PLAN_LOOP, PLAN_NAME_RE,
};
use crate::env::Cmd;
use crate::settings::{self, Loaded};
use crate::{Env, Error, Warning, gofmt, json, repos, scaffold, ticket, write};

/// A change as `switch` lists it: Go's `KnownChange`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Known {
    pub name: String,
    #[serde(rename = "loop")]
    pub loop_: String,
    /// The folder's leading number; 0 for a change without one.
    pub id: u64,
    pub phase: String,
    pub status: String,
    pub gates: BTreeMap<String, String>,
    pub repos: Vec<String>,
}

/// What `seed` writes besides the change itself.
#[derive(Debug, Clone, Default)]
pub struct SeedOptions {
    pub sources: Vec<String>,
    pub repos: Vec<String>,
    /// Phases to record as skipped, with their reasons.
    pub skip: Vec<(&'static str, &'static str)>,
    /// More paths to commit with the change (its intake).
    pub paths: Vec<String>,
    /// Refresh the overview after seeding (a build's plan).
    pub refresh: bool,
}

/// The store for one workspace. Reads need no settings; transitions carry the
/// `vcs` and `overview` settings (Go's `storeAt`).
pub struct Store<'a> {
    pub root: &'a Path,
    pub env: &'a Env,
    settings: Option<Loaded>,
    warnings: RefCell<Vec<Warning>>,
}

/// Go's `os.ReadFile`, with the error worded as Go's `*PathError`: `None` when the
/// file doesn't exist.
pub fn read_file(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let shown = gofmt::to_slash(&path.to_string_lossy());
    let mut f = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("open {shown}: {}", gofmt::io_error_text(&e))),
    };
    let mut data = Vec::new();
    f.read_to_end(&mut data)
        .map_err(|e| format!("read {shown}: {}", gofmt::io_error_text(&e)))?;
    Ok(Some(data))
}

/// Go's `os.ReadFile` where a missing file is an error too: `open <path>: …` when
/// it can't be opened, `read <path>: …` when it can but not read (a folder).
pub fn read_all(path: &Path) -> Result<Vec<u8>, Error> {
    match read_file(path) {
        Ok(Some(data)) => Ok(data),
        Ok(None) => Err(Error::PathError {
            text: format!(
                "open {}: no such file or directory",
                gofmt::to_slash(&path.to_string_lossy())
            ),
        }),
        Err(text) => Err(Error::PathError { text }),
    }
}

/// Go's `realDir`: a folder that isn't a symlink.
pub fn real_dir(p: &Path) -> bool {
    std::fs::symlink_metadata(p).is_ok_and(|m| m.is_dir())
}

/// `rel` joined to `root` for messages, slash-separated, as Go's `filepath.Join`.
pub fn shown(root: &Path, rel: &str) -> String {
    gofmt::join(&[gofmt::to_slash(&root.to_string_lossy()).as_str(), rel])
}

/// `rel` (slash-separated) joined to `root`.
///
/// As with Go's `filepath.Join`, a root of `.` adds nothing, so paths in messages
/// read `STATE.json`, not `./STATE.json`.
pub fn at(root: &Path, rel: &str) -> PathBuf {
    let start = if root == Path::new(".") {
        PathBuf::new()
    } else {
        root.to_path_buf()
    };
    let p = rel
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .fold(start, |p, part| p.join(part));
    if p.as_os_str().is_empty() {
        root.to_path_buf()
    } else {
        p
    }
}

fn max_id(max: u64, name: &str) -> u64 {
    CHANGE_ID_RE
        .captures(name)
        .and_then(|m| m[1].parse::<u64>().ok())
        .filter(|&n| n > max)
        .unwrap_or(max)
}

/// The log line for a transition: `<change>: <phase> → <status>[ (<reason>)]`.
pub fn transition_line(name: &str, phase: &str, status: &str, reason: &str) -> String {
    let mut line = format!("{name}: {phase} → {status}");
    if !reason.is_empty() {
        line.push_str(&format!(" ({reason})"));
    }
    line
}

impl<'a> Store<'a> {
    /// A store for reads.
    pub fn reader(root: &'a Path, env: &'a Env) -> Store<'a> {
        Store {
            root,
            env,
            settings: None,
            warnings: RefCell::new(Vec::new()),
        }
    }

    /// A store for transitions: it loads the settings first (Go's `storeAt`).
    pub fn at(root: &'a Path, env: &'a Env) -> Result<Store<'a>, Error> {
        let loaded = settings::load(root, &env.dirs)?;
        let mut s = Store::reader(root, env);
        s.settings = Some(loaded);
        Ok(s)
    }

    /// The warnings raised so far, emptied.
    pub fn take_warnings(&self) -> Vec<Warning> {
        std::mem::take(&mut self.warnings.borrow_mut())
    }

    pub(crate) fn warn(&self, message: String) {
        self.warnings.borrow_mut().push(Warning { message });
    }

    pub(crate) fn settings(&self) -> Option<&Loaded> {
        self.settings.as_ref()
    }

    fn local(&self) -> bool {
        self.settings.as_ref().is_some_and(|s| s.vcs == "none")
    }

    pub fn state_path(&self) -> String {
        shown(self.root, "STATE.json")
    }

    pub fn log_path(&self) -> String {
        shown(self.root, "LOG.md")
    }

    pub fn active_path(&self) -> String {
        shown(self.root, ".daiku/active")
    }

    /// Go's `relDir`: the change's folder relative to the root.
    pub fn rel_dir(&self, c: &Change) -> Result<String, Error> {
        let rel = if c.loop_ == BUILD_LOOP {
            if !ticket::ID_RE.is_match(&c.name) {
                return Err(Error::InvalidChangeName {
                    name: c.name.clone(),
                });
            }
            let Some(repo) = c.repos.first() else {
                return Err(Error::BuildNoRepo {
                    name: c.name.clone(),
                });
            };
            if !repos::NAME_RE.is_match(repo) {
                return Err(Error::BuildInvalidRepo {
                    name: c.name.clone(),
                    repo: repo.clone(),
                });
            }
            format!("build/{repo}/changes/{}", c.name)
        } else {
            if !PLAN_NAME_RE.is_match(&c.name) {
                return Err(Error::InvalidChangeName {
                    name: c.name.clone(),
                });
            }
            format!("{PLAN_DIR}/{}", c.name)
        };
        if !gofmt::is_local(&rel) {
            return Err(Error::ChangeFolderOutside {
                name: c.name.clone(),
                rel,
            });
        }
        Ok(gofmt::clean(&rel))
    }

    fn check_loop(&self, c: &Change) -> Result<(), Error> {
        if c.loop_ != PLAN_LOOP && c.loop_ != BUILD_LOOP {
            return Err(Error::UnknownLoopFor {
                loop_: c.loop_.clone(),
                name: c.name.clone(),
            });
        }
        Ok(())
    }

    /// Go's `ChangeDir`: the change's folder.
    pub fn change_dir(&self, c: &Change) -> Result<PathBuf, Error> {
        self.check_loop(c)?;
        Ok(at(self.root, &self.rel_dir(c)?))
    }

    /// Go's `NextID` for the plan loop: one past the highest number recorded or
    /// found in `plan/specs/`.
    pub fn next_plan_id(&self) -> Result<u64, Error> {
        let changes = self.read_changes()?;
        let mut max = changes
            .iter()
            .filter(|(_, c)| c.loop_ == PLAN_LOOP)
            .fold(0, |m, (name, _)| max_id(m, name));
        let dir = at(self.root, PLAN_DIR);
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                for e in entries.flatten() {
                    max = max_id(max, &e.file_name().to_string_lossy());
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::CannotRead {
                    path: shown(self.root, PLAN_DIR),
                    detail: format!(
                        "open {}: {}",
                        shown(self.root, PLAN_DIR),
                        gofmt::io_error_text(&e)
                    ),
                });
            }
        }
        Ok(max + 1)
    }

    /// Go's `ReadChanges`: every recorded change, by name. A missing file is no
    /// changes; a zumen or tomo workspace is refused first.
    pub fn read_changes(&self) -> Result<BTreeMap<String, Change>, Error> {
        scaffold::check_legacy(self.root)?;
        let path = self.state_path();
        let data = match read_file(&at(self.root, "STATE.json")) {
            Ok(Some(d)) => d,
            Ok(None) => return Ok(BTreeMap::new()),
            Err(detail) => return Err(Error::CannotRead { path, detail }),
        };
        let raw = file::decode(&data).map_err(|detail| Error::StateParse {
            path: path.clone(),
            detail,
        })?;
        if raw.schema > file::SCHEMA {
            return Err(Error::StateSchema {
                path,
                schema: raw.schema,
            });
        }
        let mut out = BTreeMap::new();
        for (name, c) in raw.changes.unwrap_or_default() {
            let Some(c) = c else { continue };
            if c.loop_.is_empty() {
                return Err(Error::StateNoLoop { path, name });
            }
            if c.loop_ != PLAN_LOOP && c.loop_ != BUILD_LOOP {
                return Err(Error::StateUnknownLoop {
                    path,
                    name,
                    loop_: c.loop_,
                });
            }
            let gates = c
                .gates
                .unwrap_or_default()
                .into_iter()
                .filter(|(_, s)| !s.is_empty())
                .collect();
            out.insert(
                name.clone(),
                Change {
                    name,
                    loop_: c.loop_,
                    phase: c.phase,
                    status: c.status,
                    gates,
                    reasons: c.reasons.unwrap_or_default(),
                    sources: c.sources.items().to_vec(),
                    repos: c.repos.items().to_vec(),
                    updated_at: c.updated_at,
                },
            );
        }
        Ok(out)
    }

    /// Go's `ReadActive`: the active change's name, "" when none is set.
    pub fn read_active(&self) -> Result<String, Error> {
        match read_file(&at(self.root, ".daiku/active")) {
            Ok(Some(d)) => Ok(String::from_utf8_lossy(&d).trim().to_string()),
            Ok(None) => Ok(String::new()),
            Err(detail) => Err(Error::CannotRead {
                path: self.active_path(),
                detail,
            }),
        }
    }

    fn write_active(&self, name: &str) -> Result<(), Error> {
        write::write_file(
            self.root,
            &at(self.root, ".daiku/active"),
            format!("{name}\n").as_bytes(),
        )
    }

    /// Go's `resolve`: the recorded change, or one found by its plan folder (with
    /// its loop set), or a bare change with no loop.
    pub fn resolve(&self, name: &str, changes: &BTreeMap<String, Change>) -> Change {
        if let Some(c) = changes.get(name) {
            return c.clone();
        }
        let mut c = Change::new(name);
        if PLAN_NAME_RE.is_match(name) && real_dir(&at(self.root, &format!("{PLAN_DIR}/{name}"))) {
            c.loop_ = PLAN_LOOP.to_string();
        }
        c
    }

    /// Go's `Load`: the active change, resolved; a bare change when none is set.
    pub fn load(&self) -> Result<Change, Error> {
        let changes = self.read_changes()?;
        let name = self.read_active()?;
        Ok(self.resolve(&name, &changes))
    }

    /// Go's `LoadState`: [`Store::load`], with no active change read as an empty
    /// plan change.
    pub fn load_state(&self) -> Result<Change, Error> {
        let mut c = self.load()?;
        if c.name.is_empty() && c.loop_.is_empty() {
            c.loop_ = PLAN_LOOP.to_string();
        }
        Ok(c)
    }

    fn write(&self, c: &mut Change) -> Result<(), Error> {
        c.updated_at = super::changelog::stamp(self.env.clock.now());
        let mut changes = self.read_changes()?;
        changes.insert(c.name.clone(), c.clone());
        let file = WrittenFile {
            schema: file::SCHEMA,
            changes: changes
                .iter()
                .map(|(name, c)| {
                    (
                        name.as_str(),
                        Written {
                            loop_: &c.loop_,
                            phase: &c.phase,
                            status: &c.status,
                            gates: &c.gates,
                            reasons: &c.reasons,
                            sources: &c.sources,
                            repos: &c.repos,
                            updated_at: &c.updated_at,
                        },
                    )
                })
                .collect(),
        };
        let buf = json::to_pretty(&file).expect("STATE.json's types always serialize");
        write::write_file(self.root, &at(self.root, "STATE.json"), &buf)
    }

    /// Go's `CheckStateFiles`: `STATE.json`, the active pointer and `LOG.md` must
    /// be safe to write before anything changes.
    pub fn check_state_files(&self) -> Result<(), Error> {
        for rel in ["STATE.json", ".daiku/active", "LOG.md"] {
            write::check_path(self.root, &at(self.root, rel))?;
        }
        Ok(())
    }

    /// Go's `Save`: writes the change's record and makes it active.
    pub fn save(&self, c: &mut Change) -> Result<(), Error> {
        self.check_state_files()?;
        self.write(c)?;
        self.write_active(&c.name)
    }

    /// Go's `PlanChanges`: `known()` filtered to the plan loop.
    pub fn plan_changes(&self) -> Result<Vec<Known>, Error> {
        Ok(self
            .known()?
            .into_iter()
            .filter(|k| k.loop_ == PLAN_LOOP)
            .collect())
    }

    /// Go's `Known`: build changes by name, then plan folders by number.
    pub fn known(&self) -> Result<Vec<Known>, Error> {
        let changes = self.read_changes()?;
        let mut out: Vec<Known> = changes
            .values()
            .filter(|c| c.loop_ == BUILD_LOOP)
            .map(|c| Known {
                name: c.name.clone(),
                loop_: BUILD_LOOP.to_string(),
                id: 0,
                phase: c.phase.clone(),
                status: c.status.clone(),
                gates: c.gates.clone(),
                repos: c.repos.clone(),
            })
            .collect();
        let mut plans = Vec::new();
        if let Ok(entries) = std::fs::read_dir(at(self.root, PLAN_DIR)) {
            let mut names: Vec<String> = entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            for name in names {
                let Some(m) = CHANGE_ID_RE.captures(&name) else {
                    continue;
                };
                let id = m[1].parse::<u64>().unwrap_or(0);
                let mut k = Known {
                    name: name.clone(),
                    loop_: PLAN_LOOP.to_string(),
                    id,
                    ..Known::default()
                };
                if let Some(c) = changes.get(&name).filter(|c| c.loop_ == PLAN_LOOP) {
                    k.phase = c.phase.clone();
                    k.status = c.status.clone();
                    k.gates = c.gates.clone();
                    k.repos = c.repos.clone();
                }
                plans.push(k);
            }
            plans.sort_by_key(|k| k.id);
        }
        out.extend(plans);
        Ok(out)
    }

    /// Go's `checkFolder`: the change must have a real folder where its loop
    /// puts it.
    fn check_folder(&self, c: &Change) -> Result<(), Error> {
        let unknown = || Error::UnknownChange {
            name: c.name.clone(),
        };
        self.check_loop(c).map_err(|_| unknown())?;
        let rel = self.rel_dir(c).map_err(|_| unknown())?;
        if write::through_symlink(self.root, &rel).is_some() || !real_dir(&at(self.root, &rel)) {
            return Err(unknown());
        }
        Ok(())
    }

    /// Go's `Switch`: makes `name` active, recording a found plan folder.
    pub fn switch(&self, name: &str) -> Result<Change, Error> {
        let changes = self.read_changes()?;
        let mut c = self.resolve(name, &changes);
        self.check_folder(&c)?;
        if changes.contains_key(name) {
            self.write_active(name)?;
            return Ok(c);
        }
        self.save(&mut c)?;
        Ok(c)
    }

    /// Go's `Seed`: records a new change at its first phase (with skips), makes
    /// it active, creates its folder, and records and commits it.
    pub fn seed(&self, name: &str, loop_: &str, opts: SeedOptions) -> Result<Change, Error> {
        let name_ok = if loop_ == BUILD_LOOP {
            ticket::ID_RE.is_match(name)
        } else {
            PLAN_NAME_RE.is_match(name)
        };
        if !name_ok {
            return Err(Error::InvalidChangeName {
                name: name.to_string(),
            });
        }
        self.check_state_files()?;
        let changes = self.read_changes()?;
        if changes.contains_key(name) {
            return Err(Error::ChangeExists {
                name: name.to_string(),
                path: self.state_path(),
            });
        }
        let mut c = Change::new(name);
        c.loop_ = loop_.to_string();
        c.sources = opts.sources;
        c.repos = opts.repos;
        let rel = self.rel_dir(&c)?;
        let m = c.machine();
        let first = m.phases[0].name;
        let mut lines = vec![format!("{name}: created")];
        for p in m.phases {
            if let Some((_, reason)) = opts.skip.iter().find(|(phase, _)| *phase == p.name) {
                m.set_gate(&mut c, p.name, SKIPPED, reason)?;
                lines.push(transition_line(name, p.name, SKIPPED, reason));
            }
        }
        m.set_gate(&mut c, first, DRAFT, "")?;
        if let Some(link) = write::through_symlink(self.root, &rel) {
            return Err(Error::WontCreate { rel, link });
        }
        let dir = at(self.root, &rel);
        write::mkdir_all(&dir)?;
        let mut paths = Vec::new();
        if !self.local() {
            write::write_file(self.root, &dir.join(".gitkeep"), b"")?;
            paths.push(format!("{rel}/.gitkeep"));
        }
        self.save(&mut c)?;
        paths.extend(opts.paths);
        if opts.refresh {
            paths.extend(self.refresh(&c));
        }
        self.record(&lines, &paths)?;
        Ok(c)
    }

    /// Go's `active`: the active change, which must be recorded or found, with a
    /// real folder.
    pub fn active(&self) -> Result<Change, Error> {
        let c = self.load()?;
        if c.name.is_empty() {
            return Err(Error::NoActiveChange);
        }
        let invalid = || Error::ActiveInvalid {
            name: c.name.clone(),
        };
        if c.loop_.is_empty() {
            if std::fs::symlink_metadata(at(self.root, &format!("{PLAN_DIR}/{}", c.name))).is_ok() {
                return Err(invalid());
            }
            return Err(Error::ActiveNoFolder {
                name: c.name.clone(),
            });
        }
        self.check_loop(&c)?;
        let rel = self.rel_dir(&c).map_err(|_| invalid())?;
        if std::fs::metadata(at(self.root, &rel)).is_err() {
            return Err(Error::ActiveNoFolderAt {
                name: c.name.clone(),
                rel,
            });
        }
        self.check_folder(&c).map_err(|_| invalid())?;
        Ok(c)
    }

    /// Go's `Advance`: sets the active change's gate, then records and commits it
    /// with any notes.
    pub fn advance(
        &self,
        phase: &str,
        status: &str,
        reason: &str,
        notes: &[String],
    ) -> Result<Change, Error> {
        let mut c = self.active()?;
        c.machine().set_gate(&mut c, phase, status, reason)?;
        self.save(&mut c)?;
        let mut lines = vec![transition_line(&c.name, phase, status, reason)];
        lines.extend(notes.iter().map(|n| format!("{}: {n}", c.name)));
        let written = self.refresh(&c);
        let mut paths = self.change_paths(&c);
        paths.extend(written);
        self.record(&lines, &paths)?;
        Ok(c)
    }

    /// Go's `Reopen` on the active change.
    pub fn reopen(&self, phase: &str, reason: &str, notes: &[String]) -> Result<Change, Error> {
        let mut c = self.active()?;
        c.machine().reopen(&mut c, phase, reason)?;
        self.save(&mut c)?;
        let written = self.refresh(&c);
        let mut lines = vec![format!("{}: reopened {phase} ({reason})", c.name)];
        lines.extend(notes.iter().map(|n| format!("{}: {n}", c.name)));
        let mut paths = self.change_paths(&c);
        paths.extend(written);
        self.record(&lines, &paths)?;
        Ok(c)
    }

    /// Go's `changePaths`: the change's folder and, for a plan, its research and
    /// explore folders when they exist.
    fn change_paths(&self, c: &Change) -> Vec<String> {
        let mut paths = Vec::new();
        if let Ok(rel) = self.rel_dir(c) {
            paths.push(rel);
        }
        if c.loop_ == PLAN_LOOP {
            for dir in PLAN_CHANGE_DIRS {
                let rel = format!("{dir}/{}", c.name);
                if std::fs::metadata(at(self.root, &rel)).is_ok() {
                    paths.push(rel);
                }
            }
        }
        paths
    }

    /// The store's refresh hook: the overview of the change's plan. A failure is
    /// a warning; it returns what it wrote.
    fn refresh(&self, c: &Change) -> Vec<String> {
        match super::overview::refresh_for(self, c) {
            Ok(written) => written,
            Err(e) => {
                self.warn(e.to_string());
                Vec::new()
            }
        }
    }

    /// Go's `CloseChange`.
    pub fn close(&self, name: &str, reason: &str) -> Result<Change, Error> {
        let changes = self.read_changes()?;
        let mut c = self.resolve(name, &changes);
        self.check_folder(&c)?;
        c.machine().close(&mut c, reason)?;
        self.check_state_files()?;
        self.write(&mut c)?;
        if self.read_active()? == name {
            let p = at(self.root, ".daiku/active");
            std::fs::remove_file(&p).map_err(|source| Error::Io {
                op: "remove",
                path: gofmt::to_slash(&p.to_string_lossy()),
                source,
            })?;
        }
        let written = self.refresh(&c);
        self.record(&[format!("{name}: closed ({reason})")], &written)?;
        Ok(c)
    }

    /// Go's `Log`: one `LOG.md` line.
    pub fn log(&self, line: &str) -> Result<(), Error> {
        super::changelog::append(
            self.root,
            &at(self.root, "LOG.md"),
            self.env.clock.now(),
            line,
        )
    }

    /// Go's `Record`: logs the lines, then in a git workspace commits
    /// `STATE.json`, `LOG.md` and `paths` with the first line as the message.
    pub fn record(&self, lines: &[String], paths: &[String]) -> Result<(), Error> {
        for line in lines {
            self.log(line)?;
        }
        if self.settings.is_none() || self.local() {
            return Ok(());
        }
        let mut all = vec!["STATE.json".to_string(), "LOG.md".to_string()];
        all.extend(paths.iter().cloned());
        self.commit(&format!("daiku: {}", lines[0]), &all)
    }

    /// Go's `commit`: stage the paths, keep those whose staged diff isn't empty,
    /// and commit only them. Outside a git repo it warns and commits nothing.
    fn commit(&self, message: &str, paths: &[String]) -> Result<(), Error> {
        if self.git(&["rev-parse", "--is-inside-work-tree"]).is_err() {
            self.warn(format!(
                "{} is not in a git repo — this state change was not committed; set \"vcs\": \"none\" if this workspace is meant to be local",
                gofmt::to_slash(&self.root.to_string_lossy())
            ));
            return Ok(());
        }
        let failed = |detail: String| Error::NotCommitted { detail };
        let mut args = vec!["add", "--"];
        args.extend(paths.iter().map(String::as_str));
        self.git(&args).map_err(failed)?;
        let changed: Vec<&str> = paths
            .iter()
            .map(String::as_str)
            .filter(|p| self.git(&["diff", "--cached", "--quiet", "--", p]).is_err())
            .collect();
        if changed.is_empty() {
            return Ok(());
        }
        let mut args = vec!["commit", "--quiet", "--only", "-m", message, "--"];
        args.extend(changed);
        self.git(&args).map_err(failed)?;
        Ok(())
    }

    /// Go's `git`: runs git in the root; the error is
    /// `git <first arg>: <err>: <trimmed output>`.
    fn git(&self, args: &[&str]) -> Result<String, String> {
        let cmd = Cmd::new("git").args(args.iter().copied()).dir(self.root);
        let (err, out) = match self.env.process.output(&cmd) {
            Ok(out) => {
                let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                text.push_str(&String::from_utf8_lossy(&out.stderr));
                match out.code {
                    Some(0) => return Ok(text),
                    Some(c) => (format!("exit status {c}"), text),
                    None => ("signal: killed".to_string(), text),
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (
                "exec: \"git\": executable file not found in $PATH".to_string(),
                String::new(),
            ),
            Err(e) => (e.to_string(), String::new()),
        };
        Err(format!("git {}: {err}: {}", args[0], out.trim()))
    }
}
