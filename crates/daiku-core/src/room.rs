//! The plan room's stores as workspace operations (D3–D5): `source`, `adr`,
//! `idea`, `references check` and `overview`. Each follows its Go handler in
//! `internal/cli` after argument parsing: the calls it makes, in its order, so
//! the same failures surface. The client parses arguments and prints.
//!
//! As in Go, none of these needs a workspace: they run against the root as it
//! is, and fail only where a call they make fails.

use serde::Serialize;

use crate::changes::{ChangeNewOptions, NewChange};
use crate::plan::{adr, idea, reference, source};
use crate::state::overview::{self as state_overview, overview_error, overview_path};
use crate::state::store::at;
use crate::state::{BUILD_LOOP, Change, Store};
use crate::text::Text;
use crate::{Error, Outcome, Workspace, date, settings, write};

/// What `source new` takes. The kind is `repo` when `repo` is set.
#[derive(Debug, Clone, Default)]
pub struct SourceNewOptions {
    pub title: String,
    pub repo: String,
    pub rev: String,
    pub body: String,
}

/// The status changes `idea park|drop|reopen` make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdeaVerb {
    Park,
    Drop,
    Reopen,
}

/// What `idea promote` takes.
#[derive(Debug, Clone, Default)]
pub struct IdeaPromoteOptions {
    pub id: i64,
    pub title: String,
    pub repos: Vec<String>,
    pub rev: String,
    /// The version the CLI reports, for the plan room's manifest.
    pub version: String,
}

/// `idea promote`'s result, in Go's order: whether the plan room was created,
/// then the change, then the idea. `marked` is `None` until the change exists; a
/// failure there keeps the change (`Error::IdeaNotMarked`).
#[derive(Debug)]
pub struct IdeaPromoteResult {
    pub room_created: bool,
    pub change: Result<Outcome<NewChange>, Error>,
    pub marked: Option<Result<idea::Idea, Error>>,
}

/// A plan's README, rendered: its path, the merged text, whether the file on
/// disk already holds it, and whether this call wrote it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OverviewDoc {
    /// Relative to the root, slash-separated.
    pub rel: String,
    /// The README's bytes: the user's text outside the region kept as it is.
    pub merged: Text,
    pub up_to_date: bool,
    pub written: bool,
}

/// Go's `requirePlan`.
fn require_plan(c: &Change, verb: &'static str) -> Result<(), Error> {
    if c.loop_ == BUILD_LOOP {
        return Err(Error::BuildChange {
            name: c.name.clone(),
            verb,
        });
    }
    Ok(())
}

impl Workspace {
    fn today(&self) -> String {
        date::today(self.env())
    }

    fn effective(&self) -> Result<settings::Settings, Error> {
        Ok(settings::load(self.root(), &self.env().dirs)?.effective)
    }

    /// `daiku source list`: Go's `ListSources`, every `<n>.md` by id.
    pub fn source_list(&self) -> Result<Outcome<Vec<source::Source>>, Error> {
        let dir = self.effective()?.source_dir;
        Ok(Outcome::new(source::list(self.root(), &dir)))
    }

    /// `daiku source show <id>`.
    pub fn source_show(&self, id: i64) -> Result<Outcome<source::Source>, Error> {
        let dir = self.effective()?.source_dir;
        source::read_source(self.root(), &dir, id).map(Outcome::new)
    }

    /// `daiku source new`: the next id past both the files and every id a change
    /// records (Go's `intakeFloor`), so a deleted intake's id isn't reused.
    pub fn source_new(&self, opts: &SourceNewOptions) -> Result<Outcome<source::Source>, Error> {
        let dir = self.effective()?.source_dir;
        let recorded: Vec<String> = Store::reader(self.root(), self.env())
            .read_changes()?
            .into_values()
            .flat_map(|c| c.sources)
            .collect();
        let kind = if opts.repo.is_empty() {
            source::ISSUE
        } else {
            source::REPO
        };
        let new = source::NewSource {
            title: opts.title.clone(),
            source: kind.into(),
            repo: opts.repo.clone(),
            rev: opts.rev.clone(),
            body: opts.body.as_str().into(),
            idea: 0,
            after: source::max_recorded_id(&dir, &recorded),
        };
        source::write_new(self.root(), &dir, &new).map(Outcome::new)
    }

    /// `daiku adr list`.
    pub fn adr_list(&self) -> Result<Outcome<Vec<adr::Record>>, Error> {
        let dir = self.effective()?.adr_dir;
        Ok(Outcome::new(adr::list(self.root(), &dir)))
    }

    /// `daiku adr show <n>`.
    pub fn adr_show(&self, number: i64) -> Result<Outcome<adr::Record>, Error> {
        let dir = self.effective()?.adr_dir;
        adr::show(self.root(), &dir, number).map(Outcome::new)
    }

    /// `daiku adr new`, dated today by the `Env`'s clock.
    pub fn adr_new(&self, opts: &adr::Options) -> Result<Outcome<adr::Record>, Error> {
        let dir = self.effective()?.adr_dir;
        adr::create(self.root(), &dir, opts, &self.today()).map(Outcome::new)
    }

    /// `daiku idea new <title>`, dated today.
    pub fn idea_new(&self, title: &str) -> Result<Outcome<idea::Idea>, Error> {
        idea::new(self.root(), title, &self.today()).map(Outcome::new)
    }

    /// `daiku idea list`.
    pub fn idea_list(&self) -> Result<Outcome<Vec<idea::Idea>>, Error> {
        idea::list(self.root()).map(Outcome::new)
    }

    /// `daiku idea show <id>`: the idea's `summary.md`, byte for byte.
    pub fn idea_show(&self, id: i64) -> Result<Outcome<Vec<u8>>, Error> {
        idea::summary(self.root(), id).map(Outcome::new)
    }

    /// `daiku idea park|drop|reopen <id> --reason <r>`, dated today.
    pub fn idea_set_status(
        &self,
        verb: IdeaVerb,
        id: i64,
        reason: &str,
    ) -> Result<Outcome<idea::Idea>, Error> {
        let op = match verb {
            IdeaVerb::Park => idea::park,
            IdeaVerb::Drop => idea::drop,
            IdeaVerb::Reopen => idea::reopen,
        };
        op(self.root(), id, reason, &self.today()).map(Outcome::new)
    }

    /// `daiku idea promote`: Go's `ideaPromote`. It checks the idea first, creates
    /// the change and its intake through `change_new` (which commits in a git
    /// workspace), then records the change on the idea.
    pub fn idea_promote(&self, opts: &IdeaPromoteOptions) -> IdeaPromoteResult {
        let refused = |e| IdeaPromoteResult {
            room_created: false,
            change: Err(e),
            marked: None,
        };
        let i = match idea::load(self.root(), opts.id).and_then(|i| i.can_promote().map(|()| i)) {
            Ok(i) => i,
            Err(e) => return refused(e),
        };
        let today = self.today();
        let res = self.change_new(&ChangeNewOptions {
            title: opts.title.clone(),
            repos: opts.repos.clone(),
            rev: opts.rev.clone(),
            body: [
                format!("Promoted from idea `{}/` on {today}.\n\n", i.dir).as_bytes(),
                i.body.as_bytes(),
            ]
            .concat()
            .into(),
            idea: opts.id,
            version: opts.version.clone(),
        });
        let marked = res.result.as_ref().ok().map(|created| {
            let name = &created.value.change.name;
            idea::mark_promoted(self.root(), opts.id, name, &today).map_err(|e| {
                Error::IdeaNotMarked {
                    change: name.clone(),
                    id: opts.id,
                    cause: e.to_string(),
                }
            })
        });
        IdeaPromoteResult {
            room_created: res.room_created,
            change: res.result,
            marked,
        }
    }

    /// `daiku references check`.
    pub fn references_check(&self) -> Result<Outcome<reference::Check>, Error> {
        reference::check(self.root()).map(Outcome::new)
    }

    /// `daiku overview [<change>]`: Go's `buildOverview`, then, with `write`, the
    /// README written when it differs. With no change named it's the active one.
    /// Rendering errors name the README (Go's `OverviewError`); a write error
    /// doesn't.
    pub fn overview(
        &self,
        change: Option<&str>,
        write: bool,
    ) -> Result<Outcome<OverviewDoc>, Error> {
        let store = Store::reader(self.root(), self.env());
        let name = match change.filter(|c| !c.is_empty()) {
            Some(c) => c.to_string(),
            None => {
                let st = store.load_state()?;
                if st.name.is_empty() {
                    return Err(Error::NoActiveOverview);
                }
                require_plan(&st, "overview")?;
                st.name
            }
        };
        if let Some(rec) = store.read_changes()?.get(&name) {
            require_plan(rec, "overview")?;
        }
        if !store.plan_changes()?.iter().any(|k| k.name == name) {
            return Err(Error::NoChangeNamed { name });
        }
        let rel = overview_path(&name);
        let (merged, existing) = settings::load(self.root(), &self.env().dirs)
            .and_then(|loaded| state_overview::overview(&store, &name, &loaded))
            .map_err(|e| overview_error(&rel, &e.to_string()))?;
        let up_to_date = existing.as_deref() == Some(merged.as_slice());
        let written = write && !up_to_date;
        if written {
            write::write_file(self.root(), &at(self.root(), &rel), &merged)?;
        }
        Ok(Outcome::new(OverviewDoc {
            rel,
            merged: merged.into(),
            up_to_date,
            written,
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::time::{Duration, UNIX_EPOCH};

    use super::*;
    use crate::env::{Cmd, Output, Process};
    use crate::fake::{FixedClock, ScriptedProcess};
    use crate::testutil::{Temp, env};

    /// The test's handle on the runner the workspace uses.
    struct Shared(Rc<ScriptedProcess>);

    impl Process for Shared {
        fn output(&self, cmd: &Cmd) -> std::io::Result<Output> {
            self.0.output(cmd)
        }
        fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32> {
            self.0.run_attached(cmd)
        }
    }

    /// A `vcs: none` workspace in `t` whose clock is `clock`, and its process
    /// runner, which fails every call.
    fn workspace_with(
        t: &Temp,
        clock: impl crate::Clock + 'static,
    ) -> (Workspace, Rc<ScriptedProcess>) {
        t.write(
            ".daiku/settings.json",
            "{\n  \"schema\": 1,\n  \"vcs\": \"none\"\n}\n",
        );
        let process = Rc::new(ScriptedProcess::default());
        let mut e = env(t, Box::new(Shared(process.clone())));
        e.clock = Box::new(clock);
        (Workspace::open(t.path(), e), process)
    }

    fn workspace(t: &Temp, clock: impl crate::Clock + 'static) -> Workspace {
        workspace_with(t, clock).0
    }

    /// 2001-02-03T23:30:00Z.
    fn late_evening() -> std::time::SystemTime {
        UNIX_EPOCH + Duration::from_secs(981_243_000)
    }

    /// AC4: with a clock an hour east of UTC at 23:30Z, `adr new` and `idea new`
    /// write the next day's local date, so the date comes from the clock and its
    /// zone, not from the machine.
    #[test]
    fn ac4_dates_come_from_the_clock() {
        let t = Temp::new();
        let ws = workspace(&t, FixedClock::zoned(late_evening(), 3600));
        let rec = ws
            .adr_new(&adr::Options {
                title: "Pick a store".into(),
                ..adr::Options::default()
            })
            .unwrap()
            .value;
        assert_eq!(rec.date, "2001-02-04");
        assert!(
            t.read("plan/04_architecture/adr/0001-pick-a-store.md")
                .contains("\n- Date: 2001-02-04\n")
        );

        let i = ws.idea_new("Faster imports").unwrap().value;
        assert_eq!(i.updated, "2001-02-04");
        let summary = t.read("ideas/1-faster-imports/summary.md");
        assert!(summary.contains("\nupdated: 2001-02-04\n"), "{summary}");
        assert!(summary.contains("2001-02-04"), "{summary}");
        assert_eq!(
            t.read("ideas/1-faster-imports/log.md"),
            "# Log\n\n- 2001-02-04 created\n"
        );

        let parked = ws
            .idea_set_status(IdeaVerb::Park, 1, "later")
            .unwrap()
            .value;
        assert_eq!(parked.updated, "2001-02-04");
    }

    /// AC5: promoting an open idea in a `vcs: none` workspace creates the change
    /// and its intake, marks the idea, logs it, and runs no process.
    #[test]
    fn ac5_promote_without_git() {
        let t = Temp::new();
        let (ws, process) = workspace_with(&t, FixedClock::zoned(late_evening(), 0));
        ws.idea_new("Sync rewrite").unwrap();
        let res = ws.idea_promote(&IdeaPromoteOptions {
            id: 1,
            title: "Sync rewrite".into(),
            version: "test".into(),
            ..IdeaPromoteOptions::default()
        });
        assert!(res.room_created);
        let created = res.change.unwrap().value;
        assert_eq!(created.change.name, "1-sync-rewrite");
        let intake = t.read(&format!("plan/00_intake/{}.md", created.source_id));
        assert!(intake.contains("\nidea: 1\n"), "{intake}");
        assert!(
            intake.contains("Promoted from idea `ideas/1-sync-rewrite/` on 2001-02-03."),
            "{intake}"
        );
        let marked = res.marked.unwrap().unwrap();
        assert_eq!(marked.status, idea::PROMOTED);
        assert_eq!(marked.changes, ["1-sync-rewrite"]);
        assert!(
            t.read("ideas/1-sync-rewrite/log.md")
                .ends_with("- 2001-02-03 promoted to 1-sync-rewrite\n")
        );
        assert!(
            process.calls.borrow().is_empty(),
            "a vcs: none promote ran a process"
        );
    }

    /// A parked idea is refused before anything is created.
    #[test]
    fn promote_refuses_a_parked_idea_first() {
        let t = Temp::new();
        let ws = workspace(&t, FixedClock(late_evening()));
        ws.idea_new("Later").unwrap();
        ws.idea_set_status(IdeaVerb::Park, 1, "not now").unwrap();
        let res = ws.idea_promote(&IdeaPromoteOptions {
            id: 1,
            title: "Later".into(),
            ..IdeaPromoteOptions::default()
        });
        assert!(!res.room_created && res.marked.is_none());
        assert_eq!(
            res.change.unwrap_err().to_string(),
            "daiku: idea 1 is parked: reopen it first"
        );
        assert!(!t.exists("plan"));
    }

    fn plan_workspace() -> (Temp, Workspace) {
        let t = Temp::new();
        let ws = workspace(&t, FixedClock(late_evening()));
        let res = ws.change_new(&ChangeNewOptions {
            title: "Auth".into(),
            version: "test".into(),
            ..ChangeNewOptions::default()
        });
        res.result.unwrap();
        (t, ws)
    }

    /// The overview of a fresh plan is written once, then up to date; a hand edit
    /// to the generated region makes it stale until it's written again.
    #[test]
    fn overview_writes_when_it_differs() {
        let (t, ws) = plan_workspace();
        let rel = "plan/specs/1-auth/README.md";
        std::fs::remove_file(t.p(rel)).ok();
        let first = ws.overview(Some("1-auth"), true).unwrap().value;
        assert_eq!((first.rel.as_str(), first.written), (rel, true));
        assert_eq!(std::fs::read(t.p(rel)).unwrap(), first.merged.as_bytes());
        let again = ws.overview(None, true).unwrap().value;
        assert!(again.up_to_date && !again.written);

        let stale = t.read(rel).replacen("## ", "## Edited ", 1);
        std::fs::write(t.p(rel), &stale).unwrap();
        let checked = ws.overview(None, false).unwrap().value;
        assert!(!checked.up_to_date && !checked.written);
        assert_eq!(t.read(rel), stale, "a check wrote the README");
    }

    #[test]
    fn overview_refusals() {
        let (t, ws) = plan_workspace();
        let e = ws.overview(Some("9-nope"), false).unwrap_err().to_string();
        assert_eq!(e, r#"daiku: no change named "9-nope""#);
        std::fs::remove_file(t.p(".daiku/active")).unwrap();
        let e = ws.overview(None, false).unwrap_err().to_string();
        assert!(e.starts_with("daiku: no active change — name one"), "{e}");
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    /// N-0001 AC1: a README with bytes that aren't valid UTF-8 outside the region
    /// is rewritten with those bytes kept, then reads as up to date; a check
    /// compares bytes.
    #[test]
    fn overview_keeps_bytes_outside_the_region() {
        let (t, ws) = plan_workspace();
        let rel = "plan/specs/1-auth/README.md";
        ws.overview(None, true).unwrap();
        let fresh = std::fs::read(t.p(rel)).unwrap();
        let mut edited = fresh.clone();
        let at = edited
            .windows(23)
            .position(|w| w == b"<!-- /daiku:narrative -")
            .unwrap();
        edited.splice(at..at, b"Caf\xe9 by hand.\n".iter().copied());
        edited.extend_from_slice(b"\nAfter \xff.\n");
        let gen_end = edited
            .windows(23)
            .position(|w| w == b"<!-- /daiku:generated -")
            .unwrap();
        edited.splice(gen_end..gen_end, b"stale\n".iter().copied());
        std::fs::write(t.p(rel), &edited).unwrap();

        let doc = ws.overview(None, true).unwrap().value;
        assert!(doc.written && !doc.up_to_date);
        let got = std::fs::read(t.p(rel)).unwrap();
        assert_eq!(got, doc.merged.as_bytes());
        assert!(
            find(&got, b"Caf\xe9 by hand.\n").is_some(),
            "narrative kept"
        );
        assert!(got.ends_with(b"\nAfter \xff.\n"), "trailing text kept");
        assert!(find(&got, b"stale\n").is_none(), "region refreshed");
        let again = ws.overview(None, false).unwrap().value;
        assert!(again.up_to_date && !again.written);
    }

    /// N-0001 AC3: promote copies the idea's body bytes into the intake.
    #[test]
    fn promote_copies_the_bodys_bytes() {
        let t = Temp::new();
        let ws = workspace(&t, FixedClock(late_evening()));
        std::fs::create_dir_all(t.p("ideas/1-x")).unwrap();
        std::fs::write(
            t.p("ideas/1-x/summary.md"),
            b"---\ntitle: X\nstatus: open\n---\n\nBody \xff by hand.\n",
        )
        .unwrap();
        t.write("ideas/1-x/log.md", "# Log\n");
        let res = ws.idea_promote(&IdeaPromoteOptions {
            id: 1,
            title: "Promoted".into(),
            version: "test".into(),
            ..IdeaPromoteOptions::default()
        });
        let created = res.change.unwrap().value;
        let intake =
            std::fs::read(t.p(&format!("plan/00_intake/{}.md", created.source_id))).unwrap();
        assert!(
            intake.ends_with(
                b"Promoted from idea `ideas/1-x/` on 2001-02-03.\n\nBody \xff by hand.\n"
            ),
            "{intake:?}"
        );
        assert!(res.marked.unwrap().is_ok());
    }

    /// N-0001 AC6: the types holding bytes that aren't valid UTF-8 serialise to
    /// valid JSON, with U+FFFD for each invalid byte, as Go's `encoding/json`
    /// writes such strings.
    #[test]
    fn ac6_bytes_serialise_to_valid_json() {
        let bad = |b: &[u8]| crate::text::Text::from(b);
        let idea = idea::Idea {
            title: bad(b"Caf\xe9"),
            reason: bad(b"apr\xe8s"),
            body: bad(b"Body \xff"),
            ..idea::Idea::default()
        };
        let record = adr::Record {
            title: bad(b"D\xe9"),
            deciders: bad(b"Ren\xe9e"),
            raw: b"# 0001 \xff".to_vec(),
            ..adr::Record::default()
        };
        let doc = OverviewDoc {
            rel: "plan/specs/1-x/README.md".into(),
            merged: bad(b"# T \xe9\n"),
            up_to_date: false,
            written: true,
        };
        for (name, json, want) in [
            (
                "idea",
                serde_json::to_string(&idea).unwrap(),
                ["Caf\u{fffd}", "apr\u{fffd}s", "Body \u{fffd}"],
            ),
            (
                "adr",
                serde_json::to_string(&record).unwrap(),
                ["D\u{fffd}", "Ren\u{fffd}e", "# 0001 \u{fffd}"],
            ),
            (
                "overview",
                serde_json::to_string(&doc).unwrap(),
                ["# T \u{fffd}\n", "plan/specs/1-x/README.md", "true"],
            ),
        ] {
            let v: serde_json::Value =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}"));
            let text = v.to_string();
            for w in want {
                let w = serde_json::to_string(w).unwrap();
                assert!(
                    text.contains(w.trim_matches('"')),
                    "{name}: {text} lacks {w}"
                );
            }
        }
    }
}
