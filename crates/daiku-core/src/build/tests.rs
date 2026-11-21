//! Tests for the repo and build-room operations (S-0008): the clone events (AC2),
//! the registry commands, and `env`.

use std::rc::Rc;

use super::*;
use crate::Workspace;
use crate::env::{Cmd, Events, Output};
use crate::fake::{RecordingEvents, ScriptedProcess};
use crate::testutil::{self, Temp};

struct SharedProcess(Rc<ScriptedProcess>);

impl crate::env::Process for SharedProcess {
    fn output(&self, cmd: &Cmd) -> std::io::Result<Output> {
        self.0.output(cmd)
    }
    fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32> {
        self.0.run_attached(cmd)
    }
}

struct SharedEvents(Rc<RecordingEvents>);

impl Events for SharedEvents {
    fn emit(&self, event: Event) {
        self.0.emit(event);
    }
}

/// A workspace in `t` with a scripted process runner and a recording event sink,
/// both still reachable by the test.
fn workspace(t: &Temp) -> (Workspace, Rc<ScriptedProcess>, Rc<RecordingEvents>) {
    let process = Rc::new(ScriptedProcess::default());
    let events = Rc::new(RecordingEvents::default());
    let mut env = testutil::env(t, Box::new(SharedProcess(process.clone())));
    env.events = Box::new(SharedEvents(events.clone()));
    (Workspace::open(t.path(), env), process, events)
}

fn ok() -> Output {
    Output {
        code: Some(0),
        ..Output::default()
    }
}

#[test]
fn repo_add_list_and_path() {
    let t = Temp::new();
    let (ws, _, _) = workspace(&t);
    assert_eq!(ws.repo_list().unwrap().value, []);
    ws.repo_add("web", "https://example.invalid/web.git", None, "")
        .unwrap();
    ws.repo_add(
        "api",
        "git@example.invalid:org/api.git",
        Some("~/code/api"),
        "trunk",
    )
    .unwrap();
    assert_eq!(
        ws.repo_list().unwrap().value,
        [
            RepoRow {
                name: "api".into(),
                remote: "git@example.invalid:org/api.git".into(),
                branch: "trunk".into(),
                path: Some("~/code/api".into()),
            },
            RepoRow {
                name: "web".into(),
                remote: "https://example.invalid/web.git".into(),
                branch: "main".into(),
                path: None,
            },
        ]
    );
    // An empty --path is no path, as in Go.
    ws.repo_add("lib", "https://example.invalid/lib", Some(""), "")
        .unwrap();
    assert_eq!(ws.repo_list().unwrap().value[1].path, None);
    let cwd = t.path().to_string_lossy().into_owned();
    assert_eq!(
        ws.repo_path("web", "src/web")
            .unwrap()
            .value
            .replace('\\', "/"),
        format!("{cwd}/src/web").replace('\\', "/")
    );
    assert_eq!(
        ws.repo_path("nope", "/x").unwrap_err().to_string(),
        "daiku: unknown repo \"nope\" — run `daiku repo add`"
    );
    // A refused add leaves both files as they were.
    let before = (t.read("REPOS.json"), t.read(".daiku/paths.json"));
    assert!(
        ws.repo_add("web", "https://example.invalid/x", None, "")
            .is_err()
    );
    assert_eq!((t.read("REPOS.json"), t.read(".daiku/paths.json")), before);
}

/// Unix only: the expectations spell paths with `/`, which Windows writes with `\`;
/// the fixture cases cover Windows.
#[cfg(unix)]
/// AC2: two missing repos, an events sink, a scripted runner: a start and an end
/// for each clone, in name order, around that repo's `git clone`.
#[test]
fn doctor_clone_reports_a_start_and_an_end_for_each_clone() {
    let t = Temp::new();
    let (ws, process, events) = workspace(&t);
    ws.repo_add("api", "https://example.invalid/api.git", None, "")
        .unwrap();
    ws.repo_add(
        "web",
        "https://example.invalid/web.git",
        Some("elsewhere/web"),
        "",
    )
    .unwrap();
    process.reply(ok());
    process.reply(ok());
    let into = t.p("clones").to_string_lossy().into_owned();
    let report = ws.repo_doctor(true, Some(&into)).unwrap().value;

    let api = format!("{into}/api");
    let web = t.p("elsewhere/web").to_string_lossy().into_owned();
    assert_eq!(
        events.0.borrow().as_slice(),
        [
            Event::CloneStarted {
                repo: "api".into(),
                dest: api.clone()
            },
            Event::CloneFinished {
                repo: "api".into(),
                dest: api.clone(),
                error: None
            },
            Event::CloneStarted {
                repo: "web".into(),
                dest: web.clone()
            },
            Event::CloneFinished {
                repo: "web".into(),
                dest: web.clone(),
                error: None
            },
        ]
    );
    let args: Vec<Vec<String>> = process
        .calls
        .borrow()
        .iter()
        .map(|c| c.args.clone())
        .collect();
    assert_eq!(
        args,
        [
            [
                "clone",
                "--quiet",
                "--",
                "https://example.invalid/api.git",
                api.as_str()
            ],
            [
                "clone",
                "--quiet",
                "--",
                "https://example.invalid/web.git",
                web.as_str()
            ],
        ]
    );
    assert_eq!(
        report.clones,
        [
            CloneStep::Cloned {
                repo: "api".into(),
                path: api.clone()
            },
            CloneStep::Cloned {
                repo: "web".into(),
                path: web.clone()
            },
        ]
    );
    // Both paths were recorded.
    let rows = ws.repo_list().unwrap().value;
    assert_eq!(rows[0].path.as_deref(), Some(api.as_str()));
    assert_eq!(rows[1].path.as_deref(), Some(web.as_str()));
    assert!(!report.aborted);
}

#[test]
fn a_failed_clone_ends_with_its_error_and_the_run_goes_on() {
    let t = Temp::new();
    let (ws, process, events) = workspace(&t);
    ws.repo_add("api", "https://example.invalid/api.git", None, "")
        .unwrap();
    ws.repo_add("web", "https://example.invalid/web.git", None, "")
        .unwrap();
    process.reply(Output {
        code: Some(128),
        stderr: b"fatal: nope\n".to_vec(),
        ..Output::default()
    });
    process.reply(ok());
    let into = t.p("c").to_string_lossy().into_owned();
    let report = ws.repo_doctor(true, Some(&into)).unwrap().value;
    assert_eq!(
        report.clones[0],
        CloneStep::Failed {
            repo: "api".into(),
            error: "daiku: cloning api: exit status 128: fatal: nope".into()
        }
    );
    assert!(matches!(report.clones[1], CloneStep::Cloned { .. }));
    assert_eq!(
        events.0.borrow()[1],
        Event::CloneFinished {
            repo: "api".into(),
            dest: format!("{into}/api"),
            error: Some("daiku: cloning api: exit status 128: fatal: nope".into()),
        }
    );
    // The failed repo recorded no path, so the doctor still reports it.
    assert_eq!(ws.repo_list().unwrap().value[0].path, None);
    assert!(
        report
            .problems
            .iter()
            .any(|p| p.repo == "api" && p.detail == "no local path")
    );
}

#[test]
fn doctor_clone_skips_present_repos_and_says_when_it_cant_place_one() {
    let t = Temp::new();
    let (ws, process, events) = workspace(&t);
    std::fs::create_dir_all(t.p("here")).unwrap();
    ws.repo_add("here", "https://example.invalid/here", Some("here"), "")
        .unwrap();
    ws.repo_add("unmapped", "https://example.invalid/u", None, "")
        .unwrap();
    // `here` exists, so the doctor asks git about its origin; `unmapped` has no
    // path and no --into.
    process.reply(Output {
        code: Some(0),
        stdout: b"https://example.invalid/here\n".to_vec(),
        ..Output::default()
    });
    let report = ws.repo_doctor(true, None).unwrap().value;
    assert_eq!(
        report.clones,
        [CloneStep::NoPath {
            repo: "unmapped".into()
        }]
    );
    assert!(events.0.borrow().is_empty(), "nothing was attempted");
    let args: Vec<Vec<String>> = process
        .calls
        .borrow()
        .iter()
        .map(|c| c.args.clone())
        .collect();
    assert_eq!(args, [["remote", "get-url", "origin"]]);
    assert_eq!(report.problems.len(), 1);
    assert_eq!(report.count, 2);
    // Without --clone nothing is cloned and no event is sent.
    let report = ws.repo_doctor(false, Some("/x")).unwrap().value;
    assert!(report.clones.is_empty());
}

/// Unix only: the expectations spell paths with `/`, which Windows writes with `\`;
/// the fixture cases cover Windows.
#[cfg(unix)]
#[test]
fn env_lists_the_active_changes_repos() {
    let t = Temp::new();
    let (ws, _, _) = workspace(&t);
    assert_eq!(
        ws.env_flags().unwrap_err().to_string(),
        "daiku: no active change — run `daiku switch <change>` first"
    );
    std::fs::create_dir_all(t.p("code/api")).unwrap();
    std::fs::create_dir_all(t.p("code/my web")).unwrap();
    let (api, web) = (t.p("code/api"), t.p("code/my web"));
    ws.repo_add(
        "api",
        "https://example.invalid/api",
        Some(&api.to_string_lossy()),
        "",
    )
    .unwrap();
    ws.repo_add(
        "web",
        "https://example.invalid/web",
        Some(&web.to_string_lossy()),
        "",
    )
    .unwrap();
    t.write(".daiku/active", "1-x\n");
    t.write(
        "STATE.json",
        r#"{"schema":1,"changes":{"1-x":{"loop":"plan","phase":"intake","status":"draft","gates":{},"sources":[],"repos":["api","web"],"updatedAt":"x"}}}"#,
    );
    std::fs::create_dir_all(t.p("plan/specs/1-x")).unwrap();
    assert_eq!(
        ws.env_flags().unwrap().value,
        format!(
            "--add-dir {} --add-dir {}",
            repos::shell_quote(&api.to_string_lossy()),
            repos::shell_quote(&web.to_string_lossy())
        )
    );
}

// ---- build new (T5) ----

use crate::scaffold::InitOptions;
use crate::ticket::{NewTicket, STATUS_READY, STORY, TASK};

/// A `vcs: none` workspace from `init`, with a registered repo `api`, and a plan
/// change `1-x`.
fn initialised(t: &Temp) -> Workspace {
    let (ws, _, _) = workspace(t);
    ws.init(&InitOptions {
        no_git: true,
        force: false,
        vcs: "none".into(),
        version: "dev".into(),
    })
    .unwrap();
    ws.repo_add("api", "https://example.invalid/api", None, "")
        .unwrap();
    t.write(
        "STATE.json",
        r#"{"schema":1,"changes":{"1-x":{"loop":"plan","phase":"breakdown","status":"approved","gates":{},"sources":[],"repos":["api"],"updatedAt":"x"}}}"#,
    );
    std::fs::create_dir_all(t.p("plan/specs/1-x/tickets")).unwrap();
    ws
}

fn put(t: &Temp, rel: &str, ticket: &Ticket) {
    t.write(rel, &String::from_utf8(ticket.to_bytes()).unwrap());
}

fn planned(id: &str, status: &str, repo: &str) -> Ticket {
    let mut t = Ticket::new(NewTicket {
        id: id.into(),
        typ: STORY.into(),
        title: "Sign in & out".into(),
        repo: repo.into(),
        change: "1-x".into(),
        spec: "plan/specs/1-x/functional-spec.md".into(),
        body: "\n## Acceptance criteria\n- [ ] AC1: Given x When y Then z\n".into(),
        ..NewTicket::default()
    })
    .unwrap();
    if status == STATUS_READY {
        t.mark_ready().unwrap();
    }
    t
}

fn native(id: &str, repo: &str) -> Ticket {
    Ticket::new(NewTicket {
        id: id.into(),
        typ: TASK.into(),
        title: "Fix typo".into(),
        repo: repo.into(),
        body: "\nFix it.\n".into(),
        ..NewTicket::default()
    })
    .unwrap()
}

fn err_of(r: BuildNewResult) -> String {
    r.result.unwrap_err().to_string()
}

#[test]
fn build_new_for_an_inbox_ticket_by_number() {
    let t = Temp::new();
    let ws = initialised(&t);
    put(&t, "build/tickets/N-0001.md", &native("N-0001", "api"));
    let r = ws.build_new("1", "dev");
    assert!(r.room_created);
    let made = r.result.unwrap().value;
    assert_eq!(
        made,
        NewBuild {
            id: "N-0001".into(),
            repo: "api".into(),
            rel: "build/tickets/N-0001.md".into()
        }
    );
    assert!(t.exists("build/api/changes/N-0001"));
    let state = t.read("STATE.json");
    assert!(
        state.contains("\"N-0001\"") && state.contains("\"loop\": \"build\""),
        "{state}"
    );
    assert!(state.contains("build/tickets/N-0001.md"));
    assert!(t.read("LOG.md").contains("N-0001: created"));
    // A second run refuses, and reports no new room.
    let again = ws.build_new("N-0001", "dev");
    assert!(!again.room_created);
    assert_eq!(
        err_of(again),
        "daiku: N-0001 is already a change — run `daiku switch N-0001`"
    );
}

#[test]
fn build_new_for_a_planned_ticket_reads_it_in_place() {
    let t = Temp::new();
    let ws = initialised(&t);
    put(
        &t,
        "plan/specs/1-x/tickets/S-0002.md",
        &planned("S-0002", STATUS_READY, "api"),
    );
    let made = ws.build_new("S-0002", "dev").result.unwrap().value;
    assert_eq!(made.rel, "plan/specs/1-x/tickets/S-0002.md");
    assert!(
        t.read("STATE.json")
            .contains("plan/specs/1-x/tickets/S-0002.md"),
        "the plan ticket is the change's source"
    );
    assert!(
        t.exists("plan/specs/1-x/tickets/S-0002.md"),
        "the ticket stays where it is"
    );
}

#[test]
fn build_new_refuses_what_cant_be_built_before_writing_anything() {
    let t = Temp::new();
    let ws = initialised(&t);
    put(
        &t,
        "plan/specs/1-x/tickets/S-0002.md",
        &planned("S-0002", "draft", "api"),
    );
    put(
        &t,
        "plan/specs/1-x/tickets/S-0003.md",
        &planned("S-0003", STATUS_READY, "web"),
    );
    put(&t, "build/tickets/N-0004.md", &native("N-0004", "api"));
    put(
        &t,
        "plan/specs/1-x/tickets/N-0004.md",
        &native("N-0004", "api"),
    );
    put(
        &t,
        "build/tickets/S-0005.md",
        &planned("S-0005", STATUS_READY, "api"),
    );
    t.write(
        "build/tickets/N-0006.md",
        &String::from_utf8(native("N-0007", "api").to_bytes()).unwrap(),
    );
    let before = (t.read("STATE.json"), t.exists("build/api"));
    for (arg, want) in [
        ("a b", "daiku: \"a b\" isn't a ticket id"),
        ("../x", "daiku: \"../x\" isn't a ticket id"),
        (
            "N-0099",
            "daiku: no ticket N-0099 in plan/specs/*/tickets/ or build/tickets/",
        ),
        (
            "S-0002",
            "daiku: S-0002 is draft; only a ready or published planned ticket can be built",
        ),
        (
            "S-0003",
            "daiku: S-0003's repo \"web\" isn't registered — run `daiku repo add web <remote>`",
        ),
        (
            "N-0004",
            "daiku: ticket N-0004 is in more than one place: plan/specs/1-x/tickets/N-0004.md, build/tickets/N-0004.md",
        ),
        (
            "S-0005",
            "daiku: S-0005 is a planned ticket in the inbox — planned tickets live under plan/specs/",
        ),
        (
            "N-0006",
            "daiku: build/tickets/N-0006.md holds ticket \"N-0007\", not N-0006",
        ),
    ] {
        let r = ws.build_new(arg, "dev");
        assert!(!r.room_created, "{arg}");
        assert_eq!(err_of(r), want, "{arg}");
    }
    assert_eq!((t.read("STATE.json"), t.exists("build/api")), before);
}

// ---- commit and pr (T6) ----

fn published_ticket() -> Ticket {
    let mut t = Ticket::new(NewTicket {
        id: "S-0014".into(),
        typ: STORY.into(),
        title: "Add release workflow".into(),
        repo: "api".into(),
        change: "1-x".into(),
        spec: "plan/specs/1-x/functional-spec.md".into(),
        adrs: vec!["ADR-0004".into()],
        body: "\n## Acceptance criteria\n- [ ] AC1: Given a tag, When it's pushed, Then a release is built.\n".into(),
        ..NewTicket::default()
    })
    .unwrap();
    t.mark_ready().unwrap();
    t.mark_published("github", "org/app#7").unwrap();
    t
}

#[test]
fn pr_body_links_the_ticket() {
    let t = published_ticket();
    assert_eq!(
        pr_body(&t, "plan/specs/1-x"),
        "Implements S-0014: Add release workflow\n\n- **github:** org/app#7\n- **Plan:** `plan/specs/1-x/`\n- **Spec:** `plan/specs/1-x/functional-spec.md`\n- **ADRs:** ADR-0004\n"
    );
    assert_eq!(
        pr_body(&native("N-0001", "api"), ""),
        "Implements N-0001: Fix typo\n"
    );
    let mut origin = Ticket::parse(
        b"---\nschema: 1\nid: S-0001\ntype: story\ntitle: t\nrepo: a\nexternal:\n  jira: P-1\n  ado: 7\norigin:\n  workspace: git@h:o/p.git\n  path: specs/x.md\n---\n",
    )
    .unwrap();
    origin.spec = String::new();
    assert_eq!(
        pr_body(&origin, "plan/specs/x/"),
        "Implements S-0001: t\n\n- **ado:** 7\n- **jira:** P-1\n- **Planned in:** git@h:o/p.git (`specs/x.md`)\n- **Plan:** `plan/specs/x/`\n"
    );
}

#[test]
fn plan_folder_comes_from_a_planned_source() {
    let build = |src: &str| Change {
        sources: vec![src.into()],
        ..Change::default()
    };
    assert_eq!(
        plan_folder(&build("plan/specs/3-release/tickets/S-0014.md")),
        "plan/specs/3-release"
    );
    assert_eq!(plan_folder(&build("build/tickets/N-0001.md")), "");
    assert_eq!(plan_folder(&build("plan/specs/3-release/README.md")), "");
    assert_eq!(plan_folder(&Change::default()), "");
}

/// A workspace whose build change `id` is active, in repo `api` checked out at
/// `code/api`, with `rev-parse` already answered for each call the test makes.
fn build_in_a_checkout(
    t: &Temp,
    ticket_rel: &str,
    ticket: &Ticket,
    id: &str,
) -> (Workspace, Rc<ScriptedProcess>) {
    let (ws, process, _) = workspace(t);
    ws.init(&InitOptions {
        no_git: true,
        force: false,
        vcs: "none".into(),
        version: "dev".into(),
    })
    .unwrap();
    std::fs::create_dir_all(t.p("code/api")).unwrap();
    ws.repo_add(
        "api",
        "https://example.invalid/api",
        Some(&t.p("code/api").to_string_lossy()),
        "",
    )
    .unwrap();
    t.write(
        "STATE.json",
        r#"{"schema":1,"changes":{"1-x":{"loop":"plan","phase":"breakdown","status":"approved","gates":{},"sources":[],"repos":["api"],"updatedAt":"x"}}}"#,
    );
    std::fs::create_dir_all(t.p("plan/specs/1-x/tickets")).unwrap();
    put(t, ticket_rel, ticket);
    ws.build_new(id, "dev").result.unwrap();
    (ws, process)
}

fn reply(p: &ScriptedProcess, code: i32, stdout: &str) {
    p.reply(Output {
        code: Some(code),
        stdout: stdout.as_bytes().to_vec(),
        ..Output::default()
    });
}

fn call_args(p: &ScriptedProcess) -> Vec<Vec<String>> {
    p.calls.borrow().iter().map(|c| c.args.clone()).collect()
}

/// AC3: `commit` makes Go's calls in Go's order, the commit attached.
#[test]
fn commit_runs_rev_parse_diff_and_an_attached_commit() {
    let t = Temp::new();
    let (ws, p) = build_in_a_checkout(
        &t,
        "build/tickets/N-0001.md",
        &native("N-0001", "api"),
        "N-0001",
    );
    reply(&p, 0, "true\n"); // rev-parse
    reply(&p, 1, ""); // diff --cached --quiet: something is staged
    reply(&p, 0, ""); // git commit
    ws.commit(&["-m".into(), "feat: x".into()]).unwrap();
    assert_eq!(
        call_args(&p),
        [
            vec!["rev-parse", "--is-inside-work-tree"],
            vec!["diff", "--cached", "--quiet"],
            vec!["commit", "-m", "feat: x"],
        ]
    );
    let checkout = t.p("code/api");
    assert!(
        p.calls
            .borrow()
            .iter()
            .all(|c| c.program == "git" && c.dir.as_deref() == Some(checkout.as_path()))
    );
    assert_eq!(*p.attached.borrow(), [2], "only the commit is attached");
}

#[test]
fn commit_refuses_when_nothing_is_staged_and_reports_a_failed_commit() {
    let t = Temp::new();
    let (ws, p) = build_in_a_checkout(
        &t,
        "build/tickets/N-0001.md",
        &native("N-0001", "api"),
        "N-0001",
    );
    reply(&p, 0, "true\n");
    reply(&p, 0, "");
    assert_eq!(
        ws.commit(&[]).unwrap_err().to_string(),
        "daiku commit: nothing staged — stage one logical change with `git add`, then commit it"
    );
    assert!(p.attached.borrow().is_empty(), "git commit never ran");
    reply(&p, 0, "true\n");
    reply(&p, 1, "");
    reply(&p, 1, "");
    assert_eq!(ws.commit(&[]).unwrap_err().to_string(), "exit status 1");
}

/// AC3: `pr` makes Go's calls: rev-parse, the branch, then gh with Go's body and
/// title, attached.
#[test]
fn pr_runs_gh_with_the_body_and_title() {
    let t = Temp::new();
    let ticket = published_ticket();
    let (ws, p) = build_in_a_checkout(&t, "plan/specs/1-x/tickets/S-0014.md", &ticket, "S-0014");
    reply(&p, 0, "true\n"); // rev-parse
    reply(&p, 0, "s-0014-add-release-workflow\n"); // branch --show-current
    let out = ws.pr_command(&[]).unwrap();
    assert!(out.warnings.is_empty(), "the branch includes the id");
    let cmd = out.value;
    assert_eq!(cmd.program, "gh");
    assert_eq!(std::path::Path::new(&cmd.dir), t.p("code/api"));
    assert_eq!(
        cmd.args,
        [
            "pr",
            "create",
            "--body",
            "Implements S-0014: Add release workflow\n\n- **github:** org/app#7\n- **Plan:** `plan/specs/1-x/`\n- **Spec:** `plan/specs/1-x/functional-spec.md`\n- **ADRs:** ADR-0004\n",
            "--title",
            "[S-0014] Add release workflow",
        ]
    );
    reply(&p, 0, "");
    ws.run_ship(&cmd).unwrap();
    assert_eq!(
        call_args(&p),
        [
            vec!["rev-parse", "--is-inside-work-tree"],
            vec!["branch", "--show-current"],
            cmd.args.iter().map(String::as_str).collect::<Vec<_>>(),
        ]
    );
    assert_eq!(*p.attached.borrow(), [2]);
    assert_eq!(p.calls.borrow()[2].program, "gh");
}

#[test]
fn pr_uses_a_given_title_and_warns_about_a_branch_without_the_id() {
    let t = Temp::new();
    let (ws, p) = build_in_a_checkout(
        &t,
        "plan/specs/1-x/tickets/S-0014.md",
        &published_ticket(),
        "S-0014",
    );
    reply(&p, 0, "true\n");
    reply(&p, 0, "main\n");
    let out = ws
        .pr_command(&["My title".into(), "--draft".into()])
        .unwrap();
    assert_eq!(out.warnings.len(), 1);
    assert_eq!(
        out.warnings[0].message,
        "branch main doesn't include S-0014 — `daiku status` shows the suggested name"
    );
    assert_eq!(out.value.args[4..], ["--title", "My title"]);
    // A flag first means no title argument, so the ticket's is used.
    reply(&p, 0, "true\n");
    reply(&p, 1, ""); // no branch (detached HEAD): no warning
    let out = ws.pr_command(&["--draft".into()]).unwrap();
    assert!(out.warnings.is_empty());
    assert_eq!(out.value.args[5], "[S-0014] Add release workflow");
}

#[test]
fn run_ship_words_failures_as_go_does() {
    let t = Temp::new();
    let (ws, p, _) = workspace(&t);
    let cmd = ShipCommand {
        dir: t.path().to_string_lossy().into_owned(),
        program: "gh".into(),
        args: vec!["pr".into(), "create".into()],
    };
    reply(&p, 4, "");
    assert_eq!(ws.run_ship(&cmd).unwrap_err().to_string(), "exit status 4");
    // The scripted runner has nothing left to reply with: a missing program.
    let path = if cfg!(windows) { "%PATH%" } else { "$PATH" };
    assert_eq!(
        ws.run_ship(&cmd).unwrap_err().to_string(),
        format!("exec: \"gh\": executable file not found in {path}")
    );
}

#[test]
fn commit_and_pr_need_an_active_build_in_a_usable_checkout() {
    let t = Temp::new();
    let (ws, p, _) = workspace(&t);
    assert_eq!(
        ws.commit(&[]).unwrap_err().to_string(),
        "daiku: commit and pr need an active build — run `daiku build new <ticket-id>`"
    );
    assert_eq!(
        ws.pr_command(&[]).unwrap_err().to_string(),
        "daiku: commit and pr need an active build — run `daiku build new <ticket-id>`"
    );
    assert!(p.calls.borrow().is_empty());
}

/// Unix only: the messages embed a path spelled with `/`, which Windows writes with `\`.
#[cfg(unix)]
#[test]
fn repo_dir_must_be_a_folder_inside_a_git_work_tree() {
    let t = Temp::new();
    let (ws, p) = build_in_a_checkout(
        &t,
        "build/tickets/N-0001.md",
        &native("N-0001", "api"),
        "N-0001",
    );
    let checkout = t.p("code/api").to_string_lossy().into_owned();
    // Not a checkout: git says no.
    reply(&p, 128, "");
    assert_eq!(
        ws.commit(&[]).unwrap_err().to_string(),
        format!(
            "daiku: api's local path {checkout} isn't a git checkout — run `daiku repo path api <dir>`"
        )
    );
    // Prints something other than `true` (a bare repo).
    reply(&p, 0, "false\n");
    assert!(
        ws.commit(&[])
            .unwrap_err()
            .to_string()
            .contains("isn't a git checkout")
    );
    // The path is gone.
    std::fs::remove_dir_all(t.p("code/api")).unwrap();
    assert_eq!(
        ws.commit(&[]).unwrap_err().to_string(),
        format!(
            "daiku: api has no usable local path ({checkout}) — run `daiku repo path api <dir>`"
        )
    );
}
