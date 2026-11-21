//! `daiku check`'s term lint: build-room docs may not use plan-only terms, and plan
//! docs may not use build-only ones (D10). A port of Go's `internal/icm/lint.go`.

use regex::Regex;

use crate::scaffold::engine;

pub(crate) const PLAN_ONLY: [&str; 18] = [
    "functional spec",
    "functional-spec",
    "architecture.md",
    "ADR",
    "researcher",
    "explorer",
    "publisher",
    "synthesis",
    "00_intake",
    "01_research",
    "02_explore",
    "03_spec",
    "04_architecture",
    "05_breakdown",
    "06_publish",
    "change new",
    "tickets ready",
    "mark-published",
];

pub(crate) const BUILD_ONLY: [&str; 13] = [
    "tasks.json",
    "test-results",
    "code-review",
    "security-review",
    "coder",
    "01_design",
    "02_implementation",
    "03_testing",
    "04_refactoring",
    "build new",
    "tickets pull",
    "daiku pr",
    "daiku commit",
];

/// A whole term, any case: not touching a letter, digit or `_` on either side.
pub(crate) fn term_pattern(term: &str) -> Regex {
    Regex::new(&format!(
        r"(?i)(?:^|[^\pL\pN_]){}(?:$|[^\pL\pN_])",
        regex::escape(term)
    ))
    .unwrap_or_else(|e| panic!("term pattern for {term:?}: {e}"))
}

/// A room to lint: its name, the room whose terms it may not use, those terms, and
/// its documents as (display path, text).
pub(crate) struct LintRoom<'a> {
    pub name: &'a str,
    pub owner: &'a str,
    pub foreign: &'a [&'a str],
    pub docs: Vec<(String, String)>,
}

pub(crate) fn lint_docs(rooms: &[LintRoom]) -> Vec<String> {
    let mut problems = Vec::new();
    for room in rooms {
        let pats: Vec<(&str, Regex)> = room.foreign.iter().map(|t| (*t, term_pattern(t))).collect();
        for (display, text) in &room.docs {
            for (n, line) in text.lines().enumerate() {
                for (term, re) in &pats {
                    if re.is_match(line) {
                        problems.push(format!(
                            "{display}:{}: {}-only term {} in the {} room",
                            n + 1,
                            room.owner,
                            crate::gofmt::quote(term),
                            room.name
                        ));
                    }
                }
            }
        }
    }
    problems
}

/// The markdown files of a room's tree plus its AGENTS fragment, as Go's
/// `scaffoldRooms` lists them, with Go's display paths.
fn room_docs(room: &str) -> Vec<(String, String)> {
    let mut docs: Vec<(String, String)> = engine::walk_tree(room, "", false)
        .into_iter()
        .filter(|(rel, _)| rel.ends_with(".md"))
        .map(|(rel, data)| {
            (
                format!("scaffold/{room}/{rel}"),
                String::from_utf8_lossy(data).into_owned(),
            )
        })
        .collect();
    if let Some(text) = engine::agents_file(&format!("{room}.md")) {
        docs.push((format!("scaffold/agents/{room}.md"), text.to_string()));
    }
    docs
}

/// The lint over the embedded scaffold.
pub(crate) fn lint_scaffold() -> Vec<String> {
    lint_docs(&[
        LintRoom {
            name: "build",
            owner: "plan",
            foreign: &PLAN_ONLY,
            docs: room_docs("build"),
        },
        LintRoom {
            name: "plan",
            owner: "build",
            foreign: &BUILD_ONLY,
            docs: room_docs("plan"),
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room<'a>(
        name: &'a str,
        owner: &'a str,
        foreign: &'a [&'a str],
        docs: &[(&str, &str)],
    ) -> LintRoom<'a> {
        LintRoom {
            name,
            owner,
            foreign,
            docs: docs
                .iter()
                .filter(|(p, _)| p.ends_with(".md"))
                .map(|(p, t)| (format!("scaffold/{name}/{p}"), t.to_string()))
                .collect(),
        }
    }

    // lint_test.go, TestLintBuildDocUsingAPlanTerm.
    #[test]
    fn build_doc_using_a_plan_term() {
        let got = lint_docs(&[room(
            "build",
            "plan",
            &PLAN_ONLY,
            &[(
                "01_design/personas/analyst.md",
                "# analyst\n\nRead the Functional spec first.\n",
            )],
        )]);
        assert_eq!(
            got,
            [
                "scaffold/build/01_design/personas/analyst.md:3: plan-only term \"functional spec\" in the build room"
            ]
        );
    }

    // TestLintPlanDocUsingABuildTerm.
    #[test]
    fn plan_doc_using_a_build_term() {
        let shared = "Each ticket has requirements, a design, a review and one repo.\n";
        let plan_doc = format!("{shared}Write tasks.json next.\n");
        let got = lint_docs(&[
            room(
                "plan",
                "build",
                &BUILD_ONLY,
                &[
                    ("05_breakdown/context.md", &plan_doc),
                    (
                        "notes.txt",
                        "tasks.json in a non-markdown file is ignored\n",
                    ),
                ],
            ),
            room("build", "plan", &PLAN_ONLY, &[("context.md", shared)]),
        ]);
        assert_eq!(
            got,
            [
                "scaffold/plan/05_breakdown/context.md:2: build-only term \"tasks.json\" in the plan room"
            ]
        );
    }

    // TestLintTermBoundaries.
    #[test]
    fn term_boundaries() {
        for (term, line, want) in [
            ("ADR", "See ADR-0001.", true),
            ("functional spec", "FUNCTIONAL SPEC", true),
            ("functional spec", "(functional spec)", true),
            ("ADR", "the adrDir setting", false),
            ("coder", "an encoder", false),
            ("coder", "coders", false),
            ("coder", "the coder writes it", true),
            ("04_architecture", "plan/04_architecture/adr/x", true),
            ("functional spec", "functional specification", false),
            ("daiku pr", "run `daiku pr` from the repo", true),
            ("ADR", "éADRé", false),
        ] {
            assert_eq!(
                term_pattern(term).is_match(line),
                want,
                "{term:?} in {line:?}"
            );
        }
    }

    // TestShippedScaffoldIsClean.
    #[test]
    fn shipped_scaffold_is_clean() {
        assert_eq!(lint_scaffold(), Vec::<String>::new());
    }

    // TestQALintChecksFragments: the fragments are linted too.
    #[test]
    fn fragments_are_linted() {
        let docs = room_docs("build");
        assert!(docs.iter().any(|(p, _)| p == "scaffold/agents/build.md"));
        assert!(
            docs.iter()
                .all(|(p, _)| p.starts_with("scaffold/build/") || p == "scaffold/agents/build.md")
        );
        assert!(
            room_docs("plan")
                .iter()
                .any(|(p, _)| p == "scaffold/agents/plan.md")
        );
    }
}
