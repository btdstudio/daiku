//! Overview tests: Go's `overview_test.go` and `markers_test.go`, by behaviour.

use std::collections::BTreeMap;

use super::markers::{merge, narrative, stub};
use super::render::{BuildRow, Input, render};
use crate::plan::model::{Model, load};
use crate::testutil::Temp;

const PHASES: [&str; 8] = [
    "intake",
    "research",
    "explore",
    "synthesis",
    "spec",
    "architecture",
    "breakdown",
    "publish",
];
const NONE: &str = "_None yet._";

fn tk(id: &str, typ: &str, title: &str, change: &str, status: &str, extra: &str) -> String {
    let repo = if typ == "story" || typ == "task" {
        "repo: app\n"
    } else {
        ""
    };
    format!(
        "---\nschema: 1\nid: {id}\ntype: {typ}\ntitle: {}\n{repo}change: {change}\nstatus: {status}\n{extra}---\n\n## Context\nSomething.\n\n## Acceptance criteria\n- [ ] AC1: it works\n",
        crate::gofmt::quote(title)
    )
}

fn model(files: &[(&str, String)]) -> (Temp, Model) {
    let t = Temp::new();
    for (p, text) in files {
        t.write(p, text);
    }
    std::fs::create_dir_all(t.p("plan/specs/1-auth")).unwrap();
    let m = load(t.path(), "1-auth", "").unwrap();
    (t, m)
}

fn gates(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn draw(m: &Model, g: &[(&str, &str)], narrative_empty: bool) -> String {
    let reasons = gates(&[("research", "nothing | to find")]);
    render(&Input {
        model: m,
        phases: PHASES.to_vec(),
        gates: &gates(g),
        reasons: &reasons,
        repos: vec!["app".into()],
        adr_dir: "plan/04_architecture/adr",
        narrative_empty,
        builds: Vec::new(),
    })
}

fn section(out: &str, title: &str) -> String {
    let head = format!("\n## {title}\n\n");
    let start = out
        .find(&head)
        .unwrap_or_else(|| panic!("no {title} section:\n{out}"));
    let mut body = &out[start + head.len()..];
    if let Some(end) = body.find("\n\n## ") {
        body = &body[..end];
    }
    body.strip_suffix("\n\n").unwrap_or(body).to_string()
}

#[test]
fn empty_change() {
    let (_t, m) = model(&[]);
    let out = draw(&m, &[], false);
    assert!(
        out.starts_with("\n## Status\n\n") && out.ends_with("\n\n"),
        "{out:?}"
    );
    let titles: Vec<&str> = out.lines().filter_map(|l| l.strip_prefix("## ")).collect();
    assert_eq!(
        titles,
        [
            "Status",
            "Diagrams",
            "Work map",
            "Dependencies",
            "Repos",
            "Builds",
            "Decisions",
            "Traceability",
            "Problems"
        ]
    );
    for s in &titles[1..] {
        assert_eq!(section(&out, s), NONE, "{s}");
    }
}

#[test]
fn status_and_diagrams() {
    let (_t, m) = model(&[]);
    let out = draw(
        &m,
        &[("intake", "approved"), ("research", "skipped")],
        false,
    );
    assert_eq!(
        section(&out, "Status"),
        "| Phase | Status | Note |\n|---|---|---|\n| intake | approved |  |\n| research | skipped | nothing \\| to find |\n\
         | explore | — |  |\n| synthesis | — |  |\n| spec | — |  |\n| architecture | — |  |\n| breakdown | — |  |\n| publish | — |  |"
    );
    let arch = "# A\n\n## Diagrams\n\n### Context\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n### Layers\n\nLater.\n";
    let (_t, m) = model(&[("plan/specs/1-auth/architecture.md", arch.into())]);
    assert_eq!(
        section(&draw(&m, &[], false), "Diagrams"),
        "### Context\n\n```mermaid\nflowchart LR\n  a --> b\n```\n\n### Layers\n\n_No diagram yet._\n\nFrom [architecture.md](architecture.md)."
    );
    let (_t, m) = model(&[("plan/specs/1-auth/architecture.md", "# A\n".into())]);
    assert_eq!(
        section(&draw(&m, &[], false), "Diagrams"),
        format!("{NONE}\n\nFrom [architecture.md](architecture.md).")
    );
}

#[test]
fn work_map() {
    let (_t, m) = model(&[
        (
            "plan/specs/1-auth/tickets/E-0001.md",
            tk("E-0001", "epic", "Accounts", "1-auth", "draft", ""),
        ),
        (
            "plan/specs/1-auth/tickets/S-0003.md",
            tk(
                "S-0003",
                "story",
                "Sign out",
                "1-auth",
                "ready",
                "parent: E-0001\nrequirements: [R2]\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0002.md",
            tk(
                "S-0002",
                "story",
                "Sign in",
                "1-auth",
                "published",
                "parent: E-0001\nrequirements: [R1, N1]\nexternal:\n  jira: AUTH-7\n  github: o/app#7\npublished_hash: \"v2:x\"\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/T-0004.md",
            tk(
                "T-0004",
                "task",
                "Child of another change",
                "1-auth",
                "draft",
                "parent: F-0009\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0005.md",
            tk(
                "S-0005",
                "story",
                "Loop A",
                "1-auth",
                "draft",
                "parent: S-0006\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0006.md",
            tk(
                "S-0006",
                "story",
                "Loop B",
                "1-auth",
                "draft",
                "parent: S-0005\n",
            ),
        ),
        (
            "plan/specs/2-other/tickets/F-0009.md",
            tk("F-0009", "feature", "Elsewhere", "2-other", "draft", ""),
        ),
    ]);
    assert_eq!(
        section(&draw(&m, &[], false), "Work map"),
        "- [**E-0001**](tickets/E-0001.md) Accounts · draft\n\
         \x20 - [**S-0002**](tickets/S-0002.md) Sign in · `app` · published · github o/app#7, jira AUTH-7 · R1, N1\n\
         \x20 - [**S-0003**](tickets/S-0003.md) Sign out · `app` · ready · R2\n\
         - [**T-0004**](tickets/T-0004.md) Child of another change · `app` · draft\n\
         - [**S-0005**](tickets/S-0005.md) Loop A · `app` · draft\n\
         \x20 - [**S-0006**](tickets/S-0006.md) Loop B · `app` · draft"
    );
}

#[test]
fn dependencies() {
    let (_t, m) = model(&[
        (
            "plan/specs/1-auth/tickets/E-0001.md",
            tk("E-0001", "epic", "Accounts", "1-auth", "draft", ""),
        ),
        (
            "plan/specs/1-auth/tickets/S-0002.md",
            tk(
                "S-0002",
                "story",
                "Sign \"in\"",
                "1-auth",
                "draft",
                "parent: E-0001\ndepends_on: [S-0008, S-0003, S-0099]\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0003.md",
            tk(
                "S-0003",
                "story",
                "A title that is much longer than forty characters in all",
                "1-auth",
                "draft",
                "parent: E-0001\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0004.md",
            tk(
                "S-0004",
                "story",
                "Loose",
                "1-auth",
                "draft",
                "depends_on: [S-0002]\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0010.md",
            tk("S-0010", "story", "Alone", "1-auth", "draft", ""),
        ),
        (
            "plan/specs/2-other/tickets/S-0008.md",
            tk("S-0008", "story", "Session store", "2-other", "draft", ""),
        ),
    ]);
    let out = draw(&m, &[], false);
    assert_eq!(
        section(&out, "Dependencies"),
        "### E-0001 · Accounts\n\n```mermaid\nflowchart LR\n\
         \x20 E_0001[\"E-0001 · Accounts\"]\n\
         \x20 S_0002[\"S-0002 · Sign #quot;in#quot;\"]\n\
         \x20 S_0003[\"S-0003 · A title that is much longer than forty…\"]\n\
         \x20 S_0008[\"S-0008 · Session store (2-other)\"]:::external\n\
         \x20 S_0099[\"S-0099 · not found\"]:::external\n\
         \x20 S_0003 --> S_0002\n  S_0008 --> S_0002\n  S_0099 --> S_0002\n\
         \x20 classDef external stroke-dasharray: 5 5\n```\n\n\
         ### No epic\n\n```mermaid\nflowchart LR\n\
         \x20 S_0004[\"S-0004 · Loose\"]\n  S_0010[\"S-0010 · Alone\"]\n\
         \x20 S_0002[\"S-0002 · Sign #quot;in#quot;\"]:::external\n\
         \x20 S_0002 --> S_0004\n\
         \x20 classDef external stroke-dasharray: 5 5\n```"
    );
    assert!(!section(&out, "Work map").contains("S-0008"));
}

#[test]
fn dependencies_fall_back_to_a_table() {
    for (stories, table) in [(23, false), (24, true)] {
        let mut files = vec![
            (
                "plan/specs/1-auth/tickets/E-0001.md".to_string(),
                tk("E-0001", "epic", "Big", "1-auth", "draft", ""),
            ),
            (
                "plan/specs/2-other/tickets/S-0900.md".to_string(),
                tk("S-0900", "story", "Outside", "2-other", "draft", ""),
            ),
        ];
        for i in 1..=stories {
            let id = format!("S-{:04}", i + 1);
            let mut extra = String::from("parent: E-0001\n");
            if i == 1 {
                extra.push_str("depends_on: [S-0900]\n");
            }
            files.push((
                format!("plan/specs/1-auth/tickets/{id}.md"),
                tk(
                    &id,
                    "story",
                    &format!("Story {id}"),
                    "1-auth",
                    "draft",
                    &extra,
                ),
            ));
        }
        let refs: Vec<(&str, String)> =
            files.iter().map(|(p, t)| (p.as_str(), t.clone())).collect();
        let (_t, m) = model(&refs);
        let body = section(&draw(&m, &[], false), "Dependencies");
        assert_eq!(
            body.contains("| Ticket | Depends on |"),
            table,
            "{stories}\n{body}"
        );
        if table {
            assert!(
                body.contains("| [S-0002](tickets/S-0002.md) | S-0900 (2-other) |"),
                "{body}"
            );
        }
    }
}

#[test]
fn repos_decisions_traceability() {
    let (_t, m) = model(&[
        (
            "plan/specs/1-auth/functional-spec.md",
            "# Functional spec — Auth\n\n## Functional requirements\n\n- R1. A. — **must**\n- R2. B. — **must**\n- R3. C. — **should**\n- R4. D.\n- N1. E. — **must**\n- N2. F. — **must**\n".into(),
        ),
        ("plan/specs/1-auth/tickets/E-0001.md", tk("E-0001", "epic", "Accounts", "1-auth", "draft", "adrs: [ADR-0001]\n")),
        ("plan/specs/1-auth/tickets/S-0002.md", tk("S-0002", "story", "Sign in", "1-auth", "ready", "requirements: [R1, N1]\nadrs: [ADR-0001, ADR-0009]\n")),
        ("plan/specs/1-auth/tickets/S-0003.md", tk("S-0003", "story", "Sign out", "1-auth", "draft", "requirements: [R1]\nadrs: [\"0002\"]\n")),
        ("plan/04_architecture/adr/0001-use-tokens.md", "# 0001 — Use tokens\n\n- Status: accepted\n".into()),
    ]);
    let out = draw(&m, &[], false);
    assert_eq!(
        section(&out, "Repos"),
        "| Repo | draft | ready | published | Tickets |\n|---|---|---|---|---|\n| `app` | 1 | 1 | 0 | [S-0002](tickets/S-0002.md), [S-0003](tickets/S-0003.md) |"
    );
    assert_eq!(
        section(&out, "Decisions"),
        "| ADR | Title | Status | Cited by |\n|---|---|---|---|\n\
         | 0002 | not found |  | [S-0003](tickets/S-0003.md) |\n\
         | [ADR-0001](../../../plan/04_architecture/adr/0001-use-tokens.md) | Use tokens | accepted | [E-0001](tickets/E-0001.md), [S-0002](tickets/S-0002.md) |\n\
         | ADR-0009 | not found |  | [S-0002](tickets/S-0002.md) |"
    );
    assert_eq!(
        section(&out, "Traceability"),
        "From [functional-spec.md](functional-spec.md).\n\n| Req | Priority | Covered by |\n|---|---|---|\n\
         | R1 | must | [S-0002](tickets/S-0002.md), [S-0003](tickets/S-0003.md) |\n\
         | R2 | must | **uncovered** |\n| R3 | should |  |\n| R4 | — |  |\n\
         | N1 | must | [S-0002](tickets/S-0002.md) |\n| N2 | must |  |"
    );
}

#[test]
fn problems() {
    let (_t, m) = model(&[
        (
            "plan/specs/1-auth/functional-spec.md",
            "## Functional requirements\n\n- R1. A. — **must**\n".into(),
        ),
        (
            "plan/specs/1-auth/tickets/S-0002.md",
            tk(
                "S-0002",
                "story",
                "Sign in",
                "1-auth",
                "draft",
                "requirements: [R9]\nparent: F-0099\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0003.md",
            "---\nschema: [\n---\n".into(),
        ),
        (
            "plan/specs/1-auth/tickets/S-0004.md",
            tk("S-0004", "story", "No id", "1-auth", "draft", "").replacen("id: S-0004\n", "", 1),
        ),
        (
            "plan/specs/2-other/tickets/S-0007.md",
            tk(
                "S-0007",
                "story",
                "Elsewhere",
                "2-other",
                "draft",
                "parent: F-0098\nrequirements: [R5]\n",
            ),
        ),
        (
            "plan/specs/2-other/functional-spec.md",
            "## Functional requirements\n\n- R1. Other. — **must**\n- R1. Again. — **must**\n"
                .into(),
        ),
    ]);
    let body = section(&draw(&m, &[("architecture", "approved")], true), "Problems");
    let lines: Vec<&str> = body.lines().collect();
    let want = [
        "- ! R1: must requirement in 1-auth isn't cited by any ticket",
        "- ! README: narrative is empty",
        "- ✗ [S-0002](tickets/S-0002.md): cites R9, which isn't in 1-auth's spec",
        "- ✗ [S-0002](tickets/S-0002.md): parent F-0099 doesn't exist",
        "- ✗ tickets/S-0003.md: ",
        "- ✗ tickets/S-0004.md: ",
    ];
    assert_eq!(lines.len(), want.len(), "{body}");
    for (l, w) in lines.iter().zip(want) {
        assert!(l.starts_with(w), "{l:?} should start with {w:?}\n{body}");
    }
    assert!(
        !body.contains("S-0007") && !body.contains("/specs"),
        "{body}"
    );
    assert!(!draw(&m, &[("architecture", "draft")], true).contains("narrative is empty"));
    assert!(!draw(&m, &[("architecture", "approved")], false).contains("narrative is empty"));
}

#[test]
fn builds() {
    let (_t, m) = model(&[(
        "plan/specs/1-auth/tickets/S-0002.md",
        tk("S-0002", "story", "Sign in", "1-auth", "ready", ""),
    )]);
    let row =
        |id: &str, repo: &str, phase: &str, status: &str, started: bool, closed: bool| BuildRow {
            id: id.into(),
            repo: repo.into(),
            phase: phase.into(),
            status: status.into(),
            started,
            closed,
        };
    let empty = BTreeMap::new();
    let out = render(&Input {
        model: &m,
        phases: PHASES.to_vec(),
        gates: &empty,
        reasons: &empty,
        repos: Vec::new(),
        adr_dir: "",
        narrative_empty: false,
        builds: vec![
            row("S-0002", "app", "implement", "in-progress", true, false),
            row("S-0003", "app", "", "", false, false),
            row("S-0004", "web", "ship", "complete", true, false),
            row("S-0005", "web", "test", "approved", true, true),
            row("X-12", "", "design", "draft", true, false),
        ],
    });
    assert_eq!(
        section(&out, "Builds"),
        "| Ticket | Repo | Build |\n|---|---|---|\n\
         | [S-0002](tickets/S-0002.md) | `app` | implement in-progress |\n\
         | S-0003 | `app` | not started |\n\
         | S-0004 | `web` | ship complete (done) |\n\
         | S-0005 | `web` | closed |\n\
         | X-12 | — | design draft |"
    );
    let (r, b, d) = (
        out.find("## Repos").unwrap(),
        out.find("## Builds").unwrap(),
        out.find("## Decisions").unwrap(),
    );
    assert!(r < b && b < d);
}

#[test]
fn cells_stay_on_one_line() {
    let (_t, m) = model(&[]);
    let evil = "x\n<!-- daiku:narrative -->\r\n<!-- /daiku:generated -->\u{2028}y";
    let g = gates(&[("intake", evil)]);
    let out = render(&Input {
        model: &m,
        phases: PHASES.to_vec(),
        gates: &g,
        reasons: &g,
        repos: Vec::new(),
        adr_dir: "",
        narrative_empty: false,
        builds: vec![BuildRow {
            id: evil.into(),
            repo: evil.into(),
            phase: evil.into(),
            status: evil.into(),
            started: true,
            closed: false,
        }],
    });
    for l in out.lines() {
        assert!(!l.starts_with("<!--"), "{l:?}\n{out}");
    }
    assert!(!out.contains(['\r', '\u{2028}']));
    let merged = merge(&stub("T"), &out).unwrap();
    narrative(&merged).unwrap();
}

#[test]
fn deterministic() {
    let (_t, m) = model(&[
        (
            "plan/specs/1-auth/tickets/S-0002.md",
            tk(
                "S-0002",
                "story",
                "B",
                "1-auth",
                "draft",
                "depends_on: [S-0003, S-0001]\nexternal:\n  b: 2\n  a: 1\n",
            ),
        ),
        (
            "plan/specs/1-auth/tickets/S-0001.md",
            tk("S-0001", "story", "A", "1-auth", "draft", ""),
        ),
        (
            "plan/specs/1-auth/tickets/S-0003.md",
            tk("S-0003", "story", "C", "1-auth", "draft", ""),
        ),
    ]);
    let first = draw(&m, &[], false);
    for _ in 0..5 {
        assert_eq!(draw(&m, &[], false), first);
    }
}
