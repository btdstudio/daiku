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
            "a missing user-owned section is put back with its default (N-0010 R5)",
            "# P\n\n## A\n\nold a\n\n## B\n\nold b\n\n## C\n\nold c\n",
            "# P\n\n## A\n\na body\n\n## B\n\nb body\n\n## Tech stack & standards\n\n- <FILL IN>\n\n## C\n\nc body\n",
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

/// The v0.5.0 table and example line of `## Model routing`, which N-0016 keeps.
const MODEL_ROUTING_TABLE: &str = "| Tier | Use for | Model |
|---|---|---|
| `deep` | judgment: ideas, synthesis, specs, architecture, breakdown, design, review, security | <FILL IN> |
| `standard` | most work: research, exploring repos, settling tickets, requirements, code | <FILL IN> |
| `fast` | mechanical steps: intake, publishing, running tests, shipping | <FILL IN> |

For example, in Claude Code: `opus`, `sonnet`, `haiku`.";

/// N-0016 AC2, AC3, AC4, N1: the intro says `CLAUDE.md` imports AGENTS.md; the
/// workflow rule routes tiers to subagents and names no model; Model routing keeps its
/// table and example line.
#[test]
fn root_text_routes_tiers_and_names_no_model() {
    let real = assemble(&["plan", "build"]);
    let doc = parse(real.as_bytes());
    let pre = joined(&doc.preamble);
    assert!(pre.contains("`CLAUDE.md` imports"), "{pre}");
    assert!(!pre.contains("just points"), "{pre}");

    let body_of = |h: &str| joined(&doc.sections.iter().find(|s| s.heading == h).unwrap().body);
    let rules = body_of("Workflow rules");
    for want in [
        "`fast`",
        "`standard`",
        "`deep`",
        "subagent",
        "`## Model routing`",
        "`daiku gate`",
        "`daiku reopen`",
        "`daiku change close`",
        "`daiku switch`",
        "`daiku commit`",
        "`daiku pr`",
        "persona",
        "`Read` and `Skip`",
    ] {
        assert!(rules.contains(want), "Workflow rules lacks {want}");
    }
    let routing = body_of("Model routing");
    assert!(routing.contains("subagent"), "{routing}");
    assert!(routing.contains(MODEL_ROUTING_TABLE), "{routing}");

    let words = |text: &str| {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    for sec in &doc.sections {
        if sec.heading == "Model routing" {
            continue;
        }
        let w = words(&joined(&sec.body));
        for id in ["opus", "sonnet", "haiku"] {
            assert!(!w.iter().any(|x| x == id), "{} names {id}", sec.heading);
        }
    }
    let w = words(&pre);
    assert!(
        !w.iter()
            .any(|x| ["opus", "sonnet", "haiku"].contains(&x.as_str()))
    );
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

// ---- N-0010: model tiers ----

const ROUTING_HEADER: &str = "| Task / Intent | Directory | Persona | Model | Read | Skip |";

/// Every routing row's task and its Model cell, without the backticks.
fn routing_tiers(text: &str) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        if line == ROUTING_HEADER {
            in_table = true;
            continue;
        }
        if !in_table || line.starts_with("|---") {
            continue;
        }
        if !line.starts_with('|') {
            in_table = false;
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split(" | ").collect();
        assert_eq!(cells.len(), 6, "{line}");
        rows.push((
            cells[0].trim().to_string(),
            cells[3].trim().trim_matches('`').to_string(),
        ));
    }
    rows
}

/// AC1, R1, R2: each routing table has the Model column, and each row its tier.
#[test]
fn routing_tables_name_a_tier_per_row() {
    let text = assemble(&["plan", "build"]);
    assert!(
        !text.contains("| Persona | Read |"),
        "a table without the column"
    );
    let want = [
        ("Work an idea", "deep"),
        ("Define the base source", "fast"),
        ("External research", "standard"),
        ("Explore the repos", "standard"),
        ("Synthesize findings", "deep"),
        ("Write the spec", "deep"),
        ("Design the architecture", "deep"),
        ("Break down into tickets", "deep"),
        ("Publish tickets", "fast"),
        ("Settle the ticket", "standard"),
        ("Write requirements", "standard"),
        ("Design solution", "deep"),
        ("Break down into tasks", "deep"),
        ("Implement code", "standard"),
        ("Test", "fast"),
        ("Review", "deep"),
        ("Security pass", "deep"),
        ("Ship", "fast"),
    ];
    let want: Vec<(String, String)> = want
        .iter()
        .map(|(t, m)| (t.to_string(), m.to_string()))
        .collect();
    assert_eq!(routing_tiers(&text), want);

    // AC2: the section comes straight after Tech stack, and holds its parts.
    let doc = parse(assemble(&[]).as_bytes());
    let at = |h: &str| doc.sections.iter().position(|s| s.heading == h).unwrap();
    assert_eq!(at("Model routing"), at("Tech stack & standards") + 1);
    let body = joined(&doc.sections[at("Model routing")].body);
    for part in [
        "doesn't switch the current session's model",
        "| Tier | Use for | Model |",
        "| `deep` |",
        "| `standard` |",
        "| `fast` |",
        "in Claude Code: `opus`, `sonnet`, `haiku`",
    ] {
        assert!(body.contains(part), "{part}\n{body}");
    }
}

/// The real scaffold with every section, `## Tech stack` edited, an optional
/// `## Model routing` replacement (`None` drops it), and `## Team notes` at the end.
fn real_user_file(model_routing: Option<&str>) -> (String, String) {
    let real = assemble(&["plan", "build"]);
    let doc = parse(real.as_bytes());
    let mut user = format!("{}\n", joined(&doc.preamble));
    for sec in &doc.sections {
        match sec.heading.as_str() {
            "Tech stack & standards" => {
                user += "## Tech stack & standards\n\n- Rust 1.100, Postgres\n\n";
            }
            "Model routing" => {
                if let Some(text) = model_routing {
                    user += text;
                }
            }
            _ => user += &format!("{}\n\n", joined(&sec.body)),
        }
    }
    user += "## Team notes\n\nkeep me\n";
    (user, real)
}

/// The routing tables as the scaffold had them before N-0010: no Model column.
fn without_model_column(text: &str) -> String {
    let mut out = Vec::new();
    let mut in_table = false;
    for line in text.split('\n') {
        if line == ROUTING_HEADER {
            in_table = true;
        } else if !line.starts_with('|') {
            in_table = false;
        }
        if in_table {
            let mut cells: Vec<&str> = line.split('|').collect();
            cells.remove(4);
            out.push(cells.join("|"));
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// AC4, R7: an AGENTS.md from before N-0010 gains the column and `## Model routing`
/// after Tech stack, keeps the user's text, and a second merge changes nothing.
#[test]
fn merge_adds_model_routing_to_an_old_file() {
    let (old, real) = real_user_file(None);
    let old = without_model_column(&old);
    assert!(!old.contains("\n## Model routing\n") && !old.contains("| Persona | Model |"));
    let doc = parse(real.as_bytes());
    let default = doc
        .sections
        .iter()
        .find(|s| s.heading == "Model routing")
        .unwrap();
    let (want, _) = real_user_file(Some(&format!("{}\n\n", joined(&default.body))));
    let got = merge_s(&old, &real).unwrap();
    assert_eq!(got, want);
    assert_eq!(merge_s(&got, &real).unwrap(), got, "idempotent");
}

/// AC3, R4: an edited `## Model routing` is kept byte for byte.
#[test]
fn merge_keeps_an_edited_model_routing() {
    let mine = "## Model routing\n\n| Tier | Model |\n|---|---|\n| `deep` | opus |\n\nWe use opus for everything deep.\n\n";
    let (user, real) = real_user_file(Some(mine));
    let got = merge_s(&user, &real).unwrap();
    assert_eq!(got, user);
    assert!(got.contains("We use opus for everything deep."));
}
