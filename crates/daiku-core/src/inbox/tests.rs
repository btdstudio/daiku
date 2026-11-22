//! Go's `inbox_test.go`, plus re-pulls (AC8) and body bytes (N5).

use super::*;
use crate::testutil::Temp;
use crate::ticket::{STATUS_DRAFT, STORY, validate_file};

fn opts(title: &str, repo: &str) -> NewInbox {
    NewInbox {
        title: title.into(),
        repo: repo.into(),
        ..NewInbox::default()
    }
}

#[test]
fn create_writes_a_valid_native_ticket() {
    let t = Temp::new();
    let created = create(
        t.path(),
        &NewInbox {
            title: "Build: the thing".into(),
            repo: "app".into(),
            labels: vec!["docs".into(), "ui".into()],
            body: "Wire up the store.".into(),
        },
    )
    .unwrap();
    assert_eq!(created.id, "N-0001");
    assert_eq!(created.status, STATUS_DRAFT);
    assert_eq!(created.typ, TASK);
    assert!(
        created.path.ends_with("build/tickets/N-0001.md"),
        "{}",
        created.path
    );
    let read = Ticket::parse(t.read("build/tickets/N-0001.md").as_bytes()).unwrap();
    assert_eq!(read.title, "Build: the thing");
    assert_eq!(read.labels, ["docs", "ui"]);
    assert_eq!(read.body, "\nWire up the store.\n");
    assert!(validate_file(&read).is_empty());
}

#[test]
fn ids_follow_the_highest_number() {
    let t = Temp::new();
    for _ in 0..3 {
        create(t.path(), &opts("x", "app")).unwrap();
    }
    std::fs::remove_file(t.p("build/tickets/N-0002.md")).unwrap();
    assert_eq!(create(t.path(), &opts("y", "app")).unwrap().id, "N-0004");
}

#[test]
fn an_empty_inbox_lists_nothing() {
    let (all, problems) = list(Temp::new().path()).unwrap();
    assert!(all.is_empty() && problems.is_empty());
}

#[test]
fn an_unreadable_file_is_a_problem_and_blocks_ids() {
    let t = Temp::new();
    t.write("build/tickets/N-0001.md", "not a ticket");
    let (_, problems) = list(t.path()).unwrap();
    assert_eq!(problems.len(), 1);
    let err = create(t.path(), &opts("x", "app")).unwrap_err().to_string();
    assert!(err.contains("unreadable"), "{err}");
}

#[test]
fn create_needs_a_repo() {
    let err = create(Temp::new().path(), &opts("x", ""))
        .unwrap_err()
        .to_string();
    assert!(err.contains("--repo"), "{err}");
}

#[test]
fn a_planned_ticket_in_the_inbox_is_reported() {
    let t = Temp::new();
    let planned = Ticket::new(NewTicket {
        id: "S-0014".into(),
        typ: STORY.into(),
        title: "x".into(),
        repo: "app".into(),
        ..NewTicket::default()
    })
    .unwrap();
    t.write(
        "build/tickets/S-0014.md",
        &String::from_utf8(planned.to_bytes()).unwrap(),
    );
    let (_, problems) = list(t.path()).unwrap();
    assert_eq!(problems.len(), 1);
    assert!(problems[0].to_string().contains("plan/specs/"));
}

fn draft(external: &str, title: &str) -> Vec<u8> {
    format!(
        "---\nschema: 1\nid: DRAFT\ntype: task\ntitle: {title}\nrepo: api\nchange: 7-gateway\nlabels: [bug]\nstatus: draft\n{external}origin:\n  workspace: w\n  path: p\n---\n\nThe body.\n"
    )
    .into_bytes()
}

/// AC8: a pulled ticket is `X-<pm-id>`, and pulling again replaces it.
#[test]
fn pull_writes_and_replaces() {
    let t = Temp::new();
    let got = pull(
        t.path(),
        &draft("external:\n  github: \"#1234\"\n", "First"),
    )
    .unwrap();
    assert_eq!(got.id, "X-1234");
    assert!(got.path.ends_with("build/tickets/X-1234.md"));
    assert_eq!(got.origin, ticket::Origin::default());
    assert!(got.change.is_empty());
    assert_eq!(got.labels, ["bug"]);
    assert_eq!(got.external["github"], "#1234");
    pull(
        t.path(),
        &draft("external:\n  github: org/app#1234\n", "Second"),
    )
    .unwrap();
    let (all, problems) = list(t.path()).unwrap();
    assert!(problems.is_empty());
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].title, "Second");
    assert!(validate_file(&all[0]).is_empty());
}

#[test]
fn pull_needs_one_external_id() {
    let t = Temp::new();
    for ext in ["", "external:\n  github: \"#1\"\n  ado: \"2\"\n"] {
        let err = pull(t.path(), &draft(ext, "x")).unwrap_err().to_string();
        assert!(err.contains("exactly one external"), "{err}");
    }
    assert_eq!(
        pull(t.path(), &draft("external:\n  github: \"#\"\n", "x"))
            .unwrap_err()
            .to_string(),
        "daiku: ticket: PM id \"\" has no letters or digits"
    );
    assert!(!t.exists("build/tickets"), "a refused pull wrote nothing");
}

#[test]
fn pull_keeps_body_bytes_that_arent_utf8() {
    let t = Temp::new();
    let mut data = draft("external:\n  jira: PROJ-1\n", "x");
    data.extend_from_slice(b"caf\xe9\n");
    pull(t.path(), &data).unwrap();
    let written = std::fs::read(t.p("build/tickets/X-PROJ-1.md")).unwrap();
    assert!(written.ends_with(b"\nThe body.\ncaf\xe9\n"));
}

#[test]
fn pull_refuses_a_title_that_isnt_utf8() {
    let t = Temp::new();
    let data = draft("external:\n  jira: PROJ-1\n", "caf\u{e9}")
        .iter()
        .flat_map(|&b| {
            if b == 0xc3 {
                vec![]
            } else if b == 0xa9 {
                vec![0xe9]
            } else {
                vec![b]
            }
        })
        .collect::<Vec<u8>>();
    let err = pull(t.path(), &data).unwrap_err().to_string();
    assert_eq!(
        err,
        "daiku: invalid ticket: X-PROJ-1: frontmatter field title isn't valid UTF-8"
    );
    assert!(!t.exists("build/tickets"));
}

#[cfg(unix)]
#[test]
fn the_inbox_refuses_symlinks() {
    let (t, outside) = (Temp::new(), Temp::new());
    std::fs::create_dir_all(t.p("build")).unwrap();
    std::os::unix::fs::symlink(outside.path(), t.p("build/tickets")).unwrap();
    let err = create(t.path(), &opts("x", "app")).unwrap_err().to_string();
    assert!(err.contains("is a symlink"), "{err}");
    let err = pull(t.path(), &draft("external:\n  github: \"#1\"\n", "x"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("is a symlink"), "{err}");
}
