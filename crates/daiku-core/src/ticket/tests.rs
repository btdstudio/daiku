//! Ticket tests: Go's `ticket_test.go` and `pulled_test.go` read-side cases, by
//! behaviour. Tickets are built from text, since writing them arrives in S-0007.

use super::*;

const EXAMPLE: &str = "---
schema: 1
id: S-0014                 # type prefix + workspace-global sequence: E, F, S, T
type: story                # epic | feature | story | task
title: Add release workflow to shared actions repo
parent: F-0003             # optional
repo: platform-actions     # logical name from REPOS.json; required for story/task
depends_on: [S-0011, S-0012]
supersedes: S-0009         # optional; a published ticket this one replaces
change: 3-release-automation
spec: specs/3-release-automation/functional-spec.md
adrs: [ADR-0004]
labels: [ci, release]
estimate: M                # S | M | L, optional
status: draft              # draft | ready | published
origin:                    # set by tomo intake only
  workspace: git@github.com:org/platform-planning.git
  path: specs/3-release-automation/tickets/S-0014.md
---

## Context
Why this exists; links to spec sections and ADRs.

## Acceptance criteria
- [ ] AC1: Given a repo When a tag lands Then a release is cut

## Notes
Constraints, risks, out of scope.
";

const READY_BODY: &str = "\n## Acceptance criteria\n- [ ] AC1: Given x When y Then z\n";

/// A ticket from frontmatter lines and a body, with Go's `mk` defaults (a story
/// titled "A ticket" in repo `app`, draft). A ready ticket gets a valid mark.
fn mk(fields: &[(&str, &str)], body: &str) -> Ticket {
    let get = |k: &str| fields.iter().find(|(f, _)| *f == k).map(|(_, v)| *v);
    let typ = get("type").unwrap_or(STORY);
    let mut text = String::from("---\nschema: 1\n");
    text.push_str(&format!(
        "type: {typ}\ntitle: {}\n",
        get("title").unwrap_or("A ticket")
    ));
    if get("repo").is_none() && (typ == STORY || typ == TASK) {
        text.push_str("repo: app\n");
    }
    let status = get("status").unwrap_or(STATUS_DRAFT);
    text.push_str(&format!("status: {status}\n"));
    for (k, v) in fields {
        if !matches!(*k, "type" | "title" | "status") {
            text.push_str(&format!("{k}: {v}\n"));
        }
    }
    let close = format!("---\n{body}");
    let t = Ticket::parse(format!("{text}{close}").as_bytes()).unwrap();
    if status != STATUS_READY {
        return t;
    }
    let mark = content_hash(&t);
    Ticket::parse(format!("{text}ready_hash: {mark}\n{close}").as_bytes()).unwrap()
}

fn messages(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn refs(tickets: &[Ticket]) -> Vec<&Ticket> {
    tickets.iter().collect()
}

#[test]
fn parses_the_design_example() {
    let t = Ticket::parse(EXAMPLE.as_bytes()).unwrap();
    assert_eq!(
        (
            t.schema,
            t.id.as_str(),
            t.typ.as_str(),
            t.repo.as_str(),
            t.estimate.as_str()
        ),
        (1, "S-0014", STORY, "platform-actions", "M")
    );
    assert_eq!(t.depends_on, ["S-0011", "S-0012"]);
    assert_eq!(
        (t.supersedes.as_str(), t.adrs.as_slice()),
        ("S-0009", &["ADR-0004".to_string()][..])
    );
    assert_eq!(
        t.origin.workspace,
        "git@github.com:org/platform-planning.git"
    );
    assert_eq!(messages(&validate_file(&t)), "");
}

#[test]
fn parse_rejects_malformed_files() {
    for text in [
        "no frontmatter",
        "---\nschema: x\n---\n",
        "---\nid: &x\n---\n",
    ] {
        assert!(Ticket::parse(text.as_bytes()).is_err(), "{text:?}");
    }
    assert_eq!(
        Ticket::parse(b"---\nschema: x\n---\n")
            .unwrap_err()
            .to_string(),
        "ticket: invalid schema \"x\""
    );
}

#[test]
fn validate_file_cases() {
    type Fields<'a> = &'a [(&'a str, &'a str)];
    let cases: &[(Fields, &str, &str)] = &[
        (&[("id", "S-0001")], "", ""),
        (&[("id", "E-0001"), ("type", EPIC)], "", ""),
        (
            &[("id", "S-1")],
            "",
            "S-1: id \"S-1\" must look like S-0001 or X-1234",
        ),
        (
            &[("id", "F-0001")],
            "",
            "F-0001: id prefix F doesn't match type story (expected S)",
        ),
        (
            &[("id", "S-0001"), ("type", "bug")],
            "",
            "S-0001: unknown type \"bug\"",
        ),
        (
            &[("id", "S-0001"), ("status", "doing")],
            "",
            "S-0001: unknown status \"doing\"",
        ),
        (
            &[("id", "S-0001"), ("estimate", "XL")],
            "",
            "S-0001: estimate must be S, M or L, got \"XL\"",
        ),
        (
            &[("id", "S-0001"), ("status", STATUS_READY)],
            "\n## Context\n- [ ] not here\n",
            "S-0001: status ready requires at least one acceptance criterion",
        ),
        (
            &[("id", "S-0001"), ("status", STATUS_READY)],
            READY_BODY,
            "",
        ),
        (
            &[("id", "S-0001"), ("status", STATUS_PUBLISHED)],
            "",
            "S-0001: status published requires an external id",
        ),
        (
            &[("id", "S-0001"), ("status", STATUS_CLOSED)],
            "",
            "S-0001: only a native N- ticket can be closed; the PM tool owns a published ticket's status",
        ),
        (&[("id", "N-0001"), ("type", TASK)], "", ""),
        (
            &[("id", "N-0001"), ("type", TASK), ("status", STATUS_CLOSED)],
            "",
            "",
        ),
        (
            &[("id", "N-0001")],
            "",
            "N-0001: a native N- ticket must be a task",
        ),
        (
            &[("id", "X-AB-12"), ("type", TASK)],
            "",
            "X-AB-12: a pulled X- ticket needs exactly one external id, got 0",
        ),
    ];
    for (fields, body, want) in cases {
        assert_eq!(
            messages(&validate_file(&mk(fields, body))),
            *want,
            "{fields:?}"
        );
    }
}

#[test]
fn validate_file_required_fields() {
    let t = Ticket::parse(b"---\nid: S-0001\ntype: story\nstatus: draft\n---\n").unwrap();
    assert_eq!(
        messages(&validate_file(&t)),
        "S-0001: schema must be 1, got 0\nS-0001: title is required\nS-0001: repo is required for a story"
    );
    let native = Ticket::parse(
        b"---\nschema: 1\nid: N-0001\ntype: task\ntitle: x\nrepo: a\nstatus: draft\norigin:\n  path: x\n---\n",
    )
    .unwrap();
    assert_eq!(
        messages(&validate_file(&native)),
        "N-0001: a native N- ticket can't have an origin"
    );
    let hash_only = Ticket::parse(
        b"---\nschema: 1\nid: S-0001\ntype: story\ntitle: x\nrepo: a\nstatus: draft\npublished_hash: abc\n---\n",
    )
    .unwrap();
    assert_eq!(
        messages(&validate_file(&hash_only)),
        "S-0001: published_hash requires an external id\nS-0001: drifted: body changed since it was published"
    );
    let mut misnamed = mk(&[("id", "S-0001")], "");
    misnamed.path = "plan/specs/1-x/tickets/S-0002.md".into();
    assert_eq!(
        messages(&validate_file(&misnamed)),
        "S-0001: file name S-0002.md doesn't match id"
    );
}

#[test]
fn readiness_is_validated() {
    let unmarked = Ticket::parse(
        format!("---\nschema: 1\nid: S-0001\ntype: story\ntitle: x\nrepo: a\nstatus: ready\n---\n{READY_BODY}")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        messages(&validate_file(&unmarked)),
        "S-0001: not readied through the CLI"
    );
    let marked = mk(&[("id", "S-0001"), ("status", STATUS_READY)], READY_BODY);
    let text = format!(
        "---\nschema: 1\ntype: story\ntitle: Changed\nrepo: app\nstatus: ready\nid: S-0001\nready_hash: {}\n---\n{READY_BODY}x\n",
        marked.ready_hash
    );
    let edited = Ticket::parse(text.as_bytes()).unwrap();
    assert_eq!(
        messages(&validate_file(&edited)),
        "S-0001: body and frontmatter changed since readied"
    );
    let bad = Ticket::parse(
        format!("---\nschema: 1\nid: S-0001\ntype: story\ntitle: x\nrepo: a\nstatus: ready\nready_hash: v2:x\n---\n{READY_BODY}")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        messages(&validate_file(&bad)),
        "S-0001: ready_hash \"v2:x\" isn't a valid v2 hash"
    );
}

#[test]
fn placeholder_criteria_are_flagged() {
    let body = "\n## Acceptance criteria\n- [ ] AC1: Given <state> When y Then z\n\n## Notes\n- [ ] not a criterion\n";
    let t = mk(&[("id", "S-0001"), ("status", STATUS_READY)], body);
    assert_eq!(
        messages(&validate_file(&t)),
        "S-0001: status ready requires at least one acceptance criterion without a placeholder\n\
         S-0001: acceptance criterion \"- [ ] AC1: Given <state> When y Then z\" has a placeholder \"<state>\""
    );
    assert!(!has_acceptance_criteria(body));
    assert!(has_acceptance_criteria(READY_BODY));
    assert!(!has_acceptance_criteria(
        "\n## Acceptance criteria\n- [ ] \n"
    ));
}

#[test]
fn validate_workspace_cases() {
    let good = vec![
        mk(&[("id", "E-0001"), ("type", EPIC)], ""),
        mk(
            &[("id", "F-0002"), ("type", FEATURE), ("parent", "E-0001")],
            "",
        ),
        mk(&[("id", "S-0003"), ("parent", "F-0002")], ""),
        mk(
            &[
                ("id", "S-0004"),
                ("parent", "F-0002"),
                ("depends_on", "[S-0003]"),
            ],
            "",
        ),
        mk(
            &[("id", "T-0005"), ("type", TASK), ("parent", "S-0004")],
            "",
        ),
        mk(&[("id", "S-0006"), ("supersedes", "S-0003")], ""),
    ];
    let app = ["app".to_string()];
    assert_eq!(messages(&validate_workspace(&refs(&good), &app)), "");
    let cases: Vec<(Vec<Ticket>, &str)> = vec![
        (
            vec![mk(&[("id", "S-0001")], ""), mk(&[("id", "S-0001")], "")],
            "S-0001: duplicate id",
        ),
        (
            vec![mk(&[("id", "S-0001"), ("parent", "F-0009")], "")],
            "S-0001: parent F-0009 doesn't exist",
        ),
        (
            vec![
                mk(&[("id", "S-0001")], ""),
                mk(&[("id", "S-0002"), ("parent", "S-0001")], ""),
            ],
            "S-0002: parent S-0001 is a story, which isn't higher than a story",
        ),
        (
            vec![mk(&[("id", "S-0001"), ("depends_on", "[S-0009]")], "")],
            "S-0001: depends_on S-0009 doesn't exist",
        ),
        (
            vec![mk(&[("id", "S-0001"), ("supersedes", "S-0009")], "")],
            "S-0001: supersedes S-0009 doesn't exist",
        ),
        (
            vec![mk(&[("id", "S-0001"), ("repo", "other")], "")],
            "S-0001: repo \"other\" isn't registered",
        ),
        (
            vec![mk(&[("id", "S-0001"), ("depends_on", "[S-0001]")], "")],
            "tickets: dependency cycle among S-0001",
        ),
        (
            vec![
                mk(&[("id", "S-0001"), ("depends_on", "[S-0003]")], ""),
                mk(&[("id", "S-0002"), ("depends_on", "[S-0001]")], ""),
                mk(&[("id", "S-0003"), ("depends_on", "[S-0002]")], ""),
            ],
            "tickets: dependency cycle among S-0001, S-0002, S-0003",
        ),
    ];
    for (tickets, want) in cases {
        assert_eq!(messages(&validate_workspace(&refs(&tickets), &app)), want);
    }
    let missing = validate_workspace(
        &refs(&[mk(&[("id", "S-0001"), ("depends_on", "[S-0009]")], "")]),
        &app,
    );
    assert_eq!(missing[0].reference, "S-0009");
}

#[test]
fn order_puts_references_first() {
    let tickets = vec![
        mk(
            &[
                ("id", "S-0005"),
                ("parent", "F-0002"),
                ("depends_on", "[S-0006]"),
            ],
            "",
        ),
        mk(&[("id", "S-0006"), ("parent", "F-0002")], ""),
        mk(
            &[("id", "F-0002"), ("type", FEATURE), ("parent", "E-0001")],
            "",
        ),
        mk(&[("id", "E-0001"), ("type", EPIC)], ""),
        mk(
            &[
                ("id", "S-0007"),
                ("supersedes", "S-0005"),
                ("depends_on", "[S-0099]"),
            ],
            "",
        ),
    ];
    let r = refs(&tickets);
    let ids: Vec<&str> = order_tickets(&r)
        .unwrap()
        .iter()
        .map(|&i| r[i].id.as_str())
        .collect();
    assert_eq!(ids, ["E-0001", "F-0002", "S-0006", "S-0005", "S-0007"]);
}

#[test]
fn hashes_match_go() {
    assert_eq!(hash("a\nb\n"), hash("a\r\nb\n\n\n"));
    assert_eq!(
        hash("a\r\nb"),
        "911169ddaaf146aff539f58c26c489af3b892dff0fe283c1c264c65ae5aa59a2"
    );
    let text = "---\nschema: 1\nid: S-0042\ntype: story\ntitle: \"Say \\\"hi\\\" <now> & then\"\nrepo: kiso\n\
                depends_on: [S-0002, S-0001]\nlabels: []\nzeta: unknown\nmeta:\n  b: 2\n  a: 1\nstatus: published\n\
                external:\n  ado: 7\norigin:\n  workspace: w\n  path: p\nready_hash: v2:x\npublished_hash: \"v2:y\"\n---\nbody text\n";
    let t = Ticket::parse(text.as_bytes()).unwrap();
    assert_eq!(
        canonical_frontmatter(&t.doc),
        "depends_on=[\"S-0001\",\"S-0002\"]\nid=\"S-0042\"\nmeta={\"a\":\"1\",\"b\":\"2\"}\nrepo=\"kiso\"\nschema=\"1\"\n\
         title=\"Say \\\"hi\\\" \\u003cnow\\u003e \\u0026 then\"\ntype=\"story\"\nzeta=\"unknown\"\n"
    );
    assert_eq!(
        content_hash(&t),
        "v2:044bbaad51900e452b203079800e84aa.14f77507924d080a4363e45918b15b41"
    );
    let keys = Ticket::parse(b"---\na: 1\na-b: 2\na1: 3\n---\n").unwrap();
    assert_eq!(
        canonical_frontmatter(&keys.doc),
        "a=\"1\"\na-b=\"2\"\na1=\"3\"\n"
    );
}

#[test]
fn freezes_drift() {
    let base = "---\nschema: 1\nid: S-0001\ntype: story\ntitle: x\nrepo: a\nstatus: published\nexternal:\n  ado: 1\n";
    let t = Ticket::parse(format!("{base}---\nbody\n").as_bytes()).unwrap();
    let frozen = format!("{base}published_hash: {}\n", content_hash(&t));
    let same = Ticket::parse(format!("{frozen}---\nbody\n\n").as_bytes()).unwrap();
    assert!(same.frozen() && !same.drifted());
    let body = Ticket::parse(format!("{frozen}---\nchanged\n").as_bytes()).unwrap();
    assert_eq!(body.drift(), (true, false));
    assert_eq!(
        messages(&validate_file(&body)),
        "S-0001: drifted: body changed since it was published"
    );
    let fm =
        Ticket::parse(format!("{}---\nbody\n", frozen.replace("title: x", "title: y")).as_bytes())
            .unwrap();
    assert_eq!(fm.drift(), (false, true));
    let legacy =
        Ticket::parse(format!("{base}published_hash: {}\n---\nbody\n", hash("body")).as_bytes())
            .unwrap();
    assert!(!legacy.drifted());
    let malformed =
        Ticket::parse(format!("{base}published_hash: v2:nope\n---\nbody\n").as_bytes()).unwrap();
    assert_eq!(malformed.drift(), (true, true));
    assert_eq!(
        messages(&validate_file(&malformed)),
        "S-0001: published_hash \"v2:nope\" isn't a valid v2 hash"
    );
}

#[test]
fn invalid_utf8_frontmatter_is_reported() {
    let head = b"---\nschema: 1\nid: S-0001\ntype: story\nrepo: a\nstatus: draft\n".to_vec();
    for (fm, key) in [
        (&b"title: x\xffy\n"[..], "title"),
        (&b"title: x\nlabels: [ok, b\xfe]\n"[..], "labels"),
        (&b"title: x\nmeta:\n  k: v\xff\n"[..], "meta"),
    ] {
        let mut data = head.clone();
        data.extend_from_slice(fm);
        data.extend_from_slice(b"---\nbody\n");
        let t = Ticket::parse(&data).unwrap();
        assert_eq!(
            messages(&validate_file(&t)),
            format!("S-0001: frontmatter field {key} isn't valid UTF-8")
        );
    }
    let ok = Ticket::parse("---\nschema: 1\nid: S-0001\ntype: story\nrepo: a\nstatus: draft\ntitle: café — naïve\nlabels: [日本]\n---\nbody\n".as_bytes()).unwrap();
    assert_eq!(messages(&validate_file(&ok)), "");
}

#[test]
fn load_dir_reads_tickets_and_reports_bad_files() {
    let t = crate::testutil::Temp::new();
    t.write(
        "tickets/S-0002.md",
        "---\nschema: 1\nid: S-0002\ntype: story\ntitle: b\nrepo: a\nstatus: draft\n---\n",
    );
    t.write(
        "tickets/S-0001.md",
        "---\nschema: 1\nid: S-0001\ntype: story\ntitle: a\nrepo: a\nstatus: draft\n---\n",
    );
    t.write("tickets/broken.md", "no frontmatter");
    t.write("tickets/notes.txt", "x");
    std::fs::create_dir_all(t.p("tickets/sub.md")).unwrap();
    let (tickets, problems) = load_dir(&t.p("tickets"), "plan/tickets").unwrap();
    let ids: Vec<_> = tickets
        .iter()
        .map(|t| (t.id.as_str(), t.path.as_str()))
        .collect();
    assert_eq!(
        ids,
        [
            ("S-0001", "plan/tickets/S-0001.md"),
            ("S-0002", "plan/tickets/S-0002.md")
        ]
    );
    assert_eq!(
        messages(&problems),
        "plan/tickets/broken.md: frontmatter: file must start with \"---\""
    );
    let (none, _) = load_dir(&t.p("missing"), "missing").unwrap();
    assert!(none.is_empty());
}

#[test]
fn id_patterns_match_go() {
    for id in ["S-0001", "N-0042", "X-AB-12", "X-1"] {
        assert!(ID_RE.is_match(id), "{id}");
    }
    for id in [
        "S-1",
        "Q-0001",
        "X-",
        "X--1",
        "X-a-",
        "S-00001",
        "S-٠٠٠١",
        "../x",
    ] {
        assert!(!ID_RE.is_match(id), "{id}");
    }
}

// ---- The write side (S-0007): Go's write-side tests, and the byte rules. ----

fn new_ticket(f: NewTicket) -> Ticket {
    let mut f = f;
    if f.title.is_empty() {
        f.title = "A ticket".into();
    }
    if f.typ.is_empty() {
        f.typ = STORY.into();
    }
    if f.repo.is_empty() && (f.typ == STORY || f.typ == TASK) {
        f.repo = "app".into();
    }
    let ready = f.status == STATUS_READY;
    let mut t = Ticket::new(f).unwrap();
    if ready {
        t.mark_ready().unwrap();
    }
    t
}

fn reparse(t: &Ticket) -> Ticket {
    Ticket::parse(&t.to_bytes()).unwrap()
}

fn draft_text(criteria: &str) -> String {
    format!(
        "---\nschema: 1\nid: S-0001\ntype: story\ntitle: A ticket\nrepo: a\nlabels: [x]\nstatus: draft\n---\n\
         \n## Context\nWhy.\n\n## Acceptance criteria\n{criteria}\n## Notes\nNone.\n"
    )
}

const FREEZE_FIXTURE: &str = "---\nschema: 1\nid: S-0001\ntype: story\ntitle: A ticket\nparent: F-0001\nrepo: a\n\
    depends_on: [S-0002, S-0003]\nlabels: [x, y]\nx: keep\nmeta:\n  k: v\nstatus: ready\n---\n\
    \n## Acceptance criteria\n- [ ] AC1: Given x When y Then z\n";

#[test]
fn new_round_trips() {
    let t = new_ticket(NewTicket {
        id: "S-0001".into(),
        title: "Fix: a thing".into(),
        parent: "F-0002".into(),
        depends_on: vec!["S-0003".into()],
        labels: vec!["ci".into()],
        estimate: "S".into(),
        body: READY_BODY.into(),
        ..NewTicket::default()
    });
    let again = reparse(&t);
    assert_eq!(again.title, "Fix: a thing");
    assert_eq!(again.status, STATUS_DRAFT);
    assert_eq!(again.schema, SCHEMA);
    assert_eq!(again.parent, "F-0002");
    assert_eq!(again.body, READY_BODY);
    assert!(
        Ticket::new(NewTicket {
            id: "S-0001".into(),
            title: "a\nstatus: published".into(),
            ..NewTicket::default()
        })
        .is_err(),
        "a newline in a title must not inject a field"
    );
}

#[test]
fn new_writes_requirements_after_adrs() {
    let base = NewTicket {
        id: "S-0001".into(),
        adrs: vec!["ADR-0001".into()],
        labels: vec!["ci".into()],
        ..NewTicket::default()
    };
    let with = NewTicket {
        requirements: vec!["R3".into(), "N1".into()],
        ..base.clone()
    };
    let text = String::from_utf8(new_ticket(with).to_bytes()).unwrap();
    let adrs = text.find("\nadrs: [ADR-0001]\n").unwrap();
    let reqs = text.find("\nrequirements: [R3, N1]\n").unwrap();
    let labels = text.find("\nlabels: [ci]\n").unwrap();
    assert!(adrs < reqs && reqs < labels, "{text}");
    let want = new_ticket(base.clone()).to_bytes();
    let got = new_ticket(NewTicket {
        requirements: Vec::new(),
        ..base
    })
    .to_bytes();
    assert_eq!(got, want);
    assert!(
        Ticket::new(NewTicket {
            id: "S-0001".into(),
            typ: STORY.into(),
            title: "A ticket".into(),
            repo: "app".into(),
            requirements: vec!["R1\nstatus: published".into()],
            ..NewTicket::default()
        })
        .is_err()
    );
}

#[test]
fn publish_freeze_drift_unfreeze() {
    let mut t = new_ticket(NewTicket {
        id: "S-0001".into(),
        status: STATUS_READY.into(),
        body: READY_BODY.into(),
        ..NewTicket::default()
    });
    assert!(!t.frozen());
    assert!(t.unfreeze().is_err());
    assert!(t.mark_published("", "1").is_err());
    t.mark_published("ado", "48213").unwrap();
    assert!(t.frozen());
    assert_eq!(t.status, STATUS_PUBLISHED);
    assert_eq!(t.external["ado"], "48213");
    let mut frozen = reparse(&t);
    assert_eq!(messages(&validate_file(&frozen)), "");

    frozen.mark_published("github", "org/repo#7").unwrap();
    frozen.mark_published("ado", "48213").unwrap();
    let err = frozen.mark_published("ado", "999").unwrap_err().to_string();
    assert!(err.contains("can't change"), "{err}");

    let text = String::from_utf8(frozen.to_bytes()).unwrap();
    let mut drifted = Ticket::parse(text.replacen("AC1", "AC9", 1).as_bytes()).unwrap();
    assert!(drifted.drifted());
    assert!(messages(&validate_file(&drifted)).contains("drifted: body changed since"));
    let err = drifted
        .mark_published("jira", "X-1")
        .unwrap_err()
        .to_string();
    assert!(err.contains("changed since"), "{err}");

    let mut retagged =
        Ticket::parse(text.replacen("labels: []", "labels: [late]", 1).as_bytes()).unwrap();
    assert!(retagged.drifted());
    assert!(messages(&validate_file(&retagged)).contains("drifted: frontmatter changed since"));
    let err = retagged
        .mark_published("jira", "X-1")
        .unwrap_err()
        .to_string();
    assert!(err.contains("frontmatter changed since"), "{err}");

    drifted.unfreeze().unwrap();
    let mut after = reparse(&drifted);
    assert!(!after.frozen());
    assert_eq!(after.external["ado"], "48213");
    assert_eq!(after.status, STATUS_PUBLISHED);
    assert_eq!(messages(&validate_file(&after)), "");
    after.mark_published("ado", "48213").unwrap();
    assert!(after.frozen() && !after.drifted());
}

#[test]
fn mark_published_keeps_other_fields_byte_stable() {
    let mut t = Ticket::parse(EXAMPLE.as_bytes()).unwrap();
    t.mark_published("ado", "1").unwrap();
    let got = String::from_utf8(t.to_bytes()).unwrap();
    for line in [
        "id: S-0014                 # type prefix + workspace-global sequence: E, F, S, T",
        "repo: platform-actions     # logical name from REPOS.json; required for story/task",
        "Constraints, risks, out of scope.",
    ] {
        assert!(got.contains(line), "lost {line:?}:\n{got}");
    }
}

#[test]
fn next_ids() {
    let s = new_ticket(NewTicket {
        id: "S-0014".into(),
        ..NewTicket::default()
    });
    let f = new_ticket(NewTicket {
        id: "F-0003".into(),
        typ: FEATURE.into(),
        ..NewTicket::default()
    });
    let tickets = [&s, &f];
    assert_eq!(next_id(&tickets, TASK).unwrap(), "T-0015");
    assert_eq!(next_id(&[], EPIC).unwrap(), "E-0001");
    assert_eq!(
        next_id(&[], "bug").unwrap_err().to_string(),
        "ticket: unknown type \"bug\""
    );
    assert_eq!(next_native_id(&tickets), "N-0015");
    let pulled = Ticket::parse(
        b"---\nschema: 1\nid: X-9999\ntype: story\ntitle: t\nrepo: app\nexternal:\n  jira: PROJ-9999\nstatus: draft\n---\n",
    )
    .unwrap();
    let s3 = new_ticket(NewTicket {
        id: "S-0003".into(),
        ..NewTicket::default()
    });
    assert_eq!(next_native_id(&[&s3, &pulled]), "N-0004");
}

#[test]
fn freeze_survives_round_trips() {
    let mut t = Ticket::parse(FREEZE_FIXTURE.as_bytes()).unwrap();
    t.mark_published("ado", "1").unwrap();
    let again = reparse(&t);
    assert!(!again.drifted());
    assert_eq!(content_hash(&again), t.published_hash);
    let mut built = new_ticket(NewTicket {
        id: "S-0009".into(),
        status: STATUS_READY.into(),
        body: READY_BODY.into(),
        depends_on: vec!["S-0002".into(), "S-0001".into()],
        labels: vec!["b".into(), "a".into()],
        ..NewTicket::default()
    });
    built.mark_published("ado", "9").unwrap();
    assert_eq!(messages(&validate_file(&reparse(&built))), "");
}

#[test]
fn mark_ready_is_byte_stable() {
    let draft = draft_text("- [ ] AC1: Given a repo When a tag lands Then a release is cut\n");
    let mut t = Ticket::parse(draft.as_bytes()).unwrap();
    t.mark_ready().unwrap();
    assert_eq!(t.status, STATUS_READY);
    assert_eq!(t.ready_hash, content_hash(&t));
    assert_eq!(messages(&validate_file(&t)), "");
    let got = String::from_utf8(t.to_bytes()).unwrap();
    let want = draft.replacen(
        "status: draft\n",
        &format!("status: ready\nready_hash: \"{}\"\n", t.ready_hash),
        1,
    );
    assert_eq!(got, want);
    let again = reparse(&t);
    assert_eq!(again.ready_hash, t.ready_hash);
    assert_eq!(messages(&validate_file(&again)), "");
}

#[test]
fn mark_ready_refuses_other_statuses() {
    let mut published =
        Ticket::parse(draft_text("- [ ] AC1: Given x When y Then z\n").as_bytes()).unwrap();
    published.mark_published("ado", "1").unwrap();
    let mut closed = new_ticket(NewTicket {
        id: "N-0001".into(),
        typ: TASK.into(),
        status: STATUS_CLOSED.into(),
        ..NewTicket::default()
    });
    for t in [&mut published, &mut closed] {
        let before = t.to_bytes();
        let err = t.mark_ready().unwrap_err().to_string();
        assert!(
            err.contains("only a draft or ready ticket can be readied"),
            "{err}"
        );
        assert_eq!(t.to_bytes(), before);
    }
}

#[test]
fn mark_published_removes_the_readiness_mark() {
    let mut t = Ticket::parse(draft_text("- [ ] AC1: Given x When y Then z\n").as_bytes()).unwrap();
    t.mark_ready().unwrap();
    t.mark_published("ado", "1").unwrap();
    assert!(t.ready_hash.is_empty());
    assert!(
        !String::from_utf8(t.to_bytes())
            .unwrap()
            .contains("ready_hash")
    );
    let again = reparse(&t);
    assert_eq!(messages(&validate_file(&again)), "");
    assert_eq!(again.published_hash, content_hash(&again));
}

#[test]
fn pulled_ids() {
    for (input, want) in [
        ("X-1234", "X-1234"),
        ("X-PROJ-123", "X-PROJ-123"),
        ("X-#12", "X-12"),
        ("#1234", "X-1234"),
        ("1234", "X-1234"),
        ("PROJ-123", "X-PROJ-123"),
        (" #77 ", "X-77"),
        ("a--b", "X-a-b"),
        ("-a-", "X-a"),
        ("../x", "X-x"),
        ("x-12", "X-12"),
        ("x-PROJ-1", "X-PROJ-1"),
    ] {
        let got = pulled_id(input).unwrap();
        assert_eq!(got, want, "{input:?}");
        assert!(ID_RE.is_match(&got), "{got}");
    }
    for input in ["", "#", "X-", "X-#", "--", "/"] {
        assert!(pulled_id(input).is_err(), "{input:?}");
    }
    assert_eq!(
        pulled_id("#").unwrap_err().to_string(),
        "ticket: PM id \"#\" has no letters or digits"
    );
    assert_eq!(pm_id("org/app#1234"), "1234");
    assert_eq!(pm_id("#1234"), "1234");
    assert_eq!(pm_id("PROJ-123"), "PROJ-123");
}

#[test]
fn a_written_ticket_keeps_body_bytes_that_arent_utf8() {
    let mut data = b"---\nschema: 1\nid: S-0001\ntype: story\ntitle: A ticket\nrepo: a\nstatus: draft\n---\n\n## Acceptance criteria\n- [ ] AC1: Given x\xff When y Then z\n".to_vec();
    let t = Ticket::parse(&data).unwrap();
    assert_eq!(
        t.to_bytes(),
        data,
        "unchanged, it round-trips byte for byte"
    );
    let mut t = t;
    t.mark_ready().unwrap();
    let out = t.to_bytes();
    let body_at = data.windows(4).position(|w| w == b"---\n").unwrap();
    let body_at = body_at
        + 4
        + data[body_at + 4..]
            .windows(4)
            .position(|w| w == b"---\n")
            .unwrap()
        + 4;
    assert!(out.ends_with(&data[body_at..]), "the body's bytes are kept");
    // A frontmatter byte that isn't UTF-8 makes the ticket unwritable.
    let at = data.windows(5).position(|w| w == b"story").unwrap();
    data[at + 2] = 0xfe;
    let t = Ticket::parse(&data).unwrap();
    assert_eq!(
        t.check_writable().unwrap_err().to_string(),
        "daiku: ticket S-0001: its frontmatter isn't valid UTF-8 — fix the file first"
    );
    // Unfreeze, which validates nothing, refuses rather than write U+FFFD (D2).
    let mut text = data.clone();
    let at = text
        .windows(14)
        .position(|w| w == b"status: draft\n")
        .unwrap();
    text.splice(
        at..at + 14,
        b"status: published\nexternal:\n  ado: 1\npublished_hash: v2:x\n"
            .iter()
            .copied(),
    );
    let mut t = Ticket::parse(&text).unwrap();
    assert!(matches!(t.unfreeze(), Err(Error::TicketNotUtf8 { .. })));
}

#[test]
fn unchanged_tickets_round_trip_exactly() {
    for text in [
        EXAMPLE.to_string(),
        EXAMPLE.replace('\n', "\r\n"),
        "---\nschema: 1\n# a comment\n\nid: \"S-0001\"\ntype: story\n---".to_string(),
    ] {
        let t = Ticket::parse(text.as_bytes()).unwrap();
        assert_eq!(String::from_utf8(t.to_bytes()).unwrap(), {
            let mut want = text.clone();
            if !want.ends_with('\n') {
                want.push('\n');
            }
            want
        });
    }
}

/// AC4: ready, publish and unfreeze change only the lines daiku owns.
#[test]
fn writes_touch_only_owned_lines() {
    let text = "---\r\nschema: 1\r\n# planned in the kickoff\r\nid: S-0001   # keep\r\ntype: story\r\ntitle: \"A ticket: quoted\"\r\n\r\nrepo: a\r\nlabels: [x,   y]\r\nzeta: unknown\r\nstatus: draft   # owned\r\n---\r\n\r\n## Acceptance criteria\r\n- [ ] AC1: Given x When y Then z\r\n";
    let lines = |b: &[u8]| -> Vec<String> {
        String::from_utf8(b.to_vec())
            .unwrap()
            .split("\r\n")
            .map(str::to_string)
            .collect()
    };
    let diff = |a: &[String], b: &[String]| -> Vec<String> {
        let mut out: Vec<String> = a
            .iter()
            .filter(|l| !b.contains(l))
            .map(|l| format!("-{l}"))
            .collect();
        out.extend(b.iter().filter(|l| !a.contains(l)).map(|l| format!("+{l}")));
        out.iter()
            .map(|l| l.split(':').next().unwrap().to_string())
            .collect()
    };
    let mut t = Ticket::parse(text.as_bytes()).unwrap();
    let before = lines(text.as_bytes());
    t.mark_ready().unwrap();
    let ready = lines(&t.to_bytes());
    assert_eq!(diff(&before, &ready), ["-status", "+status", "+ready_hash"]);
    t.mark_published("ado", "7").unwrap();
    let published = lines(&t.to_bytes());
    let mut d = diff(&ready, &published);
    d.sort();
    assert_eq!(
        d,
        [
            "+  ado",
            "+external",
            "+published_hash",
            "+status",
            "-ready_hash",
            "-status"
        ]
    );
    t.unfreeze().unwrap();
    let unfrozen = lines(&t.to_bytes());
    assert_eq!(diff(&published, &unfrozen), ["-published_hash"]);
}

/// AC2: the Go CLI's own readied and published files, with `<`, `>` and `&` in
/// the title and labels, carry hashes the port computes byte for byte (ADR-0006
/// amendment). Both files come from Go captures, not from this code.
#[test]
fn go_captured_hashes_match() {
    let cases = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/cases/");
    let rel = "plan/specs/1-auth-flow/tickets/S-0003.md";
    let readied = std::fs::read(format!("{cases}tickets-ready-html-title/expect/{rel}")).unwrap();
    let t = Ticket::parse(&readied).unwrap();
    assert!(
        t.title.contains('&') && t.title.contains('<'),
        "{}",
        t.title
    );
    assert_eq!(content_hash(&t), t.ready_hash);
    let published = std::fs::read(format!(
        "{cases}tickets-check-go-published-html/before/{rel}"
    ))
    .unwrap();
    let t = Ticket::parse(&published).unwrap();
    assert!(!t.published_hash.is_empty());
    assert_eq!(content_hash(&t), t.published_hash);
    assert!(!t.drifted());
}

/// A body that isn't UTF-8 hashes over its bytes, as Go's string hash does. The
/// expected mark is what the Go CLI at 5aa7a78 wrote for this file (S-0007
/// test-phase differential, `non-utf8-body-ready`).
#[test]
fn a_non_utf8_body_hashes_as_go() {
    let data = b"---\nschema: 1\nid: S-0004\ntype: story\ntitle: Bytes\nrepo: api\nchange: 1-auth-flow\nstatus: draft\n---\n\n## Acceptance criteria\n- [ ] AC1: Given caf\xe9, When y, Then z\n";
    let mut t = Ticket::parse(data).unwrap();
    t.mark_ready().unwrap();
    assert_eq!(
        t.ready_hash,
        "v2:4673d3c401f8ceafe6d9ac20303175a2.04e0cd98cffb241bab6928657c21d1bd"
    );
    // CRLF and trailing newlines still don't count, on bytes as on text.
    assert_eq!(hash_bytes(b"a\xff\r\nb\n\n"), hash_bytes(b"a\xff\nb"));
    assert_eq!(hash_bytes(b"a\r\nb"), hash("a\nb"));
    // Read back, the Go-made mark holds: no drift since readied.
    let again = Ticket::parse(&t.to_bytes()).unwrap();
    assert_eq!(messages(&validate_file(&again)), "");
}
