//! `Workspace::next`: the walk, the candidate rule, dependencies, order and `since`
//! (AC1-AC11). The text and `--json` forms are the fixtures'.

use std::rc::Rc;

use super::*;
use crate::fake::ScriptedProcess;
use crate::testutil::{self, Temp};

struct Shared(Rc<ScriptedProcess>);

impl crate::env::Process for Shared {
    fn output(&self, cmd: &crate::env::Cmd) -> std::io::Result<crate::env::Output> {
        self.0.output(cmd)
    }
    fn run_attached(&self, cmd: &crate::env::Cmd) -> std::io::Result<i32> {
        self.0.run_attached(cmd)
    }
}

/// One STATE entry.
fn ch(name: &str, lp: &str, status: &str, gates: &[(&str, &str)], updated: &str) -> String {
    let gates: Vec<String> = gates
        .iter()
        .map(|(k, v)| format!("\"{k}\":\"{v}\""))
        .collect();
    format!(
        "\"{name}\":{{\"loop\":\"{lp}\",\"phase\":\"x\",\"status\":\"{status}\",\"gates\":{{{}}},\"sources\":[],\"repos\":[\"app\"],\"updatedAt\":\"{updated}\"}}",
        gates.join(",")
    )
}

fn plan(name: &str, gates: &[(&str, &str)], updated: &str) -> String {
    ch(name, "plan", "approved", gates, updated)
}

fn build(name: &str, gates: &[(&str, &str)], updated: &str) -> String {
    ch(name, "build", "approved", gates, updated)
}

/// A workspace with these STATE entries, repos `app` and `old` (archived).
fn ws(t: &Temp, entries: &[String]) -> Workspace {
    t.write(
        ".daiku/settings.json",
        "{\"overview\": \"manual\", \"vcs\": \"none\"}\n",
    );
    t.write(
        "STATE.json",
        &format!("{{\"schema\":1,\"changes\":{{{}}}}}", entries.join(",")),
    );
    t.write(
        "REPOS.json",
        "{\"schema\": 1, \"repos\": {\"app\": {\"remote\": \"https://example.com/app.git\"}, \"old\": {\"remote\": \"https://example.com/old.git\", \"archived\": true}}}\n",
    );
    let env = testutil::env(t, Box::new(Shared(Rc::new(ScriptedProcess::default()))));
    Workspace::open(t.path(), env)
}

fn story(id: &str, status: &str, repo: &str, extra: &str) -> String {
    let external = if status == "published" {
        "external:\n  jira: J-1\n"
    } else {
        ""
    };
    format!(
        "---\nschema: 1\nid: {id}\ntype: story\ntitle: Ticket {id}\nrepo: {repo}\nstatus: {status}\n{external}{extra}---\nBody\n"
    )
}

fn native(id: &str, status: &str, repo: &str) -> String {
    format!(
        "---\nschema: 1\nid: {id}\ntype: task\ntitle: Native {id}\nrepo: {repo}\nstatus: {status}\n---\nBody\n"
    )
}

fn put_plan_ticket(t: &Temp, plan: &str, id: &str, text: &str) {
    t.write(&format!("plan/specs/{plan}/tickets/{id}.md"), text);
}

fn items(ws: &Workspace) -> Vec<NextItem> {
    ws.next().unwrap().value.items
}

/// `(kind, change-or-ticket, phase)` for compact assertions.
fn brief(items: &[NextItem]) -> Vec<String> {
    items
        .iter()
        .map(|i| match i {
            NextItem::Approve { change, phase, .. } => format!("approve {change} {phase}"),
            NextItem::Start { change, phase, .. } => format!("start {change} {phase}"),
            NextItem::Build { ticket, .. } => format!("build {ticket}"),
        })
        .collect()
}

fn build_ids(ws: &Workspace) -> Vec<String> {
    items(ws)
        .into_iter()
        .filter_map(|i| match i {
            NextItem::Build { ticket, .. } => Some(ticket),
            _ => None,
        })
        .collect()
}

// first_open

fn change(lp: &str, gates: &[(&str, &str)]) -> Change {
    let mut c = Change::new("c");
    c.loop_ = lp.to_string();
    for (k, v) in gates {
        c.gates.insert(k.to_string(), v.to_string());
    }
    c
}

#[test]
fn walk_start_approve_and_continue() {
    let (p, a) = ("plan", "approved");
    assert_eq!(first_open(&change(p, &[])), Some((Kind::Start, "intake")));
    assert_eq!(
        first_open(&change(p, &[("intake", "await-approval")])),
        Some((Kind::Approve, "intake"))
    );
    // research approved, synthesis skipped: explore is next (AC2).
    assert_eq!(
        first_open(&change(
            p,
            &[("intake", a), ("research", a), ("synthesis", "skipped")]
        )),
        Some((Kind::Start, "explore"))
    );
    // a skipped phase and an approved one both continue
    assert_eq!(
        first_open(&change(p, &[("intake", "skipped"), ("research", a)])),
        Some((Kind::Start, "explore"))
    );
}

#[test]
fn walk_starts_an_unstarted_run_phase() {
    let g = [
        ("ticket", "approved"),
        ("requirements", "approved"),
        ("design", "approved"),
        ("tasks", "approved"),
    ];
    assert_eq!(
        first_open(&change("build", &g)),
        Some((Kind::Start, "implement"))
    );
}

#[test]
fn walk_run_phases_continue_when_complete() {
    let g = [
        ("ticket", "approved"),
        ("requirements", "approved"),
        ("design", "approved"),
        ("tasks", "approved"),
        ("implement", "complete"),
    ];
    assert_eq!(
        first_open(&change("build", &g)),
        Some((Kind::Start, "test"))
    );
}

#[test]
fn walk_gives_nothing_for_draft_in_progress_unknown_or_the_end() {
    assert_eq!(first_open(&change("plan", &[("intake", "draft")])), None);
    let g = [
        ("ticket", "approved"),
        ("requirements", "approved"),
        ("design", "approved"),
        ("tasks", "approved"),
        ("implement", "in-progress"),
    ];
    assert_eq!(first_open(&change("build", &g)), None);
    assert_eq!(first_open(&change("plan", &[("intake", "weird")])), None);
    let all: Vec<(&str, &str)> = [
        "ticket",
        "requirements",
        "design",
        "tasks",
        "implement",
        "test",
        "review",
        "security",
        "ship",
    ]
    .iter()
    .map(|p| (*p, "complete"))
    .collect();
    assert_eq!(first_open(&change("build", &all)), None);
}

// next: the walk through STATE

#[test]
fn active_build_approve_comes_before_other_plan_approve() {
    // AC1
    let t = Temp::new();
    let w = ws(
        &t,
        &[
            plan(
                "1-x",
                &[
                    ("intake", "approved"),
                    ("research", "skipped"),
                    ("explore", "skipped"),
                    ("synthesis", "skipped"),
                    ("spec", "await-approval"),
                ],
                "2026-01-02T00:00:00.000Z",
            ),
            build(
                "S-0013",
                &[
                    ("ticket", "approved"),
                    ("requirements", "approved"),
                    ("design", "approved"),
                    ("tasks", "approved"),
                    ("implement", "complete"),
                    ("test", "approved"),
                    ("review", "await-approval"),
                ],
                "2026-01-03T00:00:00.000Z",
            ),
        ],
    );
    t.write(".daiku/active", "S-0013\n");
    let got = items(&w);
    assert_eq!(
        brief(&got),
        vec!["approve S-0013 review", "approve 1-x spec"]
    );
    assert!(matches!(&got[0], NextItem::Approve { active: true, .. }));
    assert!(matches!(&got[1], NextItem::Approve { active: false, .. }));
}

#[test]
fn start_explore_after_skips() {
    // AC2
    let t = Temp::new();
    let w = ws(
        &t,
        &[plan(
            "1-x",
            &[
                ("intake", "approved"),
                ("research", "approved"),
                ("synthesis", "skipped"),
            ],
            "2026-01-02T00:00:00.000Z",
        )],
    );
    t.write(".daiku/active", "1-x\n");
    assert_eq!(brief(&items(&w)), vec!["start 1-x explore"]);
}

#[test]
fn start_implement_after_tasks_approved() {
    // AC3
    let t = Temp::new();
    let w = ws(
        &t,
        &[build(
            "S-0001",
            &[
                ("ticket", "approved"),
                ("requirements", "approved"),
                ("design", "approved"),
                ("tasks", "approved"),
            ],
            "2026-01-02T00:00:00.000Z",
        )],
    );
    assert_eq!(brief(&items(&w)), vec!["start S-0001 implement"]);
}

#[test]
fn nothing_for_draft_in_progress_closed_or_shipped() {
    // AC4
    let t = Temp::new();
    let all_but_ship: Vec<(&str, &str)> = vec![
        ("ticket", "approved"),
        ("requirements", "approved"),
        ("design", "approved"),
        ("tasks", "approved"),
        ("implement", "complete"),
        ("test", "approved"),
        ("review", "approved"),
        ("security", "skipped"),
        ("ship", "complete"),
    ];
    let w = ws(
        &t,
        &[
            plan(
                "1-draft",
                &[("intake", "draft")],
                "2026-01-01T00:00:00.000Z",
            ),
            build(
                "S-0001",
                &[
                    ("ticket", "approved"),
                    ("requirements", "approved"),
                    ("design", "approved"),
                    ("tasks", "approved"),
                    ("implement", "in-progress"),
                ],
                "2026-01-01T00:00:00.000Z",
            ),
            ch(
                "2-closed",
                "plan",
                "closed",
                &[("intake", "await-approval")],
                "2026-01-01T00:00:00.000Z",
            ),
            build("S-0002", &all_but_ship, "2026-01-01T00:00:00.000Z"),
            plan(
                "3-odd",
                &[("intake", "strange")],
                "2026-01-01T00:00:00.000Z",
            ),
        ],
    );
    assert!(items(&w).is_empty());
}

#[test]
fn dangling_or_closed_active_name_means_no_active_change() {
    // AC5
    let t = Temp::new();
    let w = ws(
        &t,
        &[
            plan(
                "1-x",
                &[("intake", "await-approval")],
                "2026-01-01T00:00:00.000Z",
            ),
            ch("2-y", "plan", "closed", &[], "2026-01-01T00:00:00.000Z"),
        ],
    );
    for name in ["nope", "2-y", ""] {
        t.write(".daiku/active", &format!("{name}\n"));
        let got = items(&w);
        assert_eq!(got.len(), 1);
        assert!(
            matches!(&got[0], NextItem::Approve { active: false, .. }),
            "{name}"
        );
    }
}

#[test]
fn an_unreadable_active_file_is_an_error() {
    // AC5
    let t = Temp::new();
    let w = ws(&t, &[]);
    std::fs::create_dir_all(t.p(".daiku/active")).unwrap();
    assert!(w.next().is_err());
}

#[test]
fn an_unrecorded_plan_folder_is_not_walked() {
    let t = Temp::new();
    let w = ws(&t, &[]);
    std::fs::create_dir_all(t.p("plan/specs/1-ghost")).unwrap();
    assert!(items(&w).is_empty());
}

// dependencies (AC6)

const SHIPPED: &[(&str, &str)] = &[
    ("ticket", "approved"),
    ("requirements", "approved"),
    ("design", "approved"),
    ("tasks", "approved"),
    ("implement", "complete"),
    ("test", "approved"),
    ("review", "approved"),
    ("security", "skipped"),
    ("ship", "complete"),
];

fn two_tickets(t: &Temp, entries: &[String]) -> Workspace {
    let mut all = vec![plan("1-x", &[], "2026-01-01T00:00:00.000Z")];
    all.extend_from_slice(entries);
    let w = ws(t, &all);
    put_plan_ticket(t, "1-x", "S-0001", &story("S-0001", "published", "app", ""));
    put_plan_ticket(
        t,
        "1-x",
        "S-0002",
        &story("S-0002", "published", "app", "depends_on: [S-0001]\n"),
    );
    w
}

#[test]
fn a_dependency_still_in_review_holds_the_dependant_back() {
    let t = Temp::new();
    let w = two_tickets(
        &t,
        &[build(
            "S-0001",
            &[
                ("ticket", "approved"),
                ("requirements", "approved"),
                ("design", "approved"),
                ("tasks", "approved"),
                ("implement", "complete"),
                ("test", "approved"),
                ("review", "await-approval"),
            ],
            "2026-01-05T00:00:00.000Z",
        )],
    );
    assert!(build_ids(&w).is_empty());
}

#[test]
fn a_shipped_dependency_releases_the_dependant() {
    let t = Temp::new();
    let w = two_tickets(&t, &[build("S-0001", SHIPPED, "2026-01-05T00:00:00.000Z")]);
    assert_eq!(build_ids(&w), vec!["S-0002"]);
}

#[test]
fn a_shipped_dependency_that_is_closed_still_counts() {
    let t = Temp::new();
    let w = two_tickets(
        &t,
        &[ch(
            "S-0001",
            "build",
            "closed",
            SHIPPED,
            "2026-01-05T00:00:00.000Z",
        )],
    );
    assert_eq!(build_ids(&w), vec!["S-0002"]);
}

#[test]
fn a_closed_ticket_dependency_counts() {
    let t = Temp::new();
    let w = ws(&t, &[plan("1-x", &[], "2026-01-01T00:00:00.000Z")]);
    t.write(
        "build/tickets/N-0001.md",
        &native("N-0001", "closed", "app"),
    );
    put_plan_ticket(
        &t,
        "1-x",
        "S-0002",
        &story("S-0002", "published", "app", "depends_on: [N-0001]\n"),
    );
    assert_eq!(build_ids(&w), vec!["S-0002"]);
}

#[test]
fn a_build_closed_without_shipping_is_not_done() {
    let t = Temp::new();
    let w = two_tickets(
        &t,
        &[ch(
            "S-0001",
            "build",
            "closed",
            &[("ticket", "approved")],
            "2026-01-05T00:00:00.000Z",
        )],
    );
    assert!(build_ids(&w).is_empty());
}

#[test]
fn an_unknown_dependency_id_is_not_done() {
    let t = Temp::new();
    let w = ws(&t, &[plan("1-x", &[], "2026-01-01T00:00:00.000Z")]);
    put_plan_ticket(
        &t,
        "1-x",
        "S-0002",
        &story("S-0002", "published", "app", "depends_on: [S-0099]\n"),
    );
    assert!(build_ids(&w).is_empty());
}

// candidates (AC7, AC8, AC9)

#[test]
fn only_the_inbox_draft_is_offered_among_the_ineligible() {
    // AC7
    let t = Temp::new();
    let w = ws(
        &t,
        &[
            plan("1-x", &[], "2026-01-01T00:00:00.000Z"),
            plan("2-y", &[], "2026-01-01T00:00:00.000Z").replace("approved", "closed"),
            build(
                "S-0003",
                &[("ticket", "approved")],
                "2026-01-01T00:00:00.000Z",
            ),
        ],
    );
    t.write("build/tickets/N-0001.md", &native("N-0001", "draft", "app"));
    put_plan_ticket(&t, "1-x", "S-0001", &story("S-0001", "draft", "app", ""));
    put_plan_ticket(
        &t,
        "1-x",
        "S-0003",
        &story("S-0003", "published", "app", ""),
    );
    put_plan_ticket(
        &t,
        "1-x",
        "S-0004",
        &story("S-0004", "published", "old", ""),
    );
    put_plan_ticket(
        &t,
        "2-y",
        "S-0005",
        &story("S-0005", "published", "app", ""),
    );
    assert_eq!(build_ids(&w), vec!["N-0001"]);
}

#[test]
fn a_ready_ticket_is_offered_once_readied() {
    let t = Temp::new();
    let w = ws(
        &t,
        &[plan(
            "1-x",
            &[
                ("intake", "approved"),
                ("research", "skipped"),
                ("explore", "skipped"),
                ("synthesis", "skipped"),
                ("spec", "approved"),
                ("architecture", "approved"),
                ("breakdown", "approved"),
            ],
            "2026-01-01T00:00:00.000Z",
        )],
    );
    t.write(".daiku/active", "1-x\n");
    put_plan_ticket(
        &t,
        "1-x",
        "S-0001",
        "---\nschema: 1\nid: S-0001\ntype: story\ntitle: One\nrepo: app\nchange: 1-x\nstatus: draft\n---\n\n## Acceptance criteria\n- [ ] AC1: Given x When y Then z\n",
    );
    assert!(build_ids(&w).is_empty());
    w.tickets_ready(&crate::tickets::ReadyTargets::All, false)
        .unwrap();
    assert_eq!(build_ids(&w), vec!["S-0001"]);
}

#[test]
fn an_inbox_ticket_that_is_closed_is_not_offered() {
    let t = Temp::new();
    let w = ws(&t, &[]);
    t.write(
        "build/tickets/N-0001.md",
        &native("N-0001", "closed", "app"),
    );
    assert!(build_ids(&w).is_empty());
}

#[test]
fn drifted_empty_repo_unregistered_duplicate_and_unrecorded_plan() {
    // AC8
    let t = Temp::new();
    let w = ws(
        &t,
        &[
            plan("1-x", &[], "2026-01-01T00:00:00.000Z"),
            plan("2-y", &[], "2026-01-01T00:00:00.000Z"),
        ],
    );
    // drifted: a v2 hash that matches nothing
    put_plan_ticket(
        &t,
        "1-x",
        "S-0001",
        &story(
            "S-0001",
            "published",
            "app",
            "published_hash: \"v2:00000000000000000000000000000000.00000000000000000000000000000000\"\n",
        ),
    );
    put_plan_ticket(&t, "1-x", "S-0002", &story("S-0002", "published", "", ""));
    put_plan_ticket(
        &t,
        "1-x",
        "S-0003",
        &story("S-0003", "published", "nowhere", ""),
    );
    // the same id in two plans
    put_plan_ticket(
        &t,
        "1-x",
        "S-0004",
        &story("S-0004", "published", "app", ""),
    );
    put_plan_ticket(
        &t,
        "2-y",
        "S-0004",
        &story("S-0004", "published", "app", ""),
    );
    // a plan folder with no STATE entry
    put_plan_ticket(
        &t,
        "3-z",
        "S-0005",
        &story("S-0005", "published", "app", ""),
    );
    assert_eq!(build_ids(&w), vec!["S-0005"]);
}

#[test]
fn a_planned_id_in_the_inbox_is_not_offered() {
    let t = Temp::new();
    let w = ws(&t, &[]);
    t.write(
        "build/tickets/S-0001.md",
        &story("S-0001", "published", "app", ""),
    );
    assert!(build_ids(&w).is_empty());
}

#[test]
fn a_registry_that_fails_to_load_is_a_warning_and_no_builds() {
    // AC8
    let t = Temp::new();
    let w = ws(
        &t,
        &[plan(
            "1-x",
            &[("intake", "await-approval")],
            "2026-01-01T00:00:00.000Z",
        )],
    );
    t.write("build/tickets/N-0001.md", &native("N-0001", "draft", "app"));
    t.write("REPOS.json", "{not json");
    let out = w.next().unwrap();
    assert_eq!(brief(&out.value.items), vec!["approve 1-x intake"]);
    assert_eq!(out.warnings.len(), 1);
    assert!(out.warnings[0].message.starts_with("repos: "));
    assert!(
        out.warnings[0]
            .message
            .ends_with("; no tickets offered to build")
    );
}

#[test]
fn an_unparseable_ticket_file_is_one_warning() {
    // AC9
    let t = Temp::new();
    let w = ws(&t, &[plan("1-x", &[], "2026-01-01T00:00:00.000Z")]);
    t.write("plan/specs/1-x/tickets/bad.md", "no frontmatter\n");
    let out = w.next().unwrap();
    assert!(
        !brief(&out.value.items)
            .iter()
            .any(|b| b.starts_with("build"))
    );
    assert_eq!(out.warnings.len(), 1);
    assert!(
        out.warnings[0].message.contains("bad.md"),
        "{}",
        out.warnings[0]
    );
}

// order and since (AC10, AC11)

#[test]
fn every_kind_in_order_with_since() {
    // AC10, AC11
    let t = Temp::new();
    let w = ws(
        &t,
        &[
            plan(
                "1-act",
                &[
                    ("intake", "approved"),
                    ("research", "skipped"),
                    ("explore", "skipped"),
                    ("synthesis", "skipped"),
                    ("spec", "await-approval"),
                ],
                "2026-01-09T00:00:00.000Z",
            ),
            plan(
                "2-late",
                &[("intake", "await-approval")],
                "2026-01-08T00:00:00.000Z",
            ),
            plan(
                "3-early",
                &[("intake", "await-approval")],
                "2026-01-02T00:00:00.000Z",
            ),
            plan("4-none", &[("intake", "await-approval")], ""),
            plan(
                "5-same",
                &[("intake", "await-approval")],
                "2026-01-02T00:00:00.000Z",
            ),
            plan(
                "6-start-b",
                &[("intake", "approved")],
                "2026-01-06T00:00:00.000Z",
            ),
            plan(
                "7-start-a",
                &[("intake", "approved")],
                "2026-01-05T00:00:00.000Z",
            ),
            build("S-0001", SHIPPED, "2026-01-04T00:00:00.000Z"),
            build("S-0010", SHIPPED, "2026-01-07T00:00:00.000Z"),
        ],
    );
    t.write(".daiku/active", "1-act\n");
    put_plan_ticket(
        &t,
        "7-start-a",
        "S-0003",
        &story(
            "S-0003",
            "published",
            "app",
            "depends_on: [S-0001, S-0010]\n",
        ),
    );
    put_plan_ticket(
        &t,
        "7-start-a",
        "S-0002",
        &story("S-0002", "published", "app", ""),
    );
    t.write("build/tickets/N-0001.md", &native("N-0001", "draft", "app"));
    let got = items(&w);
    assert_eq!(
        brief(&got),
        vec![
            "approve 1-act spec",
            "approve 3-early intake",
            "approve 5-same intake",
            "approve 2-late intake",
            "approve 4-none intake",
            "start 7-start-a research",
            "start 6-start-b research",
            "build N-0001",
            "build S-0002",
            "build S-0003",
        ]
    );
    let since = |i: usize| match &got[i] {
        NextItem::Approve { since, .. }
        | NextItem::Start { since, .. }
        | NextItem::Build { since, .. } => since.clone(),
    };
    assert_eq!(since(0).as_deref(), Some("2026-01-09T00:00:00.000Z"));
    assert_eq!(since(4), None);
    // N-0001 is in the inbox: no plan, no dependencies
    assert_eq!(since(7), None);
    // S-0002 has no dependencies: its plan's
    assert_eq!(since(8).as_deref(), Some("2026-01-05T00:00:00.000Z"));
    // S-0003: the latest of its dependencies' builds
    assert_eq!(since(9).as_deref(), Some("2026-01-07T00:00:00.000Z"));
}

#[test]
fn builds_follow_dependencies_then_id() {
    let t = Temp::new();
    let w = ws(&t, &[plan("1-x", &[], "2026-01-01T00:00:00.000Z")]);
    put_plan_ticket(
        &t,
        "1-x",
        "S-0001",
        &story("S-0001", "published", "app", "depends_on: [S-0002]\n"),
    );
    put_plan_ticket(
        &t,
        "1-x",
        "S-0002",
        &story("S-0002", "published", "app", ""),
    );
    put_plan_ticket(
        &t,
        "1-x",
        "S-0003",
        &story("S-0003", "published", "app", ""),
    );
    // S-0001 waits on S-0002, which is itself still to build, so it isn't offered.
    assert_eq!(build_ids(&w), vec!["S-0002", "S-0003"]);
}

#[test]
fn a_cycle_among_candidates_falls_back_to_id_order_with_a_warning() {
    // order_tickets also follows `parent`, which `next` doesn't filter on.
    let t = Temp::new();
    let w = ws(&t, &[plan("1-x", &[], "2026-01-01T00:00:00.000Z")]);
    put_plan_ticket(
        &t,
        "1-x",
        "S-0002",
        &story("S-0002", "published", "app", "parent: S-0001\n"),
    );
    put_plan_ticket(
        &t,
        "1-x",
        "S-0001",
        &story("S-0001", "published", "app", "parent: S-0002\n"),
    );
    put_plan_ticket(
        &t,
        "1-x",
        "S-0003",
        &story("S-0003", "published", "app", ""),
    );
    let out = w.next().unwrap();
    let ids: Vec<_> = brief(&out.value.items)
        .into_iter()
        .filter(|b| b.starts_with("build"))
        .collect();
    assert_eq!(ids, vec!["build S-0001", "build S-0002", "build S-0003"]);
    assert_eq!(out.warnings.len(), 1);
    assert!(out.warnings[0].message.starts_with("tickets: "));
    assert!(
        out.warnings[0]
            .message
            .ends_with("; showing tickets to build by id")
    );
}

#[test]
fn equal_since_is_ordered_by_name() {
    let t = Temp::new();
    let ws = ws(
        &t,
        &[
            plan(
                "2-b",
                &[("intake", "await-approval")],
                "2026-01-15T00:00:00.000Z",
            ),
            plan(
                "1-a",
                &[("intake", "await-approval")],
                "2026-01-15T00:00:00.000Z",
            ),
            plan("3-c", &[("intake", "approved")], "2026-01-15T00:00:00.000Z"),
            plan("4-d", &[("intake", "approved")], "2026-01-15T00:00:00.000Z"),
        ],
    );
    assert_eq!(
        brief(&items(&ws)),
        vec![
            "approve 1-a intake",
            "approve 2-b intake",
            "start 3-c research",
            "start 4-d research",
        ]
    );
}

#[test]
fn a_ticket_whose_file_name_differs_from_its_id_is_not_offered() {
    let t = Temp::new();
    let ws = ws(&t, &[]);
    t.write("build/tickets/N-0009.md", &native("N-0001", "draft", "app"));
    t.write("build/tickets/N-0002.md", &native("N-0002", "draft", "app"));
    assert_eq!(build_ids(&ws), vec!["N-0002"]);
}
