//! Scaffold engine tests: Go's scaffold_test.go, update_test.go, rooms_test.go,
//! vcs_test.go, symlink_test.go and golden_test.go, by behaviour, plus S-0004's ACs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;
use crate::env::{Dirs, Output};
use crate::fake::ScriptedProcess;
use crate::{Env, Workspace};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Temp {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "daiku-scaffold-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&p).unwrap();
        Temp(p)
    }
    fn p(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.p(rel)).unwrap()
    }
    fn write(&self, rel: &str, text: &str) {
        let p = self.p(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A workspace with a fake Env whose global config folder is empty; `git` replies
/// succeed.
fn ws(root: &Path, config: &Temp) -> Workspace {
    let mut env = Env::fake(Dirs {
        home: None,
        xdg_config_home: Some(config.0.clone().into_os_string()),
        cwd: None,
    });
    let process = ScriptedProcess::default();
    for _ in 0..4 {
        process.reply(Output {
            code: Some(0),
            ..Output::default()
        });
    }
    env.process = Box::new(process);
    Workspace::open(root, env)
}

fn opts(no_git: bool, force: bool) -> InitOptions {
    InitOptions {
        no_git,
        force,
        vcs: if no_git { "none".into() } else { "git".into() },
        version: "dev".into(),
    }
}

fn golden() -> BTreeMap<String, String> {
    let v: serde_json::Value =
        serde_json::from_str(include_str!("../../tests/data/golden-manifest.json")).unwrap();
    v["files"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
        .collect()
}

fn manifest(t: &Temp) -> Manifest {
    serde_json::from_str(&t.read(".daiku/manifest.json")).unwrap()
}

// ---- the embedded scaffold ----

/// AC4: every embedded file a git workspace with both rooms holds hashes as in Go's
/// golden manifest, so the bytes survived the move and the checkout.
#[test]
fn embedded_files_match_the_golden_manifest() {
    let mut got = BTreeMap::new();
    for (t, prefix) in [("root", ""), ("build", "build"), ("plan", "plan")] {
        for (rel, data) in engine::walk_tree(t, prefix, false) {
            got.insert(rel, engine::hash(data));
        }
    }
    assert_eq!(got, golden());
}

/// AC3: a git workspace with both rooms builds Go's golden manifest.
#[test]
fn workspace_manifest_matches_go() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(false, false)).unwrap();
    w.ensure_room("plan", "dev").unwrap();
    w.ensure_room("build", "dev").unwrap();
    let m = manifest(&t);
    assert_eq!(m.version, "dev");
    assert_eq!(m.files, golden());
}

#[test]
fn is_git_file_matches_go() {
    for (name, want) in [
        (".gitignore", true),
        (".gitattributes", true),
        (".gitkeep", true),
        ("gitignore", false),
        (".git", false),
    ] {
        assert_eq!(engine::is_git_file(name), want, "{name}");
    }
}

// ---- init ----

#[test]
fn init_writes_the_root_tree_seeds_manifest_and_agents() {
    let (t, c) = (Temp::new(), Temp::new());
    let rep = ws(&t.0, &c).init(&opts(false, false)).unwrap().value;
    assert!(rep.git_init, "git init runs when there's no .git");
    assert_eq!(
        rep.written.first().map(String::as_str),
        Some(".gitattributes")
    );
    assert_eq!(
        &rep.written[rep.written.len() - 5..],
        [
            ".daiku/settings.json",
            "STATE.json",
            "LOG.md",
            "REPOS.json",
            "AGENTS.md"
        ]
    );
    for f in [
        "CLAUDE.md",
        "sync.md",
        "STATE.json",
        "REPOS.json",
        ".daiku/settings.json",
        ".daiku/manifest.json",
        "AGENTS.md",
    ] {
        assert!(t.p(f).is_file(), "{f}");
    }
    assert!(
        !t.p("plan").exists() && !t.p("build").exists(),
        "init writes only the root"
    );
    assert_eq!(
        t.read("STATE.json"),
        "{\n  \"changes\": {},\n  \"schema\": 1\n}\n"
    );
    assert_eq!(t.read("REPOS.json"), "{\n  \"repos\": {}\n}\n");
    assert_eq!(t.read("LOG.md"), "");
    let settings: serde_json::Value =
        serde_json::from_str(&t.read(".daiku/settings.json")).unwrap();
    assert_eq!(settings["gates"], "block");
    assert!(settings.get("vcs").is_none());
    assert_eq!(t.read("AGENTS.md"), agents::assemble(&[]));
}

#[test]
fn init_skips_existing_files_unless_forced() {
    let (t, c) = (Temp::new(), Temp::new());
    t.write("CLAUDE.md", "mine\n");
    std::fs::create_dir_all(t.p(".git")).unwrap();
    let rep = ws(&t.0, &c).init(&opts(false, false)).unwrap().value;
    assert!(!rep.git_init, "an existing .git means no git init");
    assert!(!rep.written.contains(&"CLAUDE.md".to_string()));
    assert_eq!(t.read("CLAUDE.md"), "mine\n");
    let again = ws(&t.0, &c).init(&opts(false, false)).unwrap().value;
    assert!(again.written.is_empty(), "{:?}", again.written);
    let forced = ws(&t.0, &c).init(&opts(false, true)).unwrap().value;
    assert!(forced.written.contains(&"CLAUDE.md".to_string()));
    assert_ne!(t.read("CLAUDE.md"), "mine\n");
    assert!(
        !forced.written.contains(&"STATE.json".to_string()),
        "seeds are never overwritten"
    );
}

#[test]
fn local_init_omits_git_files_but_keeps_gitignore_and_folders() {
    let (t, c) = (Temp::new(), Temp::new());
    let rep = ws(&t.0, &c).init(&opts(true, false)).unwrap().value;
    assert!(!rep.git_init);
    assert!(t.p(".gitignore").is_file() && !t.p(".gitattributes").exists());
    let settings: serde_json::Value =
        serde_json::from_str(&t.read(".daiku/settings.json")).unwrap();
    assert_eq!(settings["vcs"], "none");
    assert!(!manifest(&t).files.contains_key(".gitattributes"));
    assert!(!rep.written.contains(&".gitattributes".to_string()));
}

#[test]
fn local_init_keeps_other_settings_and_reports_only_a_created_file() {
    let (t, c) = (Temp::new(), Temp::new());
    t.write(
        ".daiku/settings.json",
        "{\"gates\": \"auto\", \"extra\": [1, 2]}",
    );
    let rep = ws(&t.0, &c).init(&opts(true, false)).unwrap().value;
    assert!(!rep.written.contains(&".daiku/settings.json".to_string()));
    let s: serde_json::Value = serde_json::from_str(&t.read(".daiku/settings.json")).unwrap();
    assert_eq!(
        (
            s["gates"].as_str(),
            s["vcs"].as_str(),
            s["extra"][1].as_i64()
        ),
        (Some("auto"), Some("none"), Some(2))
    );
}

#[test]
fn local_init_refuses_bad_settings_with_go_messages() {
    for (text, want) in [
        ("[]", "daiku init: settings must be a JSON object: SETTINGS"),
        (
            "{} x",
            "daiku init: cannot parse settings SETTINGS: trailing data after the object",
        ),
        (
            "{ not",
            "daiku init: cannot parse settings SETTINGS: invalid character 'n' looking for beginning of object key string",
        ),
        (
            "{",
            "daiku init: cannot parse settings SETTINGS: unexpected EOF",
        ),
        ("", "daiku init: cannot parse settings SETTINGS: EOF"),
    ] {
        let (t, c) = (Temp::new(), Temp::new());
        t.write(".daiku/settings.json", text);
        let err = ws(&t.0, &c)
            .init(&opts(true, false))
            .unwrap_err()
            .to_string();
        let file = engine::shown(&t.0, ".daiku/settings.json");
        assert_eq!(err, want.replace("SETTINGS", &file), "{text:?}");
    }
}

#[test]
fn init_vcs_reads_the_global_setting() {
    let c = Temp::new();
    let dirs = Dirs {
        home: None,
        xdg_config_home: Some(c.0.clone().into_os_string()),
        cwd: None,
    };
    assert_eq!(init_vcs(true, &dirs).unwrap(), "none");
    assert_eq!(init_vcs(false, &dirs).unwrap(), "git");
    c.write("daiku/settings.json", "{\"vcs\": \"none\"}");
    assert_eq!(init_vcs(false, &dirs).unwrap(), "none");
}

#[test]
fn git_init_failure_reads_like_go() {
    let (t, c) = (Temp::new(), Temp::new());
    let mut w = ws(&t.0, &c);
    let p = ScriptedProcess::default();
    p.reply(Output {
        code: Some(128),
        stdout: Vec::new(),
        stderr: b"fatal: nope\n".to_vec(),
    });
    let mut env = Env::fake(Dirs::default());
    env.process = Box::new(p);
    w = Workspace::open(w.root(), env);
    assert_eq!(
        w.init(&opts(false, false)).unwrap_err().to_string(),
        "git init: exit status 128: fatal: nope\n"
    );
}

/// `daiku init ""` fails as Go's does (found by the differential test).
#[test]
fn init_into_an_empty_path_fails_like_go() {
    let c = Temp::new();
    let err = ws(Path::new(""), &c)
        .init(&opts(false, false))
        .unwrap_err()
        .to_string();
    assert_eq!(err, "mkdir : no such file or directory");
}

#[test]
fn legacy_workspaces_are_refused() {
    for tool in ["zumen", "tomo"] {
        let (t, c) = (Temp::new(), Temp::new());
        std::fs::create_dir_all(t.p(&format!(".{tool}"))).unwrap();
        let want = format!(
            "daiku: this is a {tool} workspace (found .{tool}/) — use {tool}, or start a new daiku workspace"
        );
        assert_eq!(
            ws(&t.0, &c)
                .init(&opts(true, false))
                .unwrap_err()
                .to_string(),
            want
        );
        assert_eq!(ws(&t.0, &c).update("dev").unwrap_err().to_string(), want);
    }
}

#[cfg(unix)]
#[test]
fn writes_refuse_symlinked_folders_but_not_a_linked_root() {
    let (real, holder, c, outside) = (Temp::new(), Temp::new(), Temp::new(), Temp::new());
    let root = holder.p("ws");
    std::os::unix::fs::symlink(&real.0, &root).unwrap();
    ws(&root, &c).init(&opts(true, false)).unwrap();
    assert!(
        real.p("CLAUDE.md").is_file(),
        "a linked root is written through, as in Go"
    );

    let t = Temp::new();
    std::os::unix::fs::symlink(&outside.0, t.p("ideas")).unwrap();
    let err = ws(&t.0, &c)
        .init(&opts(true, false))
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("daiku: won't write ideas/context.md: ")
            && err.ends_with("ideas is a symlink, which could lead outside the workspace"),
        "{err}"
    );
    assert_eq!(std::fs::read_dir(&outside.0).unwrap().count(), 0);
}

// ---- update ----

#[test]
fn update_skips_edited_restores_missing_and_replaces_unedited() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    t.write("sync.md", "my edit\n");
    std::fs::remove_file(t.p("CLAUDE.md")).unwrap();
    // An unedited file from an older version: on disk and in the old manifest both.
    t.write("README.md", "old version\n");
    let mut m = manifest(&t);
    m.files
        .insert("README.md".into(), engine::hash(b"old version\n"));
    t.write(".daiku/manifest.json", &serde_json::to_string(&m).unwrap());
    let rep = w.update("dev").unwrap().value;
    assert_eq!(rep.skipped, ["sync.md"]);
    assert_eq!(rep.updated, ["CLAUDE.md", "README.md"]);
    assert!(rep.unchanged.contains(&"AGENTS.md".to_string()));
    assert_eq!(t.read("sync.md"), "my edit\n");
    assert_ne!(t.read("README.md"), "old version\n");
}

/// AC5: every group comes out sorted.
#[test]
fn update_report_groups_are_sorted() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    for f in ["sync.md", "ideas/context.md", "README.md", "CLAUDE.md"] {
        t.write(f, "edited\n");
    }
    for f in [
        "references/context.md",
        "ideas/templates/summary.md",
        ".gitignore",
    ] {
        std::fs::remove_file(t.p(f)).unwrap();
    }
    t.write("AGENTS.md", "# Mine\n\n## Team notes\n\nx\n");
    let rep = w.update("dev").unwrap().value;
    for list in [&rep.updated, &rep.skipped, &rep.unchanged] {
        let mut sorted = list.clone();
        sorted.sort();
        assert_eq!(list, &sorted);
    }
    assert_eq!(
        rep.skipped,
        ["CLAUDE.md", "README.md", "ideas/context.md", "sync.md"]
    );
    assert_eq!(
        rep.updated,
        [
            ".gitignore",
            "ideas/templates/summary.md",
            "references/context.md"
        ]
    );
    assert_eq!(rep.merged, ["AGENTS.md"]);
}

#[test]
fn update_refuses_a_non_workspace_and_a_corrupt_manifest() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = Workspace::open(
        ".",
        Env::fake(Dirs {
            home: None,
            xdg_config_home: Some(c.0.clone().into_os_string()),
            cwd: None,
        }),
    );
    drop(w);
    let w = ws(&t.0, &c);
    assert_eq!(
        w.update("dev").unwrap_err().to_string(),
        format!(
            "daiku update: {} is not a daiku workspace (run `daiku init`)",
            crate::gofmt::to_slash(&t.0.to_string_lossy())
        )
    );
    w.init(&opts(true, false)).unwrap();
    t.write(".daiku/manifest.json", "{ not json\n");
    assert_eq!(
        w.update("dev").unwrap_err().to_string(),
        "daiku update: invalid character 'n' looking for beginning of object key string"
    );
}

#[test]
fn update_rebuilds_a_missing_manifest_without_clobbering() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    t.write("sync.md", "mine\n");
    std::fs::remove_file(t.p(".daiku/manifest.json")).unwrap();
    let rep = w.update("dev").unwrap().value;
    assert_eq!(rep.skipped, ["sync.md"]);
    assert_eq!(t.read("sync.md"), "mine\n");
    assert!(t.p(".daiku/manifest.json").is_file());
}

/// AC6 through update: an edited Tech stack and an owner's section survive.
#[test]
fn update_merges_agents_keeping_owner_sections() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    let text = t.read("AGENTS.md").replace(
        "## Tech stack & standards\n",
        "## Tech stack & standards\n\n- Rust 1.100\n",
    ) + "\n## Team notes\n\nkeep me\n";
    let first = text.find("\n## ").unwrap();
    let stale = format!(
        "{}\n## {}",
        &text[..first],
        text[first + 4..].replacen('\n', "\nstale line\n", 1)
    );
    t.write("AGENTS.md", &stale);
    let rep = w.update("dev").unwrap().value;
    assert_eq!(rep.merged, ["AGENTS.md"]);
    let got = t.read("AGENTS.md");
    assert!(
        got.contains("- Rust 1.100") && got.contains("keep me") && !got.contains("stale line"),
        "{got}"
    );
}

/// `text` without the `## <heading>` section: its heading through the line before
/// the next `## `.
fn without_section(text: &str, heading: &str) -> String {
    let start = text.find(&format!("\n## {heading}\n")).unwrap();
    let end = start + 1 + text[start + 1..].find("\n## ").unwrap();
    format!("{}{}", &text[..start], &text[end..])
}

fn heading_order(text: &str) -> Vec<&str> {
    text.lines().filter_map(|l| l.strip_prefix("## ")).collect()
}

/// `CLAUDE.md` as v0.5.0 wrote it: a pointer, not an import.
const OLD_CLAUDE_MD: &str = "# CLAUDE.md

The root map for this workspace lives in [AGENTS.md](./AGENTS.md). Read that
first \u{2014} it is the single source of truth for routing, conventions, and rules.
This file is only a pointer so every harness (Claude Code, OpenCode, etc.)
resolves to the same map.
";

/// N-0016 AC1: `init` writes `CLAUDE.md` as the import alone.
#[test]
fn init_writes_claude_md_as_an_import() {
    let (t, c) = (Temp::new(), Temp::new());
    ws(&t.0, &c).init(&opts(true, false)).unwrap();
    assert_eq!(t.read("CLAUDE.md"), "@AGENTS.md\n");
}

/// N-0016 AC5, AC6: `update` replaces an unedited v0.5.0 `CLAUDE.md` with the import,
/// and skips one the user edited.
#[test]
fn update_replaces_the_old_claude_md_pointer() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    t.write("CLAUDE.md", OLD_CLAUDE_MD);
    let mut m = manifest(&t);
    m.files
        .insert("CLAUDE.md".into(), engine::hash(OLD_CLAUDE_MD.as_bytes()));
    t.write(".daiku/manifest.json", &serde_json::to_string(&m).unwrap());
    let rep = w.update("dev").unwrap().value;
    assert_eq!(rep.updated, ["CLAUDE.md"]);
    assert_eq!(t.read("CLAUDE.md"), "@AGENTS.md\n");

    let mine = format!("{OLD_CLAUDE_MD}\nMy notes.\n");
    t.write("CLAUDE.md", &mine);
    let m = manifest(&t);
    t.write(".daiku/manifest.json", &serde_json::to_string(&m).unwrap());
    let rep = w.update("dev").unwrap().value;
    assert_eq!(rep.skipped, ["CLAUDE.md"]);
    assert_eq!(t.read("CLAUDE.md"), mine);
}

/// N-0010 AC3–AC5 through update: a missing `## Model routing` is added after Tech
/// stack and then left alone; an edited one is kept; a missing Tech stack comes back
/// too.
#[test]
fn update_adds_model_routing_and_keeps_an_edited_one() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    let fresh = t.read("AGENTS.md");

    // AC4: an old file, without the section, gains it; a second update does nothing.
    let old = without_section(&fresh, "Model routing").replace(
        "- <FILL IN: the target",
        "- Rust 1.100\n- <FILL IN: the target",
    );
    t.write("AGENTS.md", &old);
    assert_eq!(w.update("dev").unwrap().value.merged, ["AGENTS.md"]);
    let got = t.read("AGENTS.md");
    assert_eq!(
        got,
        fresh.replace(
            "- <FILL IN: the target",
            "- Rust 1.100\n- <FILL IN: the target"
        )
    );
    let rep = w.update("dev").unwrap().value;
    assert!(
        rep.merged.is_empty() && rep.unchanged.contains(&"AGENTS.md".to_string()),
        "{rep:?}"
    );

    // AC3: an edited section is kept byte for byte.
    let mine = got.replace("| `deep` | judgment", "We use opus.\n\n| `deep` | judgment");
    t.write("AGENTS.md", &mine);
    let rep = w.update("dev").unwrap().value;
    assert!(rep.merged.is_empty(), "{rep:?}");
    assert_eq!(t.read("AGENTS.md"), mine);

    // AC5: with Tech stack and Model routing both gone, both come back, in order.
    let bare = without_section(
        &without_section(&fresh, "Model routing"),
        "Tech stack & standards",
    );
    t.write("AGENTS.md", &bare);
    assert_eq!(w.update("dev").unwrap().value.merged, ["AGENTS.md"]);
    let got = t.read("AGENTS.md");
    assert_eq!(heading_order(&got), heading_order(&fresh));
    assert_eq!(got, fresh);
}

#[test]
fn update_reports_an_unmergeable_agents_and_restores_a_missing_one() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    t.write("AGENTS.md", "no headings\n");
    let rep = w.update("dev").unwrap().value;
    assert!(rep.skipped.contains(&"AGENTS.md".to_string()));
    assert_eq!(
        rep.not_merged,
        [("AGENTS.md".to_string(), "no ## headings".to_string())]
    );
    assert_eq!(t.read("AGENTS.md"), "no headings\n");
    std::fs::remove_file(t.p("AGENTS.md")).unwrap();
    let rep = w.update("dev").unwrap().value;
    assert!(rep.updated.contains(&"AGENTS.md".to_string()));
}

// ---- rooms ----

/// AC7.
#[test]
fn ensure_room_writes_the_room_and_refreshes_agents() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(false, false)).unwrap();
    let rep = w.ensure_room("build", "dev").unwrap().value;
    assert!(
        rep.created
            && rep
                .written
                .contains(&"build/01_design/context.md".to_string())
    );
    let mut sorted = rep.written.clone();
    sorted.sort();
    assert_eq!(rep.written, sorted);
    assert!(
        manifest(&t)
            .files
            .contains_key("build/01_design/context.md")
    );
    assert_eq!(t.read("AGENTS.md"), agents::assemble(&["build"]));
    assert_eq!(
        w.ensure_room("build", "dev").unwrap().value,
        RoomReport::default(),
        "an existing room is left alone"
    );
    assert_eq!(
        w.ensure_room("bogus", "dev").unwrap_err().to_string(),
        "daiku: unknown room \"bogus\""
    );
}

#[test]
fn ensure_room_outside_a_workspace_and_with_a_missing_manifest() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    assert_eq!(
        w.ensure_room("plan", "dev").unwrap_err().to_string(),
        format!(
            "daiku: {} is not a daiku workspace (run `daiku init`)",
            crate::gofmt::to_slash(&t.0.to_string_lossy())
        )
    );
    w.init(&opts(true, false)).unwrap();
    std::fs::remove_file(t.p(".daiku/manifest.json")).unwrap();
    w.ensure_room("plan", "dev").unwrap();
    assert!(manifest(&t).files.keys().all(|k| k.starts_with("plan/")));
    assert!(
        t.p("plan/specs").is_dir(),
        "vcs none still creates the .gitkeep folders"
    );
    assert!(!t.p("plan/specs/.gitkeep").exists());
}

#[cfg(unix)]
#[test]
fn a_symlinked_room_is_refused() {
    let (t, c, outside) = (Temp::new(), Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    std::os::unix::fs::symlink(&outside.0, t.p("build")).unwrap();
    assert_eq!(
        w.ensure_room("build", "dev").unwrap_err().to_string(),
        "daiku: build/ is a symlink, which could lead outside the workspace — replace it with a folder"
    );
    assert_eq!(std::fs::read_dir(&outside.0).unwrap().count(), 0);
}

#[test]
fn the_build_room_needs_its_marker() {
    let (t, c) = (Temp::new(), Temp::new());
    let w = ws(&t.0, &c);
    w.init(&opts(true, false)).unwrap();
    std::fs::create_dir_all(t.p("build/tickets")).unwrap();
    let e = engine::Engine {
        root: &t.0,
        vcs: "none".into(),
        version: "dev".into(),
    };
    assert!(
        !e.has_room("build"),
        "a build/ folder without 01_design isn't the room"
    );
    std::fs::create_dir_all(t.p("plan")).unwrap();
    assert!(e.has_room("plan"));
}

// ---- check ----

#[test]
fn the_shipped_scaffold_is_clean() {
    assert_eq!(check(), Vec::<String>::new());
}

#[test]
fn check_allows_the_links_placeholder_only() {
    let flagged = has_template_syntax;
    assert!(!flagged("## Links\n\n{{links}}\n"));
    assert!(flagged("{{links}} and {{title}}"));
    assert!(flagged("{{ links }}"));
}
