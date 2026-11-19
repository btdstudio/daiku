//! Self-tests for S-0002's runner additions: shared workspaces, `config/`, `tools`,
//! token expansion in starting files, and recipes.

use std::collections::BTreeMap;
use std::path::Path;

use daiku_fixtures::{Case, Observation, build_workspaces, load_cases, run_case};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn stub() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_daiku-fixtures-stub"))
}

fn quote(items: &[&str]) -> String {
    let q: Vec<String> = items
        .iter()
        .map(|s| {
            let e = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\t', "\\t");
            format!("\"{e}\"")
        })
        .collect();
    format!("[{}]", q.join(", "))
}

/// `args = [...]` for stub steps separated by `;`.
fn script(steps: &[&[&str]]) -> String {
    let mut args: Vec<&str> = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        if i > 0 {
            args.push(";");
        }
        args.extend_from_slice(step);
    }
    format!("args = {}\n", quote(&args))
}

/// A fixtures folder with `cases/` and `workspaces/`.
struct Fx {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
}

impl Fx {
    fn new() -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("cases")).unwrap();
        std::fs::create_dir_all(root.join("workspaces")).unwrap();
        Fx { _dir: dir, root }
    }
    fn cases(&self) -> std::path::PathBuf {
        self.root.join("cases")
    }
    fn ws(&self) -> std::path::PathBuf {
        self.root.join("workspaces")
    }
    fn case(&self, name: &str, toml: &str) -> std::path::PathBuf {
        let d = self.cases().join(name);
        write(&d.join("case.toml"), toml);
        d
    }
    fn load(&self, name: &str) -> Case {
        Case::load(&self.cases().join(name)).unwrap()
    }
    fn load_err(&self, name: &str) -> String {
        Case::load(&self.cases().join(name))
            .unwrap_err()
            .to_string()
    }
}

fn run(c: &Case) -> Observation {
    let obs = run_case(stub(), c).unwrap();
    assert_eq!(obs.exit, 0, "{}", obs.stderr);
    obs
}

// ---- T1: case options ----

#[test]
fn loads_workspace_and_tools() {
    let fx = Fx::new();
    write(&fx.ws().join("base/tree/a.txt"), "a");
    fx.case(
        "with",
        "args = []\nworkspace = \"base\"\ntools = [\"git\"]\n",
    );
    fx.case("without", "args = []\n");
    let all = load_cases(&fx.cases(), &[]).unwrap();
    assert_eq!(all[0].spec.workspace.as_deref(), Some("base"));
    assert_eq!(all[0].spec.tools, ["git"]);
    assert!(all[0].workspace_dir.as_ref().unwrap().ends_with("base"));
    assert!(all[1].spec.workspace.is_none() && all[1].spec.tools.is_empty());
    assert!(all[1].workspace_dir.is_none());
}

#[test]
fn rejects_missing_workspace_and_bad_names() {
    let fx = Fx::new();
    std::fs::create_dir_all(fx.ws().join("no-tree")).unwrap();
    fx.case("missing", "args = []\nworkspace = \"nope\"\n");
    fx.case("no-tree", "args = []\nworkspace = \"no-tree\"\n");
    for name in ["missing", "no-tree"] {
        let err = fx.load_err(name);
        assert!(err.contains(name) && err.contains("tree"), "{err}");
    }
    fx.case("dotdot", "args = []\nworkspace = \"..\"\n");
    fx.case("slash", "args = []\nworkspace = \"a/b\"\n");
    fx.case("tool", "args = []\ntools = [\"../bin/sh\"]\n");
    for name in ["dotdot", "slash", "tool"] {
        let err = fx.load_err(name);
        assert!(err.contains("plain name"), "{name}: {err}");
    }
    fx.case("cfg-file", "args = []\n");
    write(&fx.cases().join("cfg-file/config"), "not a folder");
    assert!(fx.load_err("cfg-file").contains("not a folder"));
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_config_and_workspace_tree() {
    let fx = Fx::new();
    let outside = tempfile::tempdir().unwrap();
    let d = fx.case("cfg-link", "args = []\n");
    std::os::unix::fs::symlink(outside.path(), d.join("config")).unwrap();
    assert!(fx.load_err("cfg-link").contains("symlink"));

    std::fs::create_dir_all(fx.ws().join("linked")).unwrap();
    std::os::unix::fs::symlink(outside.path(), fx.ws().join("linked/tree")).unwrap();
    fx.case("ws-link", "args = []\nworkspace = \"linked\"\n");
    assert!(fx.load_err("ws-link").contains("symlink"));
}

// ---- T2: layering, config/ and token expansion ----

#[test]
fn case_files_overlay_the_workspace() {
    let fx = Fx::new();
    write(&fx.ws().join("base/tree/keep.txt"), "from workspace");
    write(&fx.ws().join("base/tree/swap.txt"), "from workspace");
    write(&fx.ws().join("base/tree/empty/.fixture-keep"), "");
    let d = fx.case(
        "overlay",
        "args = [\"print\", \"x\"]\nworkspace = \"base\"\n",
    );
    write(&d.join("before/swap.txt"), "from case");
    write(&d.join("before/new/added.txt"), "added");
    let obs = run(&fx.load("overlay"));
    assert_eq!(
        obs.manifest,
        [
            "d empty",
            "f keep.txt",
            "d new",
            "f new/added.txt",
            "f swap.txt"
        ]
    );
    assert_eq!(obs.files["keep.txt"], b"from workspace");
    assert_eq!(obs.files["swap.txt"], b"from case");
}

#[test]
fn a_file_may_not_replace_a_folder() {
    let fx = Fx::new();
    write(&fx.ws().join("base/tree/thing/x.txt"), "x");
    let d = fx.case("clash", "args = []\nworkspace = \"base\"\n");
    write(&d.join("before/thing"), "a file");
    let err = run_case(stub(), &fx.load("clash")).unwrap_err().to_string();
    assert!(err.contains("thing") && err.contains("replace"), "{err}");
}

#[test]
fn config_reaches_the_global_config_folder() {
    let fx = Fx::new();
    write(
        &fx.ws().join("base/tree/.keep-me"),
        "workspace file in the root",
    );
    write(&fx.ws().join("base/config/daiku/from-ws.txt"), "ws\n");
    let d = fx.case(
        "cfg",
        &(script(&[
            &["cat", "{CONFIG}/daiku/settings.json"],
            &["cat", "{CONFIG}/daiku/from-ws.txt"],
        ]) + "workspace = \"base\"\n"),
    );
    write(
        &d.join("config/daiku/settings.json"),
        "{\"gates\": \"auto\"}\n",
    );
    let obs = run(&fx.load("cfg"));
    assert_eq!(obs.stdout, "{\"gates\": \"auto\"}\nws\n");
    // The config folder isn't part of the workspace tree.
    assert_eq!(obs.manifest, ["f .keep-me"]);
}

#[test]
fn tokens_in_starting_files_expand_and_normalise_back() {
    let fx = Fx::new();
    let d = fx.case("tok", &script(&[&["cat", "paths.txt"], &["pwd"]]));
    write(&d.join("before/paths.txt"), "{ROOT}|{HOME}|{CONFIG}\n");
    let bin: Vec<u8> = vec![0xff, 0xfe, b'{', b'R', b'O', b'O', b'T', b'}'];
    std::fs::write(d.join("before/blob.bin"), &bin).unwrap();
    let obs = run(&fx.load("tok"));
    assert_eq!(obs.stdout, "{ROOT}|{HOME}|{CONFIG}\n{ROOT}\n");
    assert_eq!(obs.files["paths.txt"], b"{ROOT}|{HOME}|{CONFIG}\n");
    assert_eq!(obs.files["blob.bin"], bin, "binary files are copied as is");
}

#[cfg(unix)]
#[test]
fn symlinks_may_not_escape_config_or_the_workspace() {
    let fx = Fx::new();
    let d = fx.case("cfg-escape", "args = []\n");
    std::fs::create_dir_all(d.join("config")).unwrap();
    std::os::unix::fs::symlink("../../x", d.join("config/out")).unwrap();
    let err = run_case(stub(), &fx.load("cfg-escape"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("leaves config/"), "{err}");

    std::fs::create_dir_all(fx.ws().join("w/tree")).unwrap();
    std::os::unix::fs::symlink("/etc/hosts", fx.ws().join("w/tree/hosts")).unwrap();
    fx.case("ws-escape", "args = []\nworkspace = \"w\"\n");
    let err = run_case(stub(), &fx.load("ws-escape"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("leaves workspaces/w/tree/"), "{err}");

    // A link in before/ may replace a workspace file, but not a folder.
    write(&fx.ws().join("r/tree/STATE.json"), "{}");
    write(&fx.ws().join("r/tree/real/STATE.json"), "{\"real\": true}");
    std::fs::create_dir_all(fx.ws().join("r/tree/dir")).unwrap();
    let d = fx.case(
        "link-over-file",
        "args = [\"cat\", \"STATE.json\"]\nworkspace = \"r\"\n",
    );
    std::fs::create_dir_all(d.join("before")).unwrap();
    std::os::unix::fs::symlink("real/STATE.json", d.join("before/STATE.json")).unwrap();
    let obs = run(&fx.load("link-over-file"));
    assert_eq!(obs.stdout, "{\"real\": true}");
    assert!(
        obs.manifest
            .contains(&"l STATE.json -> real/STATE.json".to_string())
    );
    let d = fx.case("link-over-dir", "args = []\nworkspace = \"r\"\n");
    std::os::unix::fs::symlink("real", d.join("before/dir")).unwrap_or_else(|_| {
        std::fs::create_dir_all(d.join("before")).unwrap();
        std::os::unix::fs::symlink("real", d.join("before/dir")).unwrap()
    });
    let err = run_case(stub(), &fx.load("link-over-dir"))
        .unwrap_err()
        .to_string();
    assert!(err.contains("replace a folder"), "{err}");

    // A link from before/ to a file the workspace provides stays inside.
    write(&fx.ws().join("v/tree/target.txt"), "t");
    let d = fx.case(
        "ws-link-ok",
        "args = [\"cat\", \"link.txt\"]\nworkspace = \"v\"\n",
    );
    std::fs::create_dir_all(d.join("before")).unwrap();
    std::os::unix::fs::symlink("target.txt", d.join("before/link.txt")).unwrap();
    assert_eq!(run(&fx.load("ws-link-ok")).stdout, "t");
}

// ---- T3: tools ----

#[cfg(unix)]
#[test]
fn tools_limit_path_to_one_folder() {
    let fx = Fx::new();
    fx.case(
        "tools",
        &(script(&[&["env"], &["write", "f.txt", "x"], &["commit", "second"]])
            + "git = true\ntools = [\"git\"]\n"),
    );
    let obs = run(&fx.load("tools"));
    let path = obs
        .stdout
        .lines()
        .find_map(|l| l.strip_prefix("PATH="))
        .unwrap();
    assert!(!path.contains(':'), "one folder: {path}");
    assert!(path.starts_with("{HOME}/"), "{path}");
    assert!(obs.stdout.contains("{COMMIT}"), "{}", obs.stdout);
    assert_eq!(obs.manifest, ["f f.txt"], "bin/ is outside the workspace");
    assert_eq!(obs.git_log.as_deref(), Some("second\n\nA\tf.txt\n"));
}

#[cfg(unix)]
#[test]
fn a_missing_tool_fails_the_run() {
    let fx = Fx::new();
    fx.case("gone", "args = []\ntools = [\"no-such-tool-xyz\"]\n");
    let err = run_case(stub(), &fx.load("gone")).unwrap_err().to_string();
    assert!(err.contains("no-such-tool-xyz"), "{err}");
}

#[cfg(windows)]
#[test]
fn tools_are_refused_on_windows() {
    let fx = Fx::new();
    fx.case("win", "args = []\ntools = [\"git\"]\n");
    let err = run_case(stub(), &fx.load("win")).unwrap_err().to_string();
    assert!(err.contains("isn't supported on Windows"), "{err}");
}

// ---- T6: recipes ----

fn recipe(fx: &Fx, name: &str, toml: &str) {
    write(&fx.ws().join(name).join("recipe.toml"), toml);
}

/// Every file under `dir`, by relative path.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&p).unwrap());
            }
        }
    }
    out
}

fn two_level(fx: &Fx) {
    recipe(
        fx,
        "parent",
        r#"steps = [["write", "a.txt", "parent at {ROOT}"], ["mkdir", "empty"]]
[[files]]
path = "config/daiku/settings.json"
text = '{"gates": "prompt"}'
"#,
    );
    recipe(
        fx,
        "child",
        r#"from = "parent"
steps = [["stamp", "stamp.txt"], ["write", "b.txt", "child"]]
"#,
    );
}

#[test]
fn recipes_build_parents_first_with_fixed_values() {
    let fx = Fx::new();
    two_level(&fx);
    let built = build_workspaces(stub(), &fx.ws(), &[]).unwrap();
    assert_eq!(built, ["parent", "child"]);

    let child = snapshot(&fx.ws().join("child/tree"));
    let names: Vec<&str> = child.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        ["a.txt", "b.txt", "empty/.fixture-keep", "stamp.txt"]
    );
    assert_eq!(child["a.txt"], b"parent at {ROOT}");
    assert_eq!(
        String::from_utf8_lossy(&child["stamp.txt"]),
        "date 2026-01-15\nat 2026-01-15T00:00:00.000Z\n"
    );
    let cfg = snapshot(&fx.ws().join("child/config"));
    assert_eq!(cfg["daiku/settings.json"], br#"{"gates": "prompt"}"#);

    // A case built on it sees the expanded paths and the config.
    fx.case(
        "use",
        &(script(&[&["cat", "a.txt"], &["cat", "{CONFIG}/daiku/settings.json"]])
            + "workspace = \"child\"\n"),
    );
    let obs = run(&fx.load("use"));
    assert_eq!(obs.stdout, "parent at {ROOT}{\"gates\": \"prompt\"}");
}

#[test]
fn rebuilding_gives_identical_bytes() {
    let fx = Fx::new();
    two_level(&fx);
    build_workspaces(stub(), &fx.ws(), &[]).unwrap();
    let first = snapshot(&fx.ws());
    build_workspaces(stub(), &fx.ws(), &[]).unwrap();
    assert_eq!(snapshot(&fx.ws()), first);
}

#[test]
fn named_builds_add_only_missing_parents() {
    let fx = Fx::new();
    two_level(&fx);
    assert_eq!(
        build_workspaces(stub(), &fx.ws(), &["child".into()]).unwrap(),
        ["parent", "child"]
    );
    assert_eq!(
        build_workspaces(stub(), &fx.ws(), &["child".into()]).unwrap(),
        ["child"]
    );
}

#[test]
fn a_failing_step_names_itself() {
    let fx = Fx::new();
    recipe(
        &fx,
        "bad",
        r#"steps = [["print", "ok"], ["eprint", "boom", ";", "exit", "3"]]"#,
    );
    let err = build_workspaces(stub(), &fx.ws(), &[])
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("step 2") && err.contains("exited 3") && err.contains("boom"),
        "{err}"
    );
}

#[test]
fn recipe_loops_and_unknown_parents_are_refused() {
    let fx = Fx::new();
    recipe(&fx, "a", "from = \"b\"\n");
    recipe(&fx, "b", "from = \"a\"\n");
    let err = build_workspaces(stub(), &fx.ws(), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("loop"), "{err}");

    let fx = Fx::new();
    recipe(&fx, "orphan", "from = \"ghost\"\n");
    let err = build_workspaces(stub(), &fx.ws(), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("ghost"), "{err}");

    let fx = Fx::new();
    recipe(&fx, "esc", "[[files]]\npath = \"../x\"\ntext = \"\"\n");
    let err = build_workspaces(stub(), &fx.ws(), &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("inside the workspace"), "{err}");
}

#[test]
fn git_recipes_drop_the_repo() {
    let fx = Fx::new();
    recipe(
        &fx,
        "g",
        "git = true\nsteps = [[\"write\", \"f.txt\", \"x\", \";\", \"commit\", \"one\"]]\n",
    );
    build_workspaces(stub(), &fx.ws(), &[]).unwrap();
    let tree = snapshot(&fx.ws().join("g/tree"));
    assert_eq!(tree.keys().collect::<Vec<_>>(), ["f.txt"]);
}

// ---- stage/: staged changes for git cases ----

#[test]
fn stage_files_are_staged_after_the_base_commit() {
    let fx = Fx::new();
    let d = fx.case("staged", "args = [\"staged\"]\ngit = true\n");
    write(&d.join("before/kept.txt"), "base");
    write(&d.join("stage/kept.txt"), "changed in {ROOT}");
    write(&d.join("stage/new/added.txt"), "new");
    let obs = run(&fx.load("staged"));
    assert_eq!(obs.stdout, "kept.txt\nnew/added.txt\n");
    assert_eq!(obs.files["kept.txt"], b"changed in {ROOT}");
    assert_eq!(obs.git_log.as_deref(), Some(""), "stage/ makes no commit");
}

#[test]
fn stage_needs_a_git_case() {
    let fx = Fx::new();
    let d = fx.case("no-git", "args = []\n");
    write(&d.join("stage/a.txt"), "a");
    assert!(fx.load_err("no-git").contains("needs git = true"));
}

// ---- security finding 1: case data may not make git run commands ----

/// A marker path outside every sandbox, and whether a command wrote it.
#[cfg(unix)]
fn marker() -> (tempfile::TempDir, std::path::PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let m = d.path().join("ran");
    (d, m)
}

#[test]
fn config_may_not_hold_git_config() {
    let fx = Fx::new();
    for (name, folder) in [("lower", "git"), ("upper", "Git")] {
        let d = fx.case(name, "args = []\ngit = true\n");
        write(&d.join("config").join(folder).join("config"), "[core]\n");
        let err = fx.load_err(name);
        assert!(err.contains("may not hold git's config"), "{name}: {err}");
    }
    write(&fx.ws().join("w/tree/a.txt"), "a");
    write(&fx.ws().join("w/config/git/config"), "[core]\n");
    fx.case("ws", "args = []\nworkspace = \"w\"\n");
    assert!(fx.load_err("ws").contains("may not hold git's config"));
}

#[test]
fn recipes_may_not_write_git_config_or_a_repo() {
    for path in [
        "config/git/config",
        "config/GIT/attributes",
        ".git",
        "sub/.GIT",
    ] {
        let fx = Fx::new();
        recipe(
            &fx,
            "r",
            &format!("[[files]]\npath = \"{path}\"\ntext = \"x\"\n"),
        );
        let err = build_workspaces(stub(), &fx.ws(), &[])
            .unwrap_err()
            .to_string();
        assert!(err.contains("git's config or repo"), "{path}: {err}");
    }
}

#[test]
fn starting_files_may_not_be_named_dot_git() {
    let fx = Fx::new();
    for (name, path) in [("top", "before/.git"), ("nested", "before/sub/.GIT")] {
        let d = fx.case(name, "args = []\n");
        write(&d.join(path), "gitdir: /elsewhere\n");
        let err = run_case(stub(), &fx.load(name)).unwrap_err().to_string();
        assert!(err.contains("named .git"), "{name}: {err}");
    }
}

#[test]
fn env_may_not_redirect_processes() {
    let fx = Fx::new();
    for key in [
        "PATH",
        "Path",
        "HOME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "GIT_CONFIG_COUNT",
        "git_dir",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "TZ",
    ] {
        fx.case(key, &format!("args = []\nenv = {{ {key} = \"x\" }}\n"));
        let err = fx.load_err(key);
        assert!(err.contains("can't be set by a case"), "{key}: {err}");
    }
    fx.case("ok", "args = []\nenv = { DAIKU_X = \"{ROOT}\" }\n");
    fx.load("ok");
}

/// The global and system git config are empty for every process in a run, so a
/// global config file the binary (or anything) writes into {HOME} or {CONFIG} is
/// ignored by the runner's own git too.
#[cfg(unix)]
#[test]
fn git_ignores_global_config_from_home_and_config() {
    let (_d, ran) = marker();
    let cfg = format!(
        "[filter \"x\"]\n\tclean = sh -c 'echo ran > {}'; cat\n",
        ran.display()
    );
    let fx = Fx::new();
    // The stub writes the would-be global configs, then commits through git with a
    // .gitattributes that asks for the filter.
    fx.case(
        "global",
        &(script(&[
            &["write", "{HOME}/.gitconfig", &cfg],
            &["write", "{CONFIG}/git/config", &cfg],
            &["write", ".gitattributes", "* filter=x\n"],
            &["write", "f.txt", "hi\n"],
            &["commit", "add"],
        ]) + "git = true\n"),
    );
    let obs = run(&fx.load("global"));
    assert!(!ran.exists(), "a filter from global config ran");
    assert!(obs.git_log.unwrap().contains("f.txt"));
}

// ---- D16: expect/ records only what the run changed ----

#[test]
fn expect_records_only_changes_and_stays_strict() {
    let fx = Fx::new();
    write(&fx.ws().join("w/tree/same.txt"), "same");
    write(&fx.ws().join("w/tree/edit.txt"), "old");
    write(&fx.ws().join("w/tree/kept/.fixture-keep"), "");
    let toml = |steps: &[&[&str]]| script(steps) + "workspace = \"w\"\n";
    // Change edit.txt, rewrite same.txt with the same bytes, add new/n.txt. Removals
    // and changes of kind are covered by the unit tests in observe.rs.
    fx.case(
        "c",
        &toml(&[
            &["write", "edit.txt", "new"],
            &["write", "same.txt", "same"],
            &["write", "new/n.txt", "n"],
        ]),
    );
    let c = fx.load("c");
    let obs = run(&c);
    assert_eq!(obs.changes, ["~ f edit.txt", "+ d new", "+ f new/n.txt"]);
    daiku_fixtures::write_expect(&c, &obs).unwrap();
    let c = fx.load("c");
    let stored = snapshot(&c.expect_dir());
    assert_eq!(stored.keys().collect::<Vec<_>>(), ["edit.txt", "new/n.txt"]);
    assert!(daiku_fixtures::compare(&c, &run(&c)).is_empty());

    // Touching a file the expectation says is unchanged fails the case: keep the
    // captured [expect], change only the args.
    let file = c.dir.join("case.toml");
    let text = std::fs::read_to_string(&file).unwrap();
    let args = script(&[
        &["write", "edit.txt", "new"],
        &["write", "same.txt", "different"],
        &["write", "new/n.txt", "n"],
    ]);
    let first = text.lines().next().unwrap().to_string();
    std::fs::write(&file, text.replacen(&first, args.trim_end(), 1)).unwrap();
    let diffs = daiku_fixtures::compare(&fx.load("c"), &run(&fx.load("c")));
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    assert_eq!(diffs[0].part, "changes");
    assert!(
        diffs[0].detail.contains("\n+~ f same.txt\n"),
        "{}",
        diffs[0].detail
    );
}

// ---- stored names for git files ----

#[test]
fn git_files_are_stored_under_another_name() {
    let fx = Fx::new();
    // A raw .gitignore in fixture data is refused.
    let d = fx.case("raw", "args = []\n");
    write(&d.join("before/.gitignore"), "x\n");
    let err = run_case(stub(), &fx.load("raw")).unwrap_err().to_string();
    assert!(err.contains("as .gitignore.fixture"), "{err}");

    // The stored name arrives under the real name, and a .gitignore the run writes
    // is stored under the other name in expect/.
    let d = fx.case(
        "named",
        &script(&[
            &["cat", ".gitignore"],
            &["write", "sub/.gitattributes", "a\n"],
        ]),
    );
    write(&d.join("before/.gitignore.fixture"), "ignored\n");
    let c = fx.load("named");
    let obs = run(&c);
    assert_eq!(obs.stdout, "ignored\n");
    assert!(obs.manifest.contains(&"f .gitignore".to_string()));
    assert_eq!(obs.changes, ["+ d sub", "+ f sub/.gitattributes"]);
    daiku_fixtures::write_expect(&c, &obs).unwrap();
    let c = fx.load("named");
    let stored = snapshot(&c.expect_dir());
    assert_eq!(
        stored.keys().collect::<Vec<_>>(),
        ["sub/.gitattributes.fixture"]
    );
    assert!(daiku_fixtures::compare(&c, &run(&c)).is_empty());

    // Workspaces store them the same way.
    recipe(&fx, "g", "steps = [[\"write\", \".gitignore\", \"x\"]]\n");
    build_workspaces(stub(), &fx.ws(), &[]).unwrap();
    let tree = snapshot(&fx.ws().join("g/tree"));
    assert_eq!(tree.keys().collect::<Vec<_>>(), [".gitignore.fixture"]);
}
