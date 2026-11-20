//! Tests for the `tickets` operations: what the fixtures can't see. Gate checks
//! under each policy, all-or-nothing readying (AC6), recording and commits
//! (AC7), and where `tickets new` puts a ticket.

use std::rc::Rc;

use super::*;
use crate::Workspace;
use crate::env::{Cmd, Output};
use crate::fake::ScriptedProcess;
use crate::testutil::{self, Temp};

struct Shared(Rc<ScriptedProcess>);

impl crate::env::Process for Shared {
    fn output(&self, cmd: &Cmd) -> std::io::Result<Output> {
        self.0.output(cmd)
    }
    fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32> {
        self.0.run_attached(cmd)
    }
}

fn exit(code: i32) -> Output {
    Output {
        code: Some(code),
        ..Output::default()
    }
}

const READY_BODY: &str = "\n## Acceptance criteria\n- [ ] AC1: Given x When y Then z\n";

fn ticket_text(id: &str, status: &str, body: &str) -> String {
    format!(
        "---\nschema: 1\nid: {id}\ntype: story\ntitle: Ticket {id}\nrepo: app\nchange: 1-x\nstatus: {status}\n---\n{body}"
    )
}

/// A plan workspace with change `1-x` active, its gates as given, and settings
/// with `overview: manual` plus `extra` (so no README is involved).
fn workspace(t: &Temp, gates: &str, extra: &str) -> (Workspace, Rc<ScriptedProcess>) {
    t.write(
        ".daiku/settings.json",
        &format!("{{\"overview\": \"manual\"{extra}}}\n"),
    );
    t.write(".daiku/active", "1-x\n");
    t.write(
        "STATE.json",
        &format!(
            r#"{{"schema":1,"changes":{{"1-x":{{"loop":"plan","phase":"breakdown","status":"approved","gates":{{{gates}}},"sources":[],"repos":["app"],"updatedAt":"x"}}}}}}"#
        ),
    );
    t.write(
        "REPOS.json",
        "{\"schema\": 1, \"repos\": {\"app\": {\"remote\": \"https://example.com/app.git\"}}}\n",
    );
    std::fs::create_dir_all(t.p("plan/specs/1-x/tickets")).unwrap();
    let p = Rc::new(ScriptedProcess::default());
    let env = testutil::env(t, Box::new(Shared(p.clone())));
    (Workspace::open(t.path(), env), p)
}

const APPROVED: &str = r#""breakdown":"approved""#;
const PUBLISHING: &str = r#""breakdown":"approved","publish":"in-progress""#;

fn args(p: &ScriptedProcess) -> Vec<Vec<String>> {
    p.calls.borrow().iter().map(|c| c.args.clone()).collect()
}

fn ids(v: &[&str]) -> ReadyTargets {
    ReadyTargets::Ids(v.iter().map(|s| s.to_string()).collect())
}

#[test]
fn the_breakdown_gate_under_each_policy() {
    // block: a refusal, as a problem, and nothing written.
    let t = Temp::new();
    let (ws, _) = workspace(&t, r#""breakdown":"draft""#, r#", "vcs": "none""#);
    let draft = ticket_text("S-0001", "draft", READY_BODY);
    t.write("plan/specs/1-x/tickets/S-0001.md", &draft);
    let out = ws.tickets_ready(&ids(&["S-0001"]), false).unwrap();
    assert_eq!(
        out.value,
        ReadyResult::Refused(vec![Problem::new(
            "1-x",
            "daiku: tickets ready requires breakdown approved for 1-x (it's \"draft\") (pass --force to override)".into()
        )])
    );
    assert_eq!(t.read("plan/specs/1-x/tickets/S-0001.md"), draft);

    // --force: a warning and an override line, then the ticket is readied.
    let out = ws.tickets_ready(&ids(&["S-0001"]), true).unwrap();
    assert_eq!(out.value, ReadyResult::Readied(vec!["S-0001".into()]));
    let violation = "daiku: tickets ready requires breakdown approved for 1-x (it's \"draft\")";
    assert_eq!(out.warnings[0].message, violation);
    let log = t.read("LOG.md");
    assert!(
        log.contains(&format!("1-x: override: {violation}\n")),
        "{log}"
    );
    assert!(log.ends_with("1-x: S-0001 readied\n"), "{log}");

    // A policy other than block lets it through without --force.
    let t = Temp::new();
    let (ws, _) = workspace(
        &t,
        r#""breakdown":"draft""#,
        r#", "vcs": "none", "gates": "auto""#,
    );
    t.write("plan/specs/1-x/tickets/S-0001.md", &draft);
    let out = ws.tickets_ready(&ids(&["S-0001"]), false).unwrap();
    assert_eq!(out.value, ReadyResult::Readied(vec!["S-0001".into()]));
    assert_eq!(out.warnings.len(), 1);
}

/// AC6: one bad target refuses the run, and no file changes.
#[test]
fn ready_is_all_or_nothing() {
    let t = Temp::new();
    let (ws, process) = workspace(&t, APPROVED, "");
    let good = ticket_text("S-0001", "draft", READY_BODY);
    let bad = ticket_text("S-0002", "draft", "\nNo criteria.\n");
    t.write("plan/specs/1-x/tickets/S-0001.md", &good);
    t.write("plan/specs/1-x/tickets/S-0002.md", &bad);
    let state = t.read("STATE.json");
    let out = ws.tickets_ready(&ReadyTargets::All, false).unwrap();
    let ReadyResult::Refused(problems) = out.value else {
        panic!("{:?}", out.value);
    };
    assert!(!problems.is_empty());
    assert!(problems.iter().all(|p| p.id == "S-0002"), "{problems:?}");
    assert_eq!(t.read("plan/specs/1-x/tickets/S-0001.md"), good);
    assert_eq!(t.read("plan/specs/1-x/tickets/S-0002.md"), bad);
    assert_eq!(t.read("STATE.json"), state);
    assert!(!t.exists("LOG.md"));
    assert!(process.calls.borrow().is_empty());
}

#[test]
fn ready_all_with_no_drafts() {
    let t = Temp::new();
    let (ws, _) = workspace(&t, APPROVED, r#", "vcs": "none""#);
    let out = ws.tickets_ready(&ReadyTargets::All, false).unwrap();
    assert_eq!(out.value, ReadyResult::NoDrafts("1-x".into()));
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Go's commit for a record of `paths`: rev-parse, add, a quiet diff per path,
/// then a commit of the ones that changed.
fn commit_calls(paths: &[&str], message: &str, changed: &[&str]) -> Vec<Vec<String>> {
    let mut want = vec![strings(&["rev-parse", "--is-inside-work-tree"])];
    let mut add = strings(&["add", "--"]);
    add.extend(strings(paths));
    want.push(add);
    for p in paths {
        want.push(strings(&["diff", "--cached", "--quiet", "--", p]));
    }
    let mut commit = strings(&["commit", "--quiet", "--only", "-m", message, "--"]);
    commit.extend(strings(changed));
    want.push(commit);
    want
}

/// Replies for one commit: rev-parse and add succeed, then each path's
/// `diff --cached --quiet` (0 = unchanged, 1 = changed), then the commit.
fn script(p: &ScriptedProcess, changed: &[bool]) {
    p.reply(exit(0));
    p.reply(exit(0));
    for &c in changed {
        p.reply(exit(i32::from(c)));
    }
    p.reply(exit(0));
}

const STAMP: &str = "- 2026-11-07T01:02:03.004Z ";
const DRAFT_PULL: &str = "---\nschema: 1\nid: DRAFT\ntype: task\ntitle: x\nrepo: app\nexternal:\n  jira: PROJ-9\nstatus: draft\n---\n";

/// AC7: each recording verb logs with the clock and commits exactly its files.
#[test]
fn recording_verbs_commit_what_they_record() {
    let ticket = "plan/specs/1-x/tickets/S-0001.md";

    // ready
    let t = Temp::new();
    let (ws, p) = workspace(&t, APPROVED, "");
    t.write(ticket, &ticket_text("S-0001", "draft", READY_BODY));
    script(&p, &[false, true, true]);
    ws.tickets_ready(&ids(&["S-0001"]), false).unwrap();
    assert_eq!(t.read("LOG.md"), format!("{STAMP}1-x: S-0001 readied\n"));
    assert_eq!(
        args(&p),
        commit_calls(
            &["STATE.json", "LOG.md", ticket],
            "daiku: 1-x: S-0001 readied",
            &["LOG.md", ticket]
        )
    );

    // mark-published, then unfreeze
    let t = Temp::new();
    let (ws, p) = workspace(&t, PUBLISHING, "");
    let mut readied = Ticket::parse(ticket_text("S-0001", "draft", READY_BODY).as_bytes()).unwrap();
    readied.mark_ready().unwrap();
    t.write(ticket, &String::from_utf8(readied.to_bytes()).unwrap());
    script(&p, &[false, true, true]);
    let out = ws
        .tickets_mark_published("S-0001", "jira", "PROJ-7", false)
        .unwrap();
    assert!(matches!(out.value, PublishResult::Published(_)));
    assert_eq!(
        args(&p),
        commit_calls(
            &["STATE.json", "LOG.md", ticket],
            "daiku: 1-x: S-0001 published to jira (PROJ-7)",
            &["LOG.md", ticket]
        )
    );
    p.calls.borrow_mut().clear();
    script(&p, &[false, true, true]);
    ws.tickets_unfreeze("S-0001", "fix a typo").unwrap();
    assert_eq!(
        args(&p),
        commit_calls(
            &["STATE.json", "LOG.md", ticket],
            "daiku: 1-x: S-0001 unfrozen (fix a typo)",
            &["LOG.md", ticket]
        )
    );
    assert_eq!(
        t.read("LOG.md"),
        format!(
            "{STAMP}1-x: S-0001 published to jira (PROJ-7)\n{STAMP}1-x: S-0001 unfrozen (fix a typo)\n"
        )
    );

    // pull
    let t = Temp::new();
    let (ws, p) = workspace(&t, APPROVED, "");
    t.write("draft.md", DRAFT_PULL);
    script(&p, &[false, true, true]);
    let out = ws.tickets_pull(&t.p("draft.md").to_string_lossy()).unwrap();
    assert_eq!(out.value.id, "X-PROJ-9");
    let rel = "build/tickets/X-PROJ-9.md";
    assert_eq!(
        args(&p),
        commit_calls(
            &["STATE.json", "LOG.md", rel],
            "daiku: X-PROJ-9: pulled from jira",
            &["LOG.md", rel]
        )
    );
}

/// AC7: under `vcs: none` the same verbs run no process at all.
#[test]
fn recording_verbs_run_no_git_under_vcs_none() {
    let t = Temp::new();
    let (ws, p) = workspace(&t, PUBLISHING, r#", "vcs": "none""#);
    t.write(
        "plan/specs/1-x/tickets/S-0001.md",
        &ticket_text("S-0001", "draft", READY_BODY),
    );
    ws.tickets_ready(&ids(&["S-0001"]), false).unwrap();
    ws.tickets_mark_published("S-0001", "jira", "PROJ-7", false)
        .unwrap();
    ws.tickets_unfreeze("S-0001", "why").unwrap();
    t.write("draft.md", DRAFT_PULL);
    ws.tickets_pull(&t.p("draft.md").to_string_lossy()).unwrap();
    assert!(p.calls.borrow().is_empty());
    assert_eq!(t.read("LOG.md").lines().count(), 4);
}

#[test]
fn new_goes_to_the_inbox_without_an_open_plan_change() {
    let t = Temp::new();
    let (ws, _) = workspace(&t, APPROVED, r#", "vcs": "none""#);
    std::fs::remove_file(t.p(".daiku/active")).unwrap();
    let o = TicketNewOptions {
        typ: TASK.into(),
        title: "Fix\n  typo".into(),
        repo: "app".into(),
        ..TicketNewOptions::default()
    };
    let out = ws.tickets_new(&o).unwrap().value;
    assert_eq!(out.id, "N-0001");
    assert!(out.inbox);
    assert!(
        out.path.ends_with("/build/tickets/N-0001.md"),
        "{}",
        out.path
    );
    let created = Ticket::parse(t.read("build/tickets/N-0001.md").as_bytes()).unwrap();
    assert_eq!(created.title, "Fix typo");
    let err = ws
        .tickets_new(&TicketNewOptions {
            typ: STORY.into(),
            ..o.clone()
        })
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "daiku: an inbox ticket is a task, not a story"
    );
}

#[test]
fn new_in_a_plan_uses_the_shared_sequence_and_the_change_repo() {
    let t = Temp::new();
    let (ws, _) = workspace(&t, APPROVED, r#", "vcs": "none""#);
    t.write(
        "plan/specs/1-x/tickets/S-0004.md",
        &ticket_text("S-0004", "draft", READY_BODY),
    );
    let out = ws
        .tickets_new(&TicketNewOptions {
            typ: TASK.into(),
            title: "Write docs".into(),
            ..TicketNewOptions::default()
        })
        .unwrap()
        .value;
    assert_eq!(out.id, "T-0005");
    let t5 = Ticket::parse(t.read("plan/specs/1-x/tickets/T-0005.md").as_bytes()).unwrap();
    assert_eq!(t5.repo, "app");
    assert_eq!(t5.change, "1-x");
    assert_eq!(t5.spec, "plan/specs/1-x/functional-spec.md");
    assert!(t5.body.contains("## Acceptance criteria"));
    assert!(!t.exists("LOG.md"), "new writes without recording");

    let err = ws
        .tickets_new(&TicketNewOptions {
            typ: STORY.into(),
            title: "x".into(),
            requirements: vec!["R1".into(), "bad".into()],
            ..TicketNewOptions::default()
        })
        .unwrap_err();
    assert_eq!(
        err.to_string(),
        "daiku tickets new: R1 can't be checked: 1-x has no spec; bad isn't a requirement id (R<n> or N<n>)"
    );
}

#[test]
fn mark_published_collects_every_problem() {
    let t = Temp::new();
    let (ws, _) = workspace(&t, APPROVED, r#", "vcs": "none""#);
    t.write(
        "plan/specs/1-x/tickets/S-0001.md",
        &ticket_text("S-0001", "draft", READY_BODY)
            .replace("status: draft", "depends_on: [S-0009]\nstatus: draft"),
    );
    let out = ws
        .tickets_mark_published("S-0001", "jira", "P-1", false)
        .unwrap();
    let PublishResult::Refused(problems) = out.value else {
        panic!("{:?}", out.value);
    };
    let text: Vec<String> = problems.iter().map(|p| p.to_string()).collect();
    assert_eq!(
        text,
        [
            "1-x: daiku: mark-published requires publish in-progress for 1-x (it's \"unset\") (pass --force to override)",
            "S-0001: is a draft — run `daiku tickets ready S-0001` first",
            "S-0001: depends_on S-0009 not found",
        ]
    );
}
