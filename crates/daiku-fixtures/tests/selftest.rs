//! Library-level self-tests for the fixture runner.

use std::path::Path;

use daiku_fixtures::{Os, load_cases};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

// ---- T3: case loading ----

#[test]
fn loads_cases_sorted_and_by_name() {
    let dir = tempfile::tempdir().unwrap();
    write(&dir.path().join("b/case.toml"), "args = [\"x\"]\n");
    write(
        &dir.path().join("a/case.toml"),
        "args = [\"y\", \"z\"]\nunordered_stdout = true\ngit = true\nskip_on = [\"windows\"]\nenv = { K = \"{ROOT}/v\" }\ndeviations = \"none\"\n",
    );
    std::fs::create_dir_all(dir.path().join("not-a-case")).unwrap();

    let all = load_cases(dir.path(), &[]).unwrap();
    let names: Vec<&str> = all.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    let a = &all[0].spec;
    assert_eq!(a.args, ["y", "z"]);
    assert!(a.unordered_stdout && a.git);
    assert_eq!(a.skip_on, [Os::Windows]);
    assert_eq!(a.env["K"], "{ROOT}/v");
    assert_eq!(a.deviations, "none");

    let b = &all[1].spec;
    assert!(!b.unordered_stdout && !b.git && b.skip_on.is_empty() && b.env.is_empty());

    let one = load_cases(dir.path(), &["b".to_string()]).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(one[0].name, "b");
}

#[test]
fn rejects_bad_cases() {
    let dir = tempfile::tempdir().unwrap();
    write(
        &dir.path().join("os/case.toml"),
        "args = []\nskip_on = [\"plan9\"]\n",
    );
    write(&dir.path().join("key/case.toml"), "args = []\nbogus = 1\n");
    write(&dir.path().join("noargs/case.toml"), "git = true\n");
    for name in ["os", "key", "noargs", "missing"] {
        let err = load_cases(dir.path(), &[name.to_string()]).unwrap_err();
        assert!(
            err.to_string().contains(name),
            "{name}: error should name the case: {err}"
        );
    }
}

// ---- helpers for running the stub ----

use daiku_fixtures::{Case, Observation, run_case, run_case_with_timeout};

fn stub() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_daiku-fixtures-stub"))
}

/// Builds a case folder `name` under `dir` with the given case.toml body.
fn case(dir: &Path, name: &str, toml: &str) -> Case {
    write(&dir.join(name).join("case.toml"), toml);
    Case::load(&dir.join(name)).unwrap()
}

/// TOML for a stub script: steps separated by `;`.
fn script(steps: &[&[&str]]) -> String {
    let mut args: Vec<String> = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            args.push(";".into());
        }
        args.extend(step.iter().map(|s| s.to_string()));
    }
    format!("args = {}\n", toml_list(&args))
}

fn toml_list(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|s| {
            let e = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r");
            format!("\"{e}\"")
        })
        .collect();
    format!("[{}]", quoted.join(", "))
}

fn run(c: &Case) -> Observation {
    run_case(stub(), c).unwrap()
}

// ---- T4: sandbox and run ----

#[test]
fn environment_is_sealed() {
    let dir = tempfile::tempdir().unwrap();
    let toml = script(&[&["env"], &["pwd"]]) + "env = { EXTRA = \"{ROOT}/x\" }\n";
    let obs = run(&case(dir.path(), "env", &toml));
    assert_eq!(obs.exit, 0, "{}", obs.stderr);

    let mut allowed = vec![
        "PATH",
        "HOME",
        "XDG_CONFIG_HOME",
        "TZ",
        "LANG",
        "LC_ALL",
        "EXTRA",
    ];
    if cfg!(windows) {
        allowed.extend(["USERPROFILE", "SystemRoot"]);
    }
    let lines: Vec<&str> = obs.stdout.lines().collect();
    let (vars, pwd) = lines.split_at(lines.len() - 1);
    for line in vars {
        let key = line.split('=').next().unwrap();
        assert!(allowed.contains(&key), "unexpected variable {line}");
    }
    assert!(vars.contains(&"HOME={HOME}"), "{vars:?}");
    assert!(vars.contains(&"XDG_CONFIG_HOME={CONFIG}"), "{vars:?}");
    assert!(vars.contains(&"TZ=UTC"));
    assert!(vars.contains(&"LANG=C") && vars.contains(&"LC_ALL=C"));
    assert!(vars.iter().any(|l| l.starts_with("PATH=")));
    assert!(
        vars.contains(&"EXTRA={ROOT}/x") || vars.contains(&"EXTRA={ROOT}\\x"),
        "{vars:?}"
    );
    assert_eq!(pwd, ["{ROOT}"]);
}

#[test]
fn before_is_copied_and_keep_markers_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(dir.path(), "copy", "args = [\"print\", \"x\"]\n");
    write(&c.dir.join("before/a.txt"), "a");
    write(&c.dir.join("before/empty/.fixture-keep"), "");
    let obs = run(&c);
    assert_eq!(obs.manifest, ["f a.txt", "d empty"]);
    assert_eq!(obs.files["a.txt"], b"a");
}

#[test]
fn args_expand_path_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let obs = run(&case(
        dir.path(),
        "expand",
        &script(&[&["write", "{ROOT}/out.txt", "{HOME}|{CONFIG}"]]),
    ));
    assert_eq!(obs.exit, 0, "{}", obs.stderr);
    assert_eq!(obs.files["out.txt"], b"{HOME}|{CONFIG}");
}

#[test]
fn slow_binary_times_out() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(dir.path(), "slow", &script(&[&["sleep", "5000"]]));
    let err = run_case_with_timeout(stub(), &c, std::time::Duration::from_millis(200)).unwrap_err();
    assert!(err.to_string().contains("timed out"), "{err}");
}

#[test]
fn exit_code_and_stderr_are_kept() {
    let dir = tempfile::tempdir().unwrap();
    let obs = run(&case(
        dir.path(),
        "exit",
        &script(&[&["eprint", "bad"], &["exit", "7"]]),
    ));
    assert_eq!(obs.exit, 7);
    assert_eq!(obs.stderr.replace("\r\n", "\n"), "bad\n");
}

// ---- T5: observation ----

#[test]
fn git_case_records_tree_and_log() {
    let dir = tempfile::tempdir().unwrap();
    let toml = script(&[
        &["write", "a.txt", "x"],
        &["commit", "one"],
        &["mkdir", "empty"],
        &["write", "b/c.txt", "y"],
        &["commit", "two"],
    ]) + "git = true\n";
    let obs = run(&case(dir.path(), "git", &toml));
    assert_eq!(obs.exit, 0, "{}", obs.stderr);
    assert_eq!(obs.manifest, ["f a.txt", "d b", "f b/c.txt", "d empty"]);
    let log = obs.git_log.unwrap();
    assert_eq!(log, "one\n\nA\ta.txt\ntwo\n\nA\tb/c.txt\n", "{log:?}");
    assert_eq!(obs.commits.len(), 3, "base plus two");
}

#[test]
fn non_git_case_has_no_log() {
    let dir = tempfile::tempdir().unwrap();
    let obs = run(&case(dir.path(), "nogit", &script(&[&["print", "x"]])));
    assert!(obs.git_log.is_none());
    assert!(obs.commits.is_empty());
}

// ---- T6: normalisation through a run ----

#[test]
fn run_output_normalises_to_tokens() {
    let dir = tempfile::tempdir().unwrap();
    let toml = script(&[
        &["now"],
        &["today"],
        &["pwd"],
        &["write", "note.txt", "{ROOT}"],
        &["commit", "x"],
    ]) + "git = true\n";
    let obs = run(&case(dir.path(), "norm", &toml));
    assert_eq!(obs.exit, 0, "{}", obs.stderr);
    assert_eq!(
        obs.stdout.replace("\r\n", "\n"),
        "{TIMESTAMP}\n{TODAY}\n{ROOT}\n{COMMIT}\n"
    );
    assert_eq!(obs.files["note.txt"], b"{ROOT}");
}

// ---- T7: compare and write ----

use daiku_fixtures::{compare, write_expect};

fn parts(c: &Case) -> Vec<(String, String)> {
    let obs = run(c);
    compare(c, &obs)
        .into_iter()
        .map(|d| (d.part, d.detail))
        .collect()
}

/// Captures `c`, then swaps its args for `args` (a stub script) and reloads it.
fn captured_then(c: &Case, args: &str) -> Case {
    write_expect(c, &run(c)).unwrap();
    let mut toml = std::fs::read_to_string(c.dir.join("case.toml")).unwrap();
    let first = toml.lines().next().unwrap().to_string();
    toml = toml.replacen(&first, args.trim_end(), 1);
    write(&c.dir.join("case.toml"), &toml);
    Case::load(&c.dir).unwrap()
}

#[test]
fn capture_then_check_passes() {
    let dir = tempfile::tempdir().unwrap();
    let toml = script(&[
        &["print", "hello"],
        &["now"],
        &["write", "a/b.json", "{\"k\": 1}"],
        &["mkdir", "empty"],
        &["write", "x.txt", "x"],
        &["commit", "c"],
    ]) + "git = true\n";
    let c = case(dir.path(), "cap", &toml);
    assert_eq!(
        parts(&c),
        [(
            "expect".to_string(),
            "no expect/ folder; run capture".to_string()
        )]
    );
    write_expect(&c, &run(&c)).unwrap();
    for f in [
        "exit",
        "stdout",
        "stderr",
        "tree.txt",
        "tree/a/b.json",
        "tree/x.txt",
        "git-log",
    ] {
        assert!(c.expect_dir().join(f).is_file(), "{f}");
    }
    assert_eq!(
        std::fs::read_to_string(c.expect_dir().join("stdout")).unwrap(),
        "hello\n{TIMESTAMP}\n{COMMIT}\n"
    );
    assert_eq!(parts(&c), []);
}

#[test]
fn capture_replaces_stale_files() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(dir.path(), "stale", &script(&[&["print", "x"]]));
    write(&c.expect_dir().join("tree/old.txt"), "old");
    write_expect(&c, &run(&c)).unwrap();
    assert!(!c.expect_dir().join("tree/old.txt").exists());
}

#[test]
fn stdout_difference_names_the_line() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(
        dir.path(),
        "out",
        &script(&[&["print", "same"], &["print", "abc"]]),
    );
    let c = captured_then(&c, &script(&[&["print", "same"], &["print", "abd"]]));
    let p = parts(&c);
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(p[0].0, "stdout");
    assert!(
        p[0].1.contains("-abc") && p[0].1.contains("+abd"),
        "{}",
        p[0].1
    );
}

#[test]
fn exit_difference() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(dir.path(), "exit", &script(&[&["exit", "0"]]));
    let c = captured_then(&c, &script(&[&["exit", "1"]]));
    assert_eq!(
        parts(&c),
        [("exit".to_string(), "expected 0, got 1".to_string())]
    );
}

#[test]
fn unordered_stdout_ignores_order() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(
        dir.path(),
        "unordered",
        &(script(&[&["print", "a"], &["print", "b"]]) + "unordered_stdout = true\n"),
    );
    let c = captured_then(&c, &script(&[&["print", "b"], &["print", "a"]]));
    assert_eq!(parts(&c), []);
    let c = captured_then(&c, &script(&[&["print", "b"], &["print", "c"]]));
    assert_eq!(parts(&c)[0].0, "stdout");
}

#[test]
fn json_compares_by_value() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(
        dir.path(),
        "json",
        &script(&[&["write", "d.json", "{\"a\":1,\"b\":[2]}"]]),
    );
    let c = captured_then(
        &c,
        &script(&[&["write", "d.json", "{ \"b\": [ 2 ],\n  \"a\": 1 }\n"]]),
    );
    assert_eq!(parts(&c), []);
    let c = captured_then(&c, &script(&[&["write", "d.json", "{\"a\":2,\"b\":[2]}"]]));
    let p = parts(&c);
    // captured_then recaptured the previous script, so this compares against {a:1}.
    assert_eq!(p.len(), 1, "{p:?}");
    assert_eq!(p[0].0, "tree/d.json");
}

#[test]
fn text_files_compare_with_lf_line_endings() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(
        dir.path(),
        "crlf",
        &script(&[&["write", "t.txt", "a\nb\n"]]),
    );
    let c = captured_then(&c, &script(&[&["write", "t.txt", "a\r\nb\r\n"]]));
    assert_eq!(parts(&c), []);
}

#[test]
fn extra_and_missing_tree_paths_are_named() {
    let dir = tempfile::tempdir().unwrap();
    let c = case(
        dir.path(),
        "tree",
        &script(&[&["write", "keep.txt", "k"], &["write", "gone.txt", "g"]]),
    );
    let c = captured_then(
        &c,
        &script(&[&["write", "keep.txt", "k"], &["write", "new.txt", "n"]]),
    );
    let mut p = parts(&c);
    p.sort();
    assert_eq!(
        p,
        [
            ("tree".to_string(), "extra: f new.txt".to_string()),
            ("tree".to_string(), "missing: f gone.txt".to_string()),
        ]
    );
}

#[test]
fn git_log_records_both_commits() {
    let dir = tempfile::tempdir().unwrap();
    let toml = script(&[
        &["write", "a.txt", "1"],
        &["commit", "first"],
        &["write", "a.txt", "2"],
        &["write", "b.txt", "b"],
        &["commit", "second"],
    ]) + "git = true\n";
    let c = case(dir.path(), "log", &toml);
    write_expect(&c, &run(&c)).unwrap();
    assert_eq!(
        std::fs::read_to_string(c.expect_dir().join("git-log")).unwrap(),
        "first\n\nA\ta.txt\nsecond\n\nM\ta.txt\nA\tb.txt\n"
    );
    assert_eq!(parts(&c), []);
}

// ---- T8: coverage ----

#[test]
fn a_case_covers_entries_its_args_start_with() {
    let dir = tempfile::tempdir().unwrap();
    let usage = "  daiku tickets check     x\n  daiku tickets new <t>\n  daiku gate <p>\n";
    let cases = [
        case(
            dir.path(),
            "a",
            "args = [\"tickets\", \"check\", \"--json\"]\n",
        ),
        case(
            dir.path(),
            "b",
            "args = [\"gate\", \"spec\", \"approved\"]\n",
        ),
    ];
    assert_eq!(daiku_fixtures::uncovered(usage, &cases), ["tickets new"]);
}

// ---- security finding 1: a case may not link outside itself ----

#[cfg(unix)]
mod symlinks {
    use super::*;
    use std::os::unix::fs::symlink;

    /// A folder outside the case holding a "secret" file.
    fn outside(dir: &Path) -> std::path::PathBuf {
        let o = dir.join("outside");
        write(&o.join("secret.txt"), "SECRET");
        o
    }

    fn load_err(dir: &Path, name: &str) -> String {
        load_cases(dir, &[name.to_string()])
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn symlinked_before_expect_case_toml_or_case_folder_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let cases = dir.path().join("cases");
        let o = outside(dir.path());

        write(&cases.join("b/case.toml"), "args = []\n");
        symlink(&o, cases.join("b/before")).unwrap();
        assert!(load_err(&cases, "b").contains("is a symlink"));

        write(&cases.join("e/case.toml"), "args = []\n");
        symlink(&o, cases.join("e/expect")).unwrap();
        assert!(load_err(&cases, "e").contains("is a symlink"));

        std::fs::create_dir_all(cases.join("t")).unwrap();
        symlink(o.join("secret.txt"), cases.join("t/case.toml")).unwrap();
        let err = load_err(&cases, "t");
        assert!(
            err.contains("is a symlink") && !err.contains("SECRET"),
            "{err}"
        );

        write(&o.join("case.toml"), "args = []\n");
        symlink(&o, cases.join("d")).unwrap();
        assert!(load_err(&cases, "d").contains("is a symlink"));
        // Listing every case refuses it too, rather than skipping it quietly.
        assert!(load_cases(&cases, &[]).is_err());
    }

    #[test]
    fn links_inside_before_must_stay_inside() {
        let dir = tempfile::tempdir().unwrap();
        let o = outside(dir.path());
        let c = case(dir.path(), "abs", "args = [\"print\", \"x\"]\n");
        std::fs::create_dir_all(c.dir.join("before")).unwrap();
        symlink(o.join("secret.txt"), c.dir.join("before/link")).unwrap();
        let err = run_case(stub(), &c).unwrap_err().to_string();
        assert!(err.contains("leaves before/"), "{err}");

        let c = case(dir.path(), "up", "args = [\"print\", \"x\"]\n");
        write(&c.dir.join("before/sub/a.txt"), "a");
        symlink("../../outside/secret.txt", c.dir.join("before/sub/link")).unwrap();
        let err = run_case(stub(), &c).unwrap_err().to_string();
        assert!(err.contains("leaves before/"), "{err}");

        let c = case(dir.path(), "ok", "args = [\"print\", \"x\"]\n");
        write(&c.dir.join("before/sub/a.txt"), "a");
        symlink("../top.txt", c.dir.join("before/sub/link")).unwrap();
        write(&c.dir.join("before/top.txt"), "t");
        let obs = run(&c);
        assert_eq!(
            obs.manifest,
            [
                "d sub",
                "f sub/a.txt",
                "l sub/link -> ../top.txt",
                "f top.txt"
            ]
        );
    }

    #[test]
    fn chained_links_that_climb_out_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let c = case(dir.path(), "chain", "args = [\"print\", \"x\"]\n");
        let before = c.dir.join("before");
        std::fs::create_dir_all(&before).unwrap();
        // Each target passes the text check; together they climb above the workspace.
        symlink(".", before.join("d")).unwrap();
        symlink("d/..", before.join("e")).unwrap();
        symlink("e/..", before.join("f")).unwrap();
        let err = run_case(stub(), &c).unwrap_err().to_string();
        assert!(err.contains("resolves outside before/"), "{err}");
        assert!(!c.expect_dir().exists());
    }

    #[test]
    fn dangling_and_looping_links_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let c = case(dir.path(), "dangling", "args = [\"print\", \"x\"]\n");
        std::fs::create_dir_all(c.dir.join("before")).unwrap();
        symlink("missing.txt", c.dir.join("before/link")).unwrap();
        let err = run_case(stub(), &c).unwrap_err().to_string();
        assert!(err.contains("can't be resolved"), "{err}");

        let c = case(dir.path(), "loop", "args = [\"print\", \"x\"]\n");
        std::fs::create_dir_all(c.dir.join("before")).unwrap();
        symlink("b", c.dir.join("before/a")).unwrap();
        symlink("a", c.dir.join("before/b")).unwrap();
        let err = run_case(stub(), &c).unwrap_err().to_string();
        assert!(err.contains("can't be resolved"), "{err}");
    }

    #[test]
    fn links_to_the_workspace_itself_are_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let c = case(dir.path(), "dot", "args = [\"print\", \"x\"]\n");
        write(&c.dir.join("before/sub/a.txt"), "a");
        symlink(".", c.dir.join("before/here")).unwrap();
        symlink("..", c.dir.join("before/sub/up")).unwrap();
        let obs = run(&c);
        assert_eq!(
            obs.manifest,
            ["l here -> .", "d sub", "f sub/a.txt", "l sub/up -> .."]
        );
    }

    #[test]
    fn symlinks_inside_expect_are_refused_without_reading_them() {
        let dir = tempfile::tempdir().unwrap();
        let o = outside(dir.path());
        let c = case(dir.path(), "x", &script(&[&["write", "a.txt", "a"]]));
        write_expect(&c, &run(&c)).unwrap();
        std::fs::remove_file(c.expect_dir().join("stdout")).unwrap();
        symlink(o.join("secret.txt"), c.expect_dir().join("stdout")).unwrap();
        let tree = c.expect_dir().join("tree");
        std::fs::remove_dir_all(&tree).unwrap();
        symlink(&o, &tree).unwrap();
        std::fs::copy(o.join("secret.txt"), o.join("a.txt")).unwrap();

        let diffs = compare(&c, &run(&c));
        let parts: Vec<&str> = diffs.iter().map(|d| d.part.as_str()).collect();
        assert_eq!(parts, ["stdout", "tree/a.txt"], "{diffs:?}");
        for d in &diffs {
            assert!(d.detail.contains("is a symlink"), "{d:?}");
            assert!(!d.detail.contains("SECRET"), "{d:?}");
        }
    }

    #[test]
    fn capture_over_a_symlinked_expect_is_refused_and_leaves_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let o = outside(dir.path());
        write(
            &dir.path().join("cases/c/case.toml"),
            "args = [\"print\", \"x\"]\n",
        );
        symlink(&o, dir.path().join("cases/c/expect")).unwrap();
        assert!(load_err(&dir.path().join("cases"), "c").contains("is a symlink"));
        assert_eq!(
            std::fs::read_to_string(o.join("secret.txt")).unwrap(),
            "SECRET"
        );
    }
}
