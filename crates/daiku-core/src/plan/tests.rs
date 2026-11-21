//! Plan tests: Go's `check_test.go`, `spec_test.go`, `diagrams_test.go`,
//! `problems_test.go` and `plan_test.go`, by behaviour.

use super::check::check_artifacts;
use super::diagrams::diagrams;
use super::model::{load, load_all, title};
use super::problems::problems;
use super::spec::{Severity, parse_spec};
use crate::testutil::Temp;

const VALID_SPEC: &str = "# Functional spec — Probe\n\n## Functional requirements\n\n- R1. One. — **must**\n- N1. Two. — **must**\n";

const VALID_ARCHITECTURE: &str = "# Architecture — Probe

## Overview

The shape.

## Diagrams

### Context

```mermaid
flowchart LR
  a --> b
```

### Layers

```mermaid
flowchart TB
  c --> d
```

### Flow: approving

```mermaid
sequenceDiagram
  A->>B: go
```

## Notes

- None.
";

const SPEC_PATH: &str = "plan/specs/1-probe/functional-spec.md";
const ARCH_PATH: &str = "plan/specs/1-probe/architecture.md";
const TICKETS: &str = "plan/specs/1-probe/tickets";

/// A workspace with change 1-probe, an `app` repo, and no artifacts.
fn check_root() -> Temp {
    let t = Temp::new();
    std::fs::create_dir_all(t.p("plan/specs/1-probe")).unwrap();
    t.write(
        "REPOS.json",
        r#"{"repos": {"app": {"remote": "https://github.com/o/app", "defaultBranch": "main"}}}"#,
    );
    t
}

fn story(id: &str, change: &str, repo: &str, status: &str, extra: &str) -> String {
    format!(
        "---\nschema: 1\nid: {id}\ntype: story\ntitle: Story {id}\nrepo: {repo}\nchange: {change}\nstatus: {status}\n{extra}---\n\n## Context\nSomething.\n\n## Acceptance criteria\n- [ ] AC1: it works\n"
    )
}

fn check(t: &Temp, phase: &str, status: &str) -> Vec<String> {
    check_artifacts(t.path(), "1-probe", phase, status, "")
        .iter()
        .map(|p| p.to_string())
        .collect()
}

#[test]
fn only_checked_transitions_check() {
    let t = check_root();
    for (phase, status) in [
        ("spec", "await-approval"),
        ("spec", "draft"),
        ("architecture", "await-approval"),
        ("breakdown", "draft"),
        ("publish", "in-progress"),
        ("publish", "approved"),
        ("research", "skipped"),
        ("intake", "approved"),
        ("nonsense", "approved"),
    ] {
        assert!(check(&t, phase, status).is_empty(), "{phase} {status}");
    }
    for (phase, status) in [
        ("spec", "approved"),
        ("architecture", "approved"),
        ("breakdown", "approved"),
    ] {
        assert!(!check(&t, phase, status).is_empty(), "{phase} {status}");
    }
}

#[test]
fn check_spec() {
    let no_r = format!("{SPEC_PATH}: no R requirements under ## Functional requirements");
    let cases: Vec<(&str, Vec<String>)> = vec![
        ("", vec![format!("{SPEC_PATH}: doesn't exist")]),
        (VALID_SPEC, vec![]),
        (
            "# Functional spec — <name>\n\n## Functional requirements\n\n- R1. <capability, verifiable> — **must | should | could**\n",
            vec![no_r.clone()],
        ),
        (
            "## Functional requirements\n\n- N1. Fast. — **must**\n",
            vec![no_r.clone()],
        ),
        (
            "## Functional Requirements\n\n- R1. One. — **must**\n",
            vec![no_r.clone()],
        ),
        (
            "## Functional requirements\n\n- R1. One. — **must**\n- R1. Again. — **must**\n",
            vec![format!(
                "{SPEC_PATH}: line 4: R1 is already defined on line 3"
            )],
        ),
        ("## Functional requirements\n\n- R1. No priority.\n", vec![]),
    ];
    for (spec, want) in cases {
        let t = check_root();
        if !spec.is_empty() {
            t.write(SPEC_PATH, spec);
        }
        assert_eq!(check(&t, "spec", "approved"), want, "{spec:?}");
    }
    let t = check_root();
    std::fs::create_dir_all(t.p(SPEC_PATH)).unwrap();
    let got = check(&t, "spec", "approved");
    assert_eq!(got.len(), 1);
    assert!(
        got[0].starts_with(&format!("{SPEC_PATH}: can't check: ")),
        "{got:?}"
    );
}

#[test]
fn check_architecture() {
    let a = VALID_ARCHITECTURE;
    let cases: Vec<(String, Vec<String>)> = vec![
        (String::new(), vec![format!("{ARCH_PATH}: doesn't exist")]),
        (a.into(), vec![]),
        (
            "# Architecture — Probe\n\n## Overview\n\nText.\n".into(),
            vec![format!("{ARCH_PATH}: no ## Diagrams section")],
        ),
        (
            a.replacen("### Flow: approving", "### Notes on flow", 1),
            vec![format!("{ARCH_PATH}: ## Diagrams has no ### Flow: <name>")],
        ),
        (
            a.replacen("### Flow: approving", "### Flow:  ", 1),
            vec![format!("{ARCH_PATH}: ## Diagrams has no ### Flow: <name>")],
        ),
        (
            "## Diagrams\n\n### Flow: x\n```mermaid\na\n```\n".into(),
            vec![
                format!("{ARCH_PATH}: ## Diagrams has no ### Context"),
                format!("{ARCH_PATH}: ## Diagrams has no ### Layers"),
            ],
        ),
        (
            a.replacen("sequenceDiagram\n  A->>B: go\n", "", 1),
            vec![format!(
                "{ARCH_PATH}:23: ### Flow: approving needs a non-empty mermaid block"
            )],
        ),
        (
            a.replacen("```mermaid\nflowchart TB\n  c --> d\n```", "Prose only.", 1),
            vec![format!(
                "{ARCH_PATH}:16: ### Layers needs a non-empty mermaid block"
            )],
        ),
        (
            a.replacen("## Notes", "### Deployment\n\nLater.\n\n## Notes", 1),
            vec![],
        ),
        (
            a.replacen("The shape.", "Talks to <component>.", 1),
            vec![format!(
                "{ARCH_PATH}:5: unfilled template placeholder <component>"
            )],
        ),
        (
            "# Architecture — <short name>\n\nText.\n".into(),
            vec![
                format!("{ARCH_PATH}: no ## Diagrams section"),
                format!("{ARCH_PATH}:1: unfilled template placeholder <short name>"),
            ],
        ),
    ];
    for (arch, want) in cases {
        let t = check_root();
        if !arch.is_empty() {
            t.write(ARCH_PATH, &arch);
        }
        assert_eq!(check(&t, "architecture", "approved"), want, "{arch:?}");
    }
}

#[test]
fn check_breakdown() {
    let mine = |n: &str| format!("{TICKETS}/{n}.md");
    let other = |n: &str| format!("plan/specs/2-other/tickets/{n}.md");
    let ghost = format!("{TICKETS}/S-0001.md: S-0001: repo \"ghost\" isn't registered");
    let s = |id: &str, ch: &str, repo: &str, st: &str, x: &str| story(id, ch, repo, st, x);
    type Files = Vec<(String, String)>;
    let cases: Vec<(Files, Vec<String>)> = vec![
        (vec![], vec![format!("{TICKETS}: no tickets")]),
        (
            vec![(mine("S-0001"), s("S-0001", "1-probe", "app", "draft", ""))],
            vec![],
        ),
        (
            vec![(mine("S-0001"), s("S-0001", "1-probe", "ghost", "draft", ""))],
            vec![ghost.clone()],
        ),
        (
            vec![
                (mine("S-0001"), s("S-0001", "1-probe", "app", "draft", "")),
                (
                    other("S-0005"),
                    s("S-0005", "2-other", "ghost", "draft", ""),
                ),
            ],
            vec![],
        ),
        (
            vec![
                (
                    mine("S-0001"),
                    s(
                        "S-0001",
                        "1-probe",
                        "app",
                        "draft",
                        "depends_on: [S-0005]\n",
                    ),
                ),
                (other("S-0005"), s("S-0005", "2-other", "app", "draft", "")),
            ],
            vec![],
        ),
        (
            vec![
                (mine("S-0001"), s("S-0001", "1-probe", "app", "draft", "")),
                (other("S-0005"), "---\nschema: [\n---\n".into()),
            ],
            vec![],
        ),
        (
            vec![(mine("S-0001"), s("S-0001", "1-probe", "app", "ready", ""))],
            vec![],
        ),
        (
            vec![(mine("S-0001"), s("S-0001", "1-probe", "ghost", "ready", ""))],
            vec![ghost],
        ),
        (
            vec![
                (
                    mine("S-0001"),
                    s(
                        "S-0001",
                        "1-probe",
                        "app",
                        "draft",
                        "depends_on: [S-0005]\n",
                    ),
                ),
                (other("S-0005"), s("S-0005", "2-other", "app", "ready", "")),
            ],
            vec![],
        ),
        (
            vec![(
                mine("S-0001"),
                s(
                    "S-0001",
                    "1-probe",
                    "app",
                    "draft",
                    "depends_on: [S-0009]\n",
                ),
            )],
            vec![format!(
                "{TICKETS}/S-0001.md: S-0001: depends_on S-0009 doesn't exist"
            )],
        ),
    ];
    for (files, want) in cases {
        let t = check_root();
        for (p, text) in &files {
            t.write(p, text);
        }
        assert_eq!(check(&t, "breakdown", "approved"), want, "{files:?}");
    }
    // An unparsable ticket of this change is a problem at its path.
    let t = check_root();
    t.write(&mine("S-0002"), "---\nschema: [\n---\n");
    let got = check_artifacts(t.path(), "1-probe", "breakdown", "approved", "");
    assert_eq!(got.len(), 1);
    assert_eq!(
        (got[0].path.as_str(), got[0].line),
        (mine("S-0002").as_str(), 0)
    );
    // A ticket without an id is reported at its path, not the root's.
    let t = check_root();
    let no_id = story("S-0001", "1-probe", "app", "draft", "").replacen("id: S-0001\n", "", 1);
    t.write(&mine("S-0001"), &no_id);
    let got = check_artifacts(t.path(), "1-probe", "breakdown", "approved", "");
    assert!(!got.is_empty());
    for p in got {
        assert_eq!(p.path, mine("S-0001"));
        assert!(!p.message.contains(&*t.path().to_string_lossy()), "{p}");
    }
    // A broken REPOS.json can't be checked.
    let t = check_root();
    t.write("REPOS.json", "{");
    t.write(
        &mine("S-0001"),
        &story("S-0001", "1-probe", "app", "draft", ""),
    );
    let got = check(&t, "breakdown", "approved");
    assert_eq!(got.len(), 1);
    assert!(
        got[0].starts_with("can't check: daiku: cannot parse "),
        "{got:?}"
    );
}

#[test]
fn check_breakdown_citations() {
    let t = check_root();
    t.write(
        SPEC_PATH,
        "## Functional requirements\n\n- R1. A. — **must**\n- R2. B. — **must**\n- R2. Again. — **must**\n",
    );
    t.write(
        &format!("{TICKETS}/S-0001.md"),
        &story("S-0001", "1-probe", "app", "ready", "requirements: [R99]\n"),
    );
    t.write(
        "plan/specs/2-other/tickets/S-0005.md",
        &story("S-0005", "2-other", "app", "draft", "requirements: [R7]\n"),
    );
    assert_eq!(
        check(&t, "breakdown", "approved"),
        [
            format!("{SPEC_PATH}: R2: spec of 1-probe: line 5: R2 is already defined on line 4"),
            format!("{TICKETS}/S-0001.md: S-0001: cites R99, which isn't in 1-probe's spec"),
        ]
    );
    t.write(
        SPEC_PATH,
        "## Functional requirements\n\n- R1. A. — **must**\n- R2. B. — **must**\n",
    );
    t.write(
        &format!("{TICKETS}/S-0001.md"),
        &story("S-0001", "1-probe", "app", "draft", ""),
    );
    assert!(check(&t, "breakdown", "approved").is_empty());
}

#[test]
fn check_publish() {
    let t = check_root();
    let w = |id: &str, ch: &str, st: &str, x: &str| {
        t.write(
            &format!("plan/specs/{ch}/tickets/{id}.md"),
            &story(id, ch, "app", st, x),
        )
    };
    w("S-0001", "1-probe", "ready", "");
    w(
        "S-0002",
        "1-probe",
        "published",
        "published_hash: \"v2:abc\"\n",
    );
    w("S-0003", "1-probe", "draft", "");
    w("S-0004", "1-probe", "published", "");
    w("S-0005", "2-other", "ready", "");
    assert_eq!(
        check(&t, "publish", "complete"),
        [
            format!("{TICKETS}/S-0001.md: S-0001: ready but not frozen (no published_hash)"),
            format!("{TICKETS}/S-0004.md: S-0004: published but not frozen (no published_hash)"),
        ]
    );
}

type Reqs = (
    Vec<(String, String, usize)>,
    Vec<(Severity, String, String)>,
);

fn reqs(body: &str) -> Reqs {
    let head = "# Spec — X\n\n## Functional requirements\n\n";
    let (r, f) = parse_spec(&format!("{head}{body}"));
    (
        r.into_iter().map(|r| (r.id, r.priority, r.line)).collect(),
        f.into_iter()
            .map(|f| (f.severity, f.id, f.message))
            .collect(),
    )
}

fn req(id: &str, priority: &str, line: usize) -> (String, String, usize) {
    (id.into(), priority.into(), line)
}

fn no_priority(id: &str, n: usize) -> (Severity, String, String) {
    (
        Severity::Warning,
        id.to_string(),
        format!(
            "line {n}: {id} has no priority (expected — **must**, — **should** or — **could**)"
        ),
    )
}

#[test]
fn parse_spec_cases() {
    let none: Reqs = (vec![], vec![]);
    assert_eq!(
        reqs("- R1. <capability, verifiable> — **must | should | could**\n"),
        none
    );
    assert_eq!(reqs("- R1. … — **must**\n"), none);
    assert_eq!(
        reqs("- R4. A thing with no priority\n"),
        (vec![req("R4", "", 5)], vec![no_priority("R4", 5)])
    );
    assert_eq!(
        reqs("- R1. A thing. —**must**\n- R2. A thing. - **must**\n"),
        (
            vec![req("R1", "", 5), req("R2", "", 6)],
            vec![no_priority("R1", 5), no_priority("R2", 6)]
        )
    );
    assert_eq!(
        reqs("- R1. A thing. — **could** (later — **must**)\n"),
        (vec![req("R1", "could", 5)], vec![])
    );
    let dup = |n: usize| {
        (
            Severity::Error,
            "R2".to_string(),
            format!("line {n}: R2 is already defined on line 5"),
        )
    };
    assert_eq!(
        reqs("- R2. One. — **must**\n- R2. Two. — **must**\n- R2. Three. — **must**\n"),
        (vec![req("R2", "must", 5)], vec![dup(6), dup(7)])
    );
    assert_eq!(
        reqs(
            "```\n- R1. Fenced. — **must**\n```\n~~~md\n- R2. Fenced. — **must**\n~~~\n- R3. Real. — **must**\n"
        ),
        (vec![req("R3", "must", 11)], vec![])
    );
    assert_eq!(
        reqs("- R1. Real. — **must**\n```\n- R2. Fenced. — **must**\n"),
        (vec![req("R1", "must", 5)], vec![])
    );
    assert!(parse_spec("- R1. Outside. — **must**\n").0.is_empty());
}

#[test]
fn diagrams_cases() {
    let (section, ds) = diagrams(VALID_ARCHITECTURE);
    assert!(section);
    let got: Vec<_> = ds
        .iter()
        .map(|d| (d.heading.as_str(), d.line, d.source.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("Context", 9, "flowchart LR\n  a --> b"),
            ("Layers", 16, "flowchart TB\n  c --> d"),
            ("Flow: approving", 23, "sequenceDiagram\n  A->>B: go"),
        ]
    );
    let (section, ds) = diagrams(
        "## Diagrams\n\n### A\n```text\nx\n```\n```mermaid\ny\n```\n### B\n```mermaid\n```\n",
    );
    assert!(section);
    assert_eq!((ds[0].source.as_str(), ds[1].source.as_str()), ("y", ""));
    assert_eq!(diagrams("```\n## Diagrams\n```\n"), (false, vec![]));
}

#[test]
fn model_and_problems() {
    let t = check_root();
    t.write(
        SPEC_PATH,
        "# Functional spec — Probe flow\n\n## Functional requirements\n\n- R1. A. — **must**\n- R2. B. — **must**\n",
    );
    t.write(
        &format!("{TICKETS}/S-0002.md"),
        &story(
            "S-0002",
            "1-probe",
            "app",
            "ready",
            "requirements: [R1]\nadrs: [ADR-0001, ADR-0009, adr-1]\n",
        ),
    );
    t.write(
        &format!("{TICKETS}/S-0001.md"),
        &story("S-0001", "1-probe", "app", "ready", ""),
    );
    t.write(
        "plan/04_architecture/adr/0001-first.md",
        "# 0001 — First\n\n- Status: accepted\n",
    );
    t.write("plan/specs/2-other/README.md", "x");
    t.write("plan/specs/notes/README.md", "x");
    let m = load(t.path(), "1-probe", "").unwrap();
    assert_eq!(m.title, "Probe flow");
    assert!(m.has_spec && !m.has_architecture);
    let ids: Vec<_> = m.tickets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["S-0001", "S-0002"]);
    let names: Vec<_> = m.changes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["1-probe", "2-other"]);
    assert_eq!(m.adrs["ADR-0001"].title, "First");
    let got: Vec<String> = problems(&m)
        .iter()
        .map(|f| format!("{} {} {}: {}", f.change, f.severity, f.id, f.message))
        .collect();
    assert_eq!(
        got,
        [
            "1-probe warning S-0001: ready story with no requirements",
            "1-probe warning R2: must requirement in 1-probe isn't cited by any ticket",
            "1-probe warning S-0002: cites ADR-0009, but there's no 0009-*.md ADR file",
            "1-probe warning S-0002: adrs entry \"adr-1\" isn't of the form ADR-<nnnn>",
        ]
    );
    assert_eq!(load_all(t.path(), "").unwrap().change, "");
    assert_eq!(title("```\n# Not\n```\n# Spec — The title \n"), "The title");
    assert_eq!(title("# Plain\n"), "Plain");
    assert_eq!(title("no heading"), "");
}

#[test]
fn duplicate_adr_numbers_keep_the_smaller_path() {
    let t = check_root();
    t.write("plan/04_architecture/adr/0001-zeta.md", "# 0001 — Zeta\n");
    t.write("plan/04_architecture/adr/0001-alpha.md", "# 0001 — Alpha\n");
    let m = load_all(t.path(), "").unwrap();
    assert_eq!(m.adrs["ADR-0001"].title, "Alpha");
}
