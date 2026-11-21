//! placeholder_test.go, translated.

use super::*;

fn m(v: &[(usize, &str)]) -> Vec<Match> {
    v.iter()
        .map(|(l, t)| Match {
            line: *l,
            text: t.to_string(),
        })
        .collect()
}

/// (name, input, expected matches as (line, text)).
type Case<'a> = (&'a str, &'a str, &'a [(usize, &'a str)]);

fn run_cases(cases: &[Case]) {
    for (name, input, want) in cases {
        assert_eq!(find(input), m(want), "{name}: {input:?}");
    }
}

#[test]
fn acceptance_examples() {
    run_cases(&[
        (
            "ellipsis criterion",
            "- [ ] AC1: Given … When … Then …",
            &[(1, "…"), (1, "…"), (1, "…")],
        ),
        (
            "angle criterion",
            "Given <context>, when <action>, then <observable result>",
            &[
                (1, "<context>"),
                (1, "<action>"),
                (1, "<observable result>"),
            ],
        ),
        (
            "code, html and prose",
            "Mentions `List<T>` in inline code.\n```\nx := f(...)\n```\nLine<br>break, <https://x.y>, </p>, <!-- note --> etc...",
            &[],
        ),
    ]);
}

#[test]
fn ellipsis_slots() {
    run_cases(&[
        ("alone", "...", &[(1, "...")]),
        ("start", "... then more", &[(1, "...")]),
        ("middle", "a … b", &[(1, "…")]),
        ("end", "- D2. ...", &[(1, "...")]),
        ("tab separated", "a\t...\tb", &[(1, "...")]),
        ("etc", "and so on etc...", &[]),
        ("wait", "wait… what", &[]),
        ("four dots", "....", &[]),
        ("parenthesised", "(...)", &[]),
        ("two dots", "..", &[]),
        ("next to code", "`x`... and ...`y`", &[]),
        (
            "next to a slot",
            "<x>... and ...<y>",
            &[(1, "<x>"), (1, "<y>")],
        ),
        ("inside a slot", "<x … z>", &[(1, "<x … z>")]),
    ]);
}

#[test]
fn angle_slots_cases() {
    run_cases(&[
        ("word", "<prefix>", &[(1, "<prefix>")]),
        ("generic in prose", "a <T> here", &[(1, "<T>")]),
        ("unlisted element", "<mark>", &[(1, "<mark>")]),
        ("words", "<ISO date>", &[(1, "<ISO date>")]),
        (
            "punctuation",
            "<file path | issue:N | url, one per line>",
            &[(1, "<file path | issue:N | url, one per line>")],
        ),
        ("no letter", "<1> and < > and <>", &[]),
        ("nested opens", "<x <y>", &[(1, "<y>")]),
        ("unclosed", "a < b", &[]),
        ("stray close", "a > b <c>", &[(1, "<c>")]),
        ("closing tag", "</p>", &[]),
        ("doctype", "<!doctype html>", &[]),
        ("url autolink", "<https://x.y>", &[]),
        ("mailto autolink", "<mailto:a@b>", &[]),
        ("mailto upper", "<MAILTO:x>", &[]),
        ("email autolink", "<a@b.c>", &[]),
        ("element", "<br>", &[]),
        ("element upper", "<BR>", &[]),
        ("self-closing", "<br/>", &[]),
        ("self-closing spaced", "<br />", &[]),
        ("attributes", "<a href=\"x\">", &[]),
        ("bare attribute", "<details open>", &[]),
        (
            "every element",
            "<a><b><br><code><details><div><em><hr><i><img><kbd><li><ol><p><pre><span><strong><sub><summary><sup><table><td><th><tr><u><ul>",
            &[],
        ),
        (
            "element prefix is a word",
            "<prefix> <bold> <divider>",
            &[(1, "<prefix>"), (1, "<bold>"), (1, "<divider>")],
        ),
        ("spans lines", "<what outcome —\nnot restated>", &[]),
    ]);
}

#[test]
fn code_and_comments() {
    run_cases(&[
        ("backtick fence", "```\n<x>\n...\n```\n<y>", &[(5, "<y>")]),
        ("tilde fence", "~~~go\n<x>\n~~~\n<y>", &[(4, "<y>")]),
        ("longer close", "```\n<x>\n`````\n<y>", &[(4, "<y>")]),
        (
            "shorter run doesn't close",
            "````\n```\n<x>\n````\n<y>",
            &[(5, "<y>")],
        ),
        (
            "other char doesn't close",
            "```\n~~~\n<x>\n```\n<y>",
            &[(5, "<y>")],
        ),
        (
            "close with trailing text doesn't close",
            "```\n``` x\n<x>\n```\n<y>",
            &[(5, "<y>")],
        ),
        (
            "indented fence in a list",
            "- item\n    ```\n    <x>\n    ```\n- <y>",
            &[(5, "<y>")],
        ),
        ("unclosed fence", "<w>\n```\n<x>\n...", &[(1, "<w>")]),
        (
            "backtick in info string is not a fence",
            "``` a`b\n<x>",
            &[(2, "<x>")],
        ),
        ("two backticks are not a fence", "``\n<x>", &[(2, "<x>")]),
        ("inline code", "`<x>` and `...` <y>", &[(1, "<y>")]),
        ("double backticks", "``a ` <x>`` <y>", &[(1, "<y>")]),
        ("unmatched backtick", "a ` <x>", &[(1, "<x>")]),
        ("comment", "<!-- <x> ... --> <y>", &[(1, "<y>")]),
        (
            "comment over lines",
            "<!--\n<x>\n... -->\n<y>",
            &[(4, "<y>")],
        ),
        ("fence inside comment", "<!--\n```\n-->\n<y>", &[(4, "<y>")]),
        ("empty comment", "<!--> <x>\n<y>", &[(1, "<x>"), (2, "<y>")]),
        (
            "empty comment with dash",
            "<!---> <x>\n<y>",
            &[(1, "<x>"), (2, "<y>")],
        ),
        ("comment of dashes", "<!----> <x>", &[(1, "<x>")]),
        ("unclosed comment", "<w>\n<!-- <x>\n<y>", &[(1, "<w>")]),
        ("comment opener in code", "`<!--` <y>", &[(1, "<y>")]),
        ("backtick in comment", "<!-- ` --> <y> `", &[(1, "<y>")]),
        (
            "slot with code inside",
            "<cite `file:line` here>",
            &[(1, "<cite `file:line` here>")],
        ),
        ("slot inside code", "`<x>`", &[]),
    ]);
}

#[test]
fn lines() {
    run_cases(&[
        ("empty", "", &[]),
        ("none", "plain text\nmore", &[]),
        ("one-based", "a\n\n<x>", &[(3, "<x>")]),
        ("crlf", "a\r\n<x>\r\n...\r\n", &[(2, "<x>"), (3, "...")]),
        (
            "column order",
            "... <x> … <y>",
            &[(1, "..."), (1, "<x>"), (1, "…"), (1, "<y>")],
        ),
    ]);
}

#[test]
fn document_templates() {
    let arch = m(&[
        (1, "<short name>"),
        (4, "<ISO date>"),
        (
            9,
            "<Three to five sentences: the shape of the solution and why it's the right one.>",
        ),
        (
            13,
            "<Current codebase state relevant to this change; cite `file:line` where useful.>",
        ),
        (17, "<decision>"),
        (17, "<rationale>"),
        (17, "<alternatives rejected in brief>"),
        (18, "..."),
        (
            22,
            "<Components, responsibilities, and how they connect. The implementer's map.>",
        ),
        (
            26,
            "<Step-by-step behavior for the main flows, aligned to the acceptance criteria.>",
        ),
        (
            30,
            "<Module boundaries, function/CLI/API shapes, config, env vars.>",
        ),
        (34, "<what the design deliberately leaves out>"),
        (
            38,
            "<assumptions, endpoints to confirm, risks with their impact>",
        ),
        (42, "<references, constraints, implementation hints.>"),
    ]);
    assert_eq!(find(include_str!("testdata/architecture.md")), arch);
    let spec = m(&[
        (1, "<short name of the change>"),
        (4, "<ISO date>"),
        (5, "<file path | issue:N | url, one per line>"),
        (16, "<what this change includes>"),
        (20, "<what it deliberately does not include>"),
        (26, "<capability, verifiable>"),
        (27, "..."),
        (31, "<constraint: performance, security, operability>"),
        (35, "<context>"),
        (35, "<action>"),
        (35, "<observable result>"),
        (36, "..."),
        (40, "<question>"),
        (40, "<resolved answer>"),
        (
            44,
            "<anything unconfirmed; resolve before design or mark as accepted risk>",
        ),
        (48, "<assumptions, constraints, references. Keep light.>"),
    ]);
    assert_eq!(find(include_str!("testdata/functional-spec.md")), spec);
}

#[test]
fn qa_edges() {
    run_cases(&[
        (
            "fence with info string",
            "```go\n<x>\n```\n<y>",
            &[(4, "<y>")],
        ),
        (
            "tab-indented fence",
            "\t~~~\n<x>\n\t~~~\n<y>",
            &[(4, "<y>")],
        ),
        (
            "close bracket inside code inside slot",
            "<x `>` y>",
            &[(1, "<x `>` y>")],
        ),
        (
            "two comments on a line",
            "<!-- a --> <x> <!-- b --> <y>",
            &[(1, "<x>"), (1, "<y>")],
        ),
        (
            "checkbox criterion with slot",
            "- [ ] Given <context>, when <action>, then <result>",
            &[(1, "<context>"), (1, "<action>"), (1, "<result>")],
        ),
        (
            "multibyte text around slots",
            "é <naïve slot> — …",
            &[(1, "<naïve slot>"), (1, "…")],
        ),
        (
            "unicode space around ellipsis",
            "a\u{a0}…\u{a0}b",
            &[(1, "…")],
        ),
        (
            "unmatched double run stays literal",
            "a `` <x> ` <y>",
            &[(1, "<x>"), (1, "<y>")],
        ),
        ("longer run does not close a span", "`x`` <y> `", &[]),
    ]);
}

/// FuzzFind's properties over its seeds and some generated mixes: no panic, line
/// numbers in range, each text on its line.
#[test]
fn find_is_well_behaved() {
    let atoms = [
        "",
        "<x> ...",
        "```\n<x>",
        "<!--\n<x>",
        "`` ` ``",
        "<<>>",
        "…\r\n<é>",
        "<",
        ">",
        "`",
        "\n",
        "...",
        "-->",
        "~~~",
    ];
    let mut inputs: Vec<String> = atoms.iter().map(|s| s.to_string()).collect();
    for a in atoms {
        for b in atoms {
            for c in atoms {
                inputs.push(format!("{a}{b}{c}"));
            }
        }
    }
    for s in inputs {
        let lines: Vec<&str> = s.split('\n').collect();
        for hit in find(&s) {
            assert!(hit.line >= 1 && hit.line <= lines.len(), "{s:?}");
            assert!(
                !hit.text.is_empty() && lines[hit.line - 1].contains(&hit.text),
                "{s:?}"
            );
        }
    }
}
