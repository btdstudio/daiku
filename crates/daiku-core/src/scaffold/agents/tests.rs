//! merge_test.go and agents_test.go, translated.

use super::*;

/// [`merge`] on text, for cases written as strings.
fn merge_s(user: &str, scaffold: &str) -> Result<String, String> {
    merge(user.as_bytes(), scaffold).map(|b| String::from_utf8(b).unwrap())
}

fn merge_with_s(
    user: &str,
    scaffold: &str,
    owned: &[&str],
    retired: &[&str],
) -> Result<String, String> {
    merge_with(user.as_bytes(), scaffold, owned, retired).map(|b| String::from_utf8(b).unwrap())
}

/// Lines joined by `\n`, as text.
fn joined(lines: &[Line]) -> String {
    String::from_utf8(lines.join(&b'\n')).unwrap()
}

const SCAF: &str = "# T\n\nintro\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\n- <FILL IN>\n\n## C\n\nc body\n";

#[test]
fn merge_cases() {
    let cases: [(&str, &str, &str); 13] = [
        (
            "keeps the user's sections, title and intro; updates daiku's",
            "# My project\n\nMy own intro.\n\n## A\n\nold a\n\n## Team notes\n\nnotes\n\n## B\n\nold b\n\n## Tech stack & standards\n\n- Go 1.22\n\n## C\n\nold c\n",
            "# My project\n\nMy own intro.\n\n## A\n\na body\n\n## Team notes\n\nnotes\n\n## B\n\nb body\n\n## Tech stack & standards\n\n- Go 1.22\n\n## C\n\nc body\n",
        ),
        (
            "a ## line inside a fence isn't a heading",
            "# P\n\n## A\n\nold a\n\n## B\n\nold b\n\n```\n## not a heading\n```\n\n## Tech stack & standards\n\nx\n\n```\n## A\n```\n\n~~~\n## A\n~~~\n\n````\n```\n## C\n````\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n```\n## A\n```\n\n~~~\n## A\n~~~\n\n````\n```\n## C\n````\n\n## C\n\nc body\n",
        ),
        (
            "a ### subsection belongs to its section",
            "# P\n\n## A\n\nold a\n\n### sub\n\nmore\n\n## B\n\nold b\n\n## Tech stack & standards\n\nx\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n",
        ),
        (
            "reordered sections keep the user's order",
            "# P\n\n## C\n\nold c\n\n## Tech stack & standards\n\nx\n\n## B\n\nold b\n\n## A\n\nold a\n",
            "# P\n\n## C\n\nc body\n\n## Tech stack & standards\n\nx\n\n## B\n\nb body\n\n## A\n\na body\n",
        ),
        (
            "a missing section goes after its preceding scaffold section",
            "# P\n\n## A\n\nold a\n\n## Tech stack & standards\n\nx\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n",
        ),
        (
            "a missing first section goes right after the intro",
            "# P\n\nintro\n\n## B\n\nold b\n\n## Tech stack & standards\n\nx\n\n## C\n\nold c\n",
            "# P\n\nintro\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n",
        ),
        (
            "a missing first section after an intro with no trailing blank line",
            "# P\n## B\n\nold b\n\n## Tech stack & standards\n\nx\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n",
        ),
        (
            "a missing last section after a section with no trail, and no final newline",
            "# P\n\n## A\n\nold a\n\n## B\n\nold b\n\n## Tech stack & standards\n\nx",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body",
        ),
        (
            "a missing section inserted before a user section keeps a blank line",
            "# P\n\n## A\n\nold a\n\n## B\n\nold b\n\n## Tech stack & standards\n\nx\n\n## Team notes\n\nn\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n\n## Team notes\n\nn\n",
        ),
        (
            "a deleted user-owned section isn't put back",
            "# P\n\n## A\n\nold a\n\n## B\n\nold b\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## C\n\nc body\n",
        ),
        (
            "no final newline is kept",
            "# P\n\n## A\n\nold a\n\n## B\n\nold b\n\n## Tech stack & standards\n\nx\n\n## C\n\nold c",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body",
        ),
        (
            "the user's spacing between sections is kept",
            "# P\n\n## A\n\nold a\n\n\n\n## B\n\nold b\n## Tech stack & standards\n\nx\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n\n\n## B\n\nb body\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n",
        ),
        ("an up-to-date file is unchanged", SCAF, SCAF),
    ];
    for (name, user, want) in cases {
        let got = merge_s(user, SCAF).unwrap_or_else(|r| panic!("{name}: refused: {r}"));
        assert_eq!(got, want, "{name}");
        assert_eq!(merge_s(&got, SCAF).unwrap(), got, "{name}: not idempotent");
    }
}

#[test]
fn merge_refuses() {
    for (name, user, reason) in [
        ("no ## headings", "mine", "no ## headings"),
        ("only a title", "# P\n\njust text\n", "no ## headings"),
        ("empty", "", "no ## headings"),
        (
            "headings only in a fence",
            "# P\n\n```\n## A\n```\n",
            "no ## headings",
        ),
        (
            "a duplicated daiku heading",
            "# P\n\n## A\n\none\n\n## B\n\nb\n\n## A\n\ntwo\n",
            "duplicated heading \"A\"",
        ),
        (
            "an unclosed fence in a daiku section",
            "# P\n\n## A\n\nold a\n\nExample:\n```sh\ndaiku status\n\n## Tech stack & standards\n\nmine\n\n## Team notes\n\nkeep\n",
            "unclosed code fence",
        ),
        (
            "an unclosed fence in a user section",
            "# P\n\n## A\n\nold a\n\n## Team notes\n\n~~~\nnotes\n",
            "unclosed code fence",
        ),
        (
            "an unclosed fence before any heading",
            "# P\n\n````\n## A\n```\n",
            "unclosed code fence",
        ),
    ] {
        assert_eq!(merge_s(user, SCAF), Err(reason.to_string()), "{name}");
    }
    let user = "# P\n\n## Team notes\n\n1\n\n## Team notes\n\n2\n\n## Tech stack & standards\n\nx\n\n## Tech stack & standards\n\ny\n";
    let got = merge_s(user, SCAF).unwrap();
    assert!(got.contains("\n1\n") && got.contains("\n2\n") && got.contains("\ny\n"));
}

#[test]
fn merge_drops_retired_sections() {
    let user = "# P\n\n## Old\n\nstale\n\n## A\n\nold a\n\n## B\n\nold b\n\n## Tech stack & standards\n\nx\n\n## C\n\nold c\n";
    let want = "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\nx\n\n## C\n\nc body\n";
    let got = merge_with_s(user, SCAF, &USER_OWNED, &["Old"]).unwrap();
    assert_eq!(got, want);
    assert_eq!(
        merge_with_s(&got, SCAF, &USER_OWNED, &["Old"]).unwrap(),
        got
    );
}

/// AC6 on the real scaffold: an edited Tech stack and the user's own section are
/// kept; every daiku section is refreshed; the merge is idempotent.
#[test]
fn merge_real_scaffold_keeps_owner_sections() {
    let real = assemble(&["plan"]);
    let doc = parse(real.as_bytes());
    let mut user = format!("{}\n", joined(&doc.preamble));
    for sec in &doc.sections {
        user += &match sec.heading.as_str() {
            "Tech stack & standards" => {
                "## Tech stack & standards\n\n- Go 1.22, Postgres\n\n".to_string()
            }
            "Plan routing" => "## Plan routing\n\nstale\n\n".to_string(),
            _ => format!("{}\n\n", joined(&sec.body)),
        };
    }
    user += "## Team notes\n\nkeep me\n";
    let got = merge_s(&user, &real).unwrap();
    let merged = parse(got.as_bytes());
    assert_eq!(merged.sections.len(), doc.sections.len() + 1);
    for sec in &merged.sections {
        let body = joined(&sec.body);
        match sec.heading.as_str() {
            "Tech stack & standards" => assert!(body.contains("Go 1.22, Postgres"), "{body}"),
            "Team notes" => assert!(body.contains("keep me"), "{body}"),
            h => {
                let want = doc.sections.iter().find(|s| s.heading == h).unwrap();
                assert_eq!(sec.body, want.body, "{h}");
            }
        }
    }
    assert!(got.contains("## Plan routing") && !got.contains("\nstale\n"));
    assert_eq!(merge_s(&got, &real).unwrap(), got);
}

#[test]
fn scaffold_headings_are_unique() {
    for rooms in [&[][..], &["plan"], &["build"], &["plan", "build"]] {
        let doc = parse(assemble(rooms).as_bytes());
        let mut seen = std::collections::HashSet::new();
        assert!(!doc.sections.is_empty());
        for s in &doc.sections {
            assert!(seen.insert(s.heading.clone()), "repeated {}", s.heading);
        }
        assert!(!doc.unclosed_fence);
        for owned in USER_OWNED {
            assert!(seen.contains(owned), "{owned}");
        }
    }
}

#[test]
fn assemble_joins_fragments_in_order() {
    let root = engine::agents_file("root.md").unwrap();
    let plan = engine::agents_file("plan.md").unwrap();
    let build = engine::agents_file("build.md").unwrap();
    assert_eq!(assemble(&[]), format!("{}\n", root.trim_end_matches('\n')));
    assert_eq!(
        assemble(&["build", "plan"]),
        format!(
            "{}\n\n{}\n\n{}\n",
            root.trim_end_matches('\n'),
            plan.trim_end_matches('\n'),
            build.trim_end_matches('\n')
        )
    );
}

/// N-0001 AC5: a user's section with bytes that aren't valid UTF-8 is kept byte
/// for byte while a stale daiku section is refreshed.
#[test]
fn merge_keeps_a_users_bytes() {
    let user: &[u8] = b"# Caf\xe9\n\n## A\n\nold a\n\n## B\n\nold b\n\n## Tech stack & standards\n\n- Caf\xe9 notes.\n\n## Mine \xff\n\nmy \xfe text\n\n## C\n\nold c\n";
    let want: &[u8] = b"# Caf\xe9\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\n- Caf\xe9 notes.\n\n## Mine \xff\n\nmy \xfe text\n\n## C\n\nc body\n";
    let got = merge(user, SCAF).unwrap();
    assert_eq!(got, want);
    assert_eq!(merge(&got, SCAF).unwrap(), got, "idempotent");
}

/// N-0001 AC5: a current file with bytes that aren't valid UTF-8 is unchanged:
/// the comparison is on bytes, so nothing is rewritten.
#[test]
fn a_current_file_with_invalid_bytes_is_unchanged() {
    let t = crate::testutil::Temp::new();
    let current = assemble(&["plan"]);
    let mut user = current.clone().into_bytes();
    let at = user.len();
    user.splice(at..at, b"\n## Mine\n\nCaf\xe9\n".iter().copied());
    std::fs::write(t.p("AGENTS.md"), &user).unwrap();
    assert_eq!(
        write_agents(t.path(), &["plan"], false).unwrap(),
        AgentsResult::Unchanged
    );
    assert_eq!(std::fs::read(t.p("AGENTS.md")).unwrap(), user);

    let routing = b"## Plan routing\n";
    let i = user
        .windows(routing.len())
        .position(|w| w == routing)
        .unwrap()
        + routing.len();
    let mut stale = user.clone();
    stale.splice(i..i, b"\nstale\n".iter().copied());
    std::fs::write(t.p("AGENTS.md"), &stale).unwrap();
    assert_eq!(
        write_agents(t.path(), &["plan"], false).unwrap(),
        AgentsResult::Merged
    );
    assert_eq!(
        std::fs::read(t.p("AGENTS.md")).unwrap(),
        user,
        "refreshed, bytes kept"
    );
}
