//! The `daiku-fixtures` binary: output and exit codes.

use std::path::Path;
use std::process::{Command, Output};

fn fixtures(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_daiku-fixtures"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run daiku-fixtures")
}

fn stub() -> &'static str {
    env!("CARGO_BIN_EXE_daiku-fixtures-stub")
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).replace("\r\n", "\n")
}

fn write(path: &Path, s: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, s).unwrap();
}

/// A case running the stub with `args` (a TOML list body).
fn case(cases: &Path, name: &str, toml: &str) {
    write(&cases.join(name).join("case.toml"), toml);
}

fn current_os() -> &'static str {
    std::env::consts::OS
}

#[test]
fn usage_errors_exit_2() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        &[][..],
        &["bogus"],
        &["check"],
        &["check", "--bin"],
        &["check", "--bin", "x", "--usage", "u"],
        &["coverage", "--bin", "x"],
        &["coverage", "name"],
        &["capture", "--nope"],
    ] {
        let out = fixtures(args, dir.path());
        assert_eq!(
            out.status.code(),
            Some(2),
            "{args:?}: {}",
            text(&out.stderr)
        );
        assert!(text(&out.stderr).contains("usage:"), "{args:?}");
    }
}

#[test]
fn check_passes_fails_and_skips() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    case(&cases, "good", "args = [\"print\", \"hi\"]\n");
    case(
        &cases,
        "skipped",
        &format!(
            "args = [\"print\", \"hi\"]\nskip_on = [\"{}\"]\n",
            current_os()
        ),
    );

    let cases_arg = cases.to_str().unwrap();
    let cap = fixtures(
        &["capture", "--bin", stub(), "--cases", cases_arg],
        dir.path(),
    );
    assert_eq!(cap.status.code(), Some(0), "{}", text(&cap.stdout));
    assert_eq!(
        text(&cap.stdout),
        format!(
            "captured good\nskip skipped ({})\n1 captured, 0 failed, 1 skipped\n",
            current_os()
        )
    );

    let ok = fixtures(
        &["check", "--bin", stub(), "--cases", cases_arg],
        dir.path(),
    );
    assert_eq!(ok.status.code(), Some(0), "{}", text(&ok.stdout));
    assert_eq!(
        text(&ok.stdout),
        format!(
            "ok   good\nskip skipped ({})\n1 passed, 0 failed, 1 skipped\n",
            current_os()
        )
    );

    // Change the args and keep the captured [expect].
    let file = cases.join("good/case.toml");
    let toml = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, toml.replacen("\"hi\"]", "\"ho\"]", 1)).unwrap();
    let bad = fixtures(
        &["check", "--bin", stub(), "--cases", cases_arg, "good"],
        dir.path(),
    );
    assert_eq!(bad.status.code(), Some(1));
    let s = text(&bad.stdout);
    assert!(s.starts_with("FAIL good\n  stdout:\n"), "{s}");
    assert!(s.contains("-hi") && s.contains("+ho"), "{s}");
    assert!(s.ends_with("0 passed, 1 failed, 0 skipped\n"), "{s}");
}

#[test]
fn run_errors_fail_the_case() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    case(&cases, "a", "args = []\n");
    let out = fixtures(
        &[
            "check",
            "--bin",
            "/no/such/daiku",
            "--cases",
            cases.to_str().unwrap(),
        ],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        text(&out.stdout).starts_with("FAIL a\n  error: "),
        "{}",
        text(&out.stdout)
    );

    let out = fixtures(
        &[
            "capture",
            "--bin",
            "/no/such/daiku",
            "--cases",
            cases.to_str().unwrap(),
        ],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn unknown_case_name_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("cases")).unwrap();
    let out = fixtures(
        &["check", "--bin", stub(), "--cases", "cases", "nope"],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("no such case"));
}

#[test]
fn coverage_lists_uncovered_entries() {
    let dir = tempfile::tempdir().unwrap();
    write(
        &dir.path().join("usage.txt"),
        "  daiku init [dir]\n  daiku tickets check\n",
    );
    case(&dir.path().join("cases"), "init", "args = [\"init\"]\n");
    let out = fixtures(
        &["coverage", "--cases", "cases", "--usage", "usage.txt"],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stdout),
        "uncovered: tickets check\n1 of 2 covered\n"
    );

    case(
        &dir.path().join("cases"),
        "check",
        "args = [\"tickets\", \"check\"]\n",
    );
    let out = fixtures(
        &["coverage", "--cases", "cases", "--usage", "usage.txt"],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(text(&out.stdout), "2 of 2 covered\n");
}

#[test]
fn checked_in_cases_cover_every_usage_entry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = fixtures(&["coverage"], &root);
    let s = text(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "{s}");
    assert_eq!(s, "43 of 43 covered\n");
}

// ---- T10: the self-test case suite ----

fn suite_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases")
}

/// Each case's result: `ok`, `skip`, or `FAIL <first part>`.
fn results(stdout: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = stdout.lines().collect();
    let mut out = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if let Some(name) = line.strip_prefix("ok   ") {
            out.push((name.to_string(), "ok".to_string()));
        } else if let Some(rest) = line.strip_prefix("skip ") {
            let name = rest.split(' ').next().unwrap();
            out.push((name.to_string(), "skip".to_string()));
        } else if let Some(name) = line.strip_prefix("FAIL ") {
            let part = lines
                .get(i + 1)
                .map(|l| l.trim().trim_end_matches(':'))
                .unwrap_or("");
            out.push((name.to_string(), format!("FAIL {part}")));
        }
    }
    out
}

#[test]
fn suite_cases_give_their_intended_results() {
    let suite = suite_dir();
    let out = fixtures(
        &["check", "--bin", stub(), "--cases", suite.to_str().unwrap()],
        &suite,
    );
    let s = text(&out.stdout);
    assert_eq!(out.status.code(), Some(1), "{s}");
    let want: Vec<(String, String)> = [
        ("empty-folder", "ok"),
        ("exit-diff", "FAIL exit"),
        ("git-two-commits", "ok"),
        ("json-by-value", "ok"),
        ("normalisation", "ok"),
        ("pass", "ok"),
        ("skip-on", "skip"),
        ("stdout-diff", "FAIL stdout"),
        ("tree-extra", "FAIL changes"),
        ("tree-missing", "FAIL changes"),
        ("unordered-stdout", "ok"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(results(&s), want, "{s}");
    assert!(s.contains("    ++ f extra.txt\n"), "{s}");
    assert!(s.contains("    -+ f gone.txt\n"), "{s}");
    assert!(s.contains("    -world!\n    +world\n"), "{s}");
    assert!(s.ends_with("6 passed, 4 failed, 1 skipped\n"), "{s}");
}

#[test]
fn passing_suite_cases_check_alone_exit_0() {
    let suite = suite_dir();
    let out = fixtures(
        &[
            "check",
            "--bin",
            stub(),
            "--cases",
            suite.to_str().unwrap(),
            "pass",
            "normalisation",
            "skip-on",
        ],
        &suite,
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
}

#[test]
fn capture_then_check_on_a_copied_case() {
    let dir = tempfile::tempdir().unwrap();
    let from = suite_dir().join("git-two-commits");
    let to = dir.path().join("cases/copy");
    let original = std::fs::read_to_string(from.join("case.toml")).unwrap();
    let inputs = original.split("\n[expect]\n").next().unwrap();
    write(&to.join("case.toml"), inputs);
    let cases = dir.path().join("cases");
    let cases = cases.to_str().unwrap();
    assert_eq!(
        fixtures(&["check", "--bin", stub(), "--cases", cases], dir.path())
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        fixtures(&["capture", "--bin", stub(), "--cases", cases], dir.path())
            .status
            .code(),
        Some(0)
    );
    let ok = fixtures(&["check", "--bin", stub(), "--cases", cases], dir.path());
    assert_eq!(ok.status.code(), Some(0), "{}", text(&ok.stdout));
    assert_eq!(
        std::fs::read_to_string(to.join("case.toml")).unwrap(),
        original,
        "capture writes the suite case's [expect] table exactly"
    );
}

// ---- QA: AC8 with an explicit canary from the caller's environment ----

#[test]
fn caller_environment_does_not_reach_the_binary() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    case(&cases, "env", "args = [\"env\"]\n");
    let out = Command::new(env!("CARGO_BIN_EXE_daiku-fixtures"))
        .args([
            "capture",
            "--bin",
            stub(),
            "--cases",
            cases.to_str().unwrap(),
        ])
        .env("DAIKU_FX_CANARY", "leaked")
        .env("TZ", "America/New_York")
        .env("LANG", "fr_FR.UTF-8")
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    let env = daiku_fixtures::Case::load(&cases.join("env"))
        .unwrap()
        .spec
        .expect
        .unwrap()
        .stdout;
    assert!(!env.contains("DAIKU_FX_CANARY"), "{env}");
    assert!(!env.contains("leaked"), "{env}");
    assert!(env.lines().any(|l| l == "TZ=UTC"), "{env}");
    assert!(env.lines().any(|l| l == "LANG=C"), "{env}");
}

// ---- S-0002 T4: capture warns about run dates in starting files ----

#[test]
fn capture_warns_about_todays_date_and_still_captures() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    let today = daiku_fixtures::civil(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64,
    )
    .0;
    case(&cases, "dated", "args = [\"print\", \"x\"]\n");
    write(
        &cases.join("dated/before/notes.md"),
        &format!("made {today}\n"),
    );
    write(
        &cases.join("dated/config/daiku/s.json"),
        &format!("\"{today}\""),
    );
    case(&cases, "past", "args = [\"print\", \"x\"]\n");
    write(&cases.join("past/before/notes.md"), "made 2026-01-02\n");
    let c = cases.to_str().unwrap();
    let out = fixtures(&["capture", "--bin", stub(), "--cases", c], dir.path());
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let err = text(&out.stderr);
    assert!(
        err.contains(&format!(
            "warning: dated: before/notes.md contains today's date {today}"
        )),
        "{err}"
    );
    assert!(err.contains("warning: dated: config/daiku/s.json"), "{err}");
    assert!(!err.contains("past"), "{err}");
    let toml = std::fs::read_to_string(cases.join("dated/case.toml")).unwrap();
    assert!(toml.contains("\n[expect]\nexit = 0\n"), "{toml}");
}

#[test]
fn workspaces_command_builds_and_reports() {
    let dir = tempfile::tempdir().unwrap();
    let ws = dir.path().join("ws");
    write(
        &ws.join("w/recipe.toml"),
        "steps = [[\"write\", \"a\", \"b\"]]\n",
    );
    let w = ws.to_str().unwrap();
    let out = fixtures(
        &["workspaces", "--bin", stub(), "--workspaces", w],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(text(&out.stdout), "built w\n1 built\n");
    assert_eq!(std::fs::read(ws.join("w/tree/a")).unwrap(), b"b");

    write(&ws.join("x/recipe.toml"), "steps = [[\"exit\", \"1\"]]\n");
    let out = fixtures(
        &["workspaces", "--bin", stub(), "--workspaces", w, "x"],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    for args in [
        &["workspaces"][..],
        &["workspaces", "--bin", "s", "--cases", "c"],
    ] {
        assert_eq!(
            fixtures(args, dir.path()).status.code(),
            Some(2),
            "{args:?}"
        );
    }
}

// ---- S-0002 T15: the error-path checklist names every case, and only cases ----

/// Backticked names in the checklist's table rows.
fn checklist_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines().filter(|l| l.starts_with('|')) {
        let parts: Vec<&str> = line.split('`').collect();
        names.extend(parts.iter().skip(1).step_by(2).map(|s| s.to_string()));
    }
    names
}

/// Problems between a checklist and a set of case folders: names that aren't
/// cases, cases that aren't named, and names given twice.
fn checklist_problems(text: &str, cases: &[String]) -> Vec<String> {
    let names = checklist_names(text);
    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for n in &names {
        if !cases.contains(n) {
            problems.push(format!("named but not a case: {n}"));
        }
        if !seen.insert(n.clone()) {
            problems.push(format!("named twice: {n}"));
        }
    }
    for c in cases {
        if !seen.contains(c) {
            problems.push(format!("case not named: {c}"));
        }
    }
    problems
}

fn repo_fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

#[test]
fn error_paths_name_every_case() {
    let dir = repo_fixtures();
    let text = std::fs::read_to_string(dir.join("ERROR-PATHS.md")).unwrap();
    let mut cases: Vec<String> = std::fs::read_dir(dir.join("cases"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("case.toml").is_file())
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    cases.sort();
    let problems = checklist_problems(&text, &cases);
    assert!(
        problems.is_empty(),
        "fixtures/ERROR-PATHS.md:\n{}",
        problems.join("\n")
    );
}

#[test]
fn checklist_check_catches_renames_and_gaps() {
    let text = "| args | `a`, `b` |\n| other | n/a: why |\nprose `not-a-row`\n";
    let cases = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert!(checklist_problems(text, &cases(&["a", "b"])).is_empty());
    assert_eq!(
        checklist_problems(text, &cases(&["a", "c"])),
        ["named but not a case: b", "case not named: c"]
    );
    assert_eq!(
        checklist_problems("| x | `a`, `a` |", &cases(&["a"])),
        ["named twice: a"]
    );
}

// ---- S-0003: check --list ----

#[test]
fn check_list_runs_the_listed_cases() {
    let dir = tempfile::tempdir().unwrap();
    let cases = dir.path().join("cases");
    case(
        &cases,
        "one",
        "args = [\"print\", \"1\"]\n\n[expect]\nexit = 0\nstdout = \"1\\n\"\n",
    );
    case(&cases, "two", "args = [\"print\", \"2\"]\n");
    let c = cases.to_str().unwrap();
    let list = dir.path().join("list.txt");
    write(&list, "# a comment\n\none   # trailing comment\n");
    let l = list.to_str().unwrap();
    let out = fixtures(
        &["check", "--bin", stub(), "--cases", c, "--list", l],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stdout));
    assert_eq!(
        text(&out.stdout),
        "ok   one\n1 passed, 0 failed, 0 skipped\n"
    );

    write(&list, "one\nghost\n");
    let out = fixtures(
        &["check", "--bin", stub(), "--cases", c, "--list", l],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("ghost") && text(&out.stderr).contains("no such case"));

    write(&list, "# nothing\n");
    let out = fixtures(
        &["check", "--bin", stub(), "--cases", c, "--list", l],
        dir.path(),
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("lists no cases"));
}
