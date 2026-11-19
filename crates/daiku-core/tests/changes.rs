//! S-0005's acceptance criteria on daiku-core's public API, in workspaces the Go
//! CLI made (the fixtures' shared workspaces): AC4, AC5, AC6 and AC7.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use daiku_core::{Dirs, Env, Error, Workspace};

/// A temporary folder, removed when dropped.
struct Temp(PathBuf);

impl Temp {
    fn new() -> Temp {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "daiku-changes-{}-{}",
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
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Copies a dir, naming `*.fixture` files back to their real names.
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let name = e.file_name().to_string_lossy().into_owned();
        let dest = to.join(name.strip_suffix(".fixture").unwrap_or(&name));
        if e.file_type().unwrap().is_dir() {
            copy(&e.path(), &dest);
        } else {
            std::fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// A copy of a Go-made fixture workspace, opened with a fake `Env` whose home
/// holds no global settings.
fn workspace(name: &str) -> (Temp, Workspace) {
    let t = Temp::new();
    copy(
        &fixtures().join("workspaces").join(name).join("tree"),
        &t.p("ws"),
    );
    let env = Env::fake(Dirs {
        home: Some(t.p("home")),
        xdg_config_home: None,
    });
    let ws = Workspace::open(t.p("ws"), env);
    (t, ws)
}

/// Every file under `dir`, by relative path.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            if e.file_type().unwrap().is_dir() {
                walk(base, &e.path(), out);
            } else {
                let rel = e
                    .path()
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(e.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// `STATE.json` as a value, with every `updatedAt` blanked.
fn state_without_times(text: &str) -> serde_json::Value {
    let mut v: serde_json::Value = serde_json::from_str(text).unwrap();
    for c in v["changes"].as_object_mut().unwrap().values_mut() {
        c["updatedAt"] = serde_json::Value::Null;
    }
    v
}

/// AC4: a gate approved by Rust in a Go-made workspace leaves `STATE.json` with
/// the keys and values Go wrote for the same gate, `updatedAt` apart.
#[test]
fn ac4_state_matches_go_after_a_gate() {
    for (workspace_name, case, phase, status) in [
        ("plan-spec-ready", "gate-spec-approved", "spec", "approved"),
        (
            "plan-intake-draft",
            "gate-intake-approved",
            "intake",
            "approved",
        ),
        (
            "build-ticket-draft",
            "gate-build-ticket-approved",
            "ticket",
            "approved",
        ),
    ] {
        let (t, ws) = workspace(workspace_name);
        ws.gate(phase, status, "", false).unwrap();
        let go = std::fs::read_to_string(
            fixtures()
                .join("cases")
                .join(case)
                .join("expect/STATE.json"),
        )
        .unwrap();
        assert_eq!(
            state_without_times(&t.read("ws/STATE.json")),
            state_without_times(&go),
            "{case}"
        );
    }
}

/// AC5: a gate out of order under `gates: block` is a typed refusal with the
/// fixture's text, and nothing is written.
#[test]
fn ac5_a_refused_gate_is_typed_and_writes_nothing() {
    let (t, ws) = workspace("plan-intake-draft");
    let before = snapshot(&t.p("ws"));
    let err = ws.gate("spec", "draft", "", false).unwrap_err();
    assert!(
        matches!(&err, Error::GateRefused { violations } if violations.len() == 1),
        "{err:?}"
    );
    let case =
        std::fs::read_to_string(fixtures().join("cases/gate-out-of-order-blocked/case.toml"))
            .unwrap();
    assert!(
        case.contains(&format!("stderr = {:?}", format!("{err}\n"))),
        "{err}"
    );
    assert_eq!(snapshot(&t.p("ws")), before);
}

/// AC6: every command of this story refuses a zumen or tomo workspace with Go's
/// message.
#[test]
fn ac6_legacy_workspaces_are_refused() {
    for tool in ["zumen", "tomo"] {
        let (t, ws) = workspace("plan-intake-draft");
        std::fs::create_dir(t.p(&format!("ws/.{tool}"))).unwrap();
        let before = snapshot(&t.p("ws"));
        let want = format!(
            "daiku: this is a {tool} workspace (found .{tool}/) — use {tool}, or start a new daiku workspace"
        );
        let new = ws.change_new(&daiku_core::changes::ChangeNewOptions {
            title: "x".into(),
            ..Default::default()
        });
        let errors = [
            new.result.map(|_| ()).unwrap_err(),
            ws.change_close("1-auth-flow", "r").map(|_| ()).unwrap_err(),
            ws.gate("intake", "approved", "", false)
                .map(|_| ())
                .unwrap_err(),
            ws.reopen("intake", "r", false).map(|_| ()).unwrap_err(),
            ws.switch("1-auth-flow").map(|_| ()).unwrap_err(),
            ws.known_changes().map(|_| ()).unwrap_err(),
            ws.status().map(|_| ()).unwrap_err(),
            ws.status_plan("1-auth-flow").map(|_| ()).unwrap_err(),
        ];
        for e in errors {
            assert_eq!(e.to_string(), want);
        }
        assert!(!new.room_created);
        assert_eq!(snapshot(&t.p("ws")), before, "{tool}");
    }
}

/// AC7, as Go behaves: a gate refreshes the generated region and keeps the
/// narrative; a README missing its markers doesn't stop the gate but warns and
/// is left alone; a symlinked README refuses the gate before anything changes.
#[test]
fn ac7_the_overview_keeps_the_narrative() {
    let readme = "ws/plan/specs/1-auth-flow/README.md";
    let (t, ws) = workspace("plan-spec-ready");
    let edited = t.read(readme).replace(
        "<!-- About 250 words",
        "Sign-in for everyone.\n<!-- About 250 words",
    );
    std::fs::write(t.p(readme), &edited).unwrap();
    ws.gate("spec", "approved", "", false).unwrap();
    let after = t.read(readme);
    assert!(after.contains("Sign-in for everyone.\n"));
    assert!(after.contains("| spec | approved |  |"), "{after}");

    let (t, ws) = workspace("plan-spec-ready");
    std::fs::write(t.p(readme), "# No markers\n").unwrap();
    let out = ws.gate("spec", "approved", "", false).unwrap();
    assert_eq!(out.warnings.len(), 1);
    assert_eq!(
        out.warnings[0].message,
        "daiku: plan/specs/1-auth-flow/README.md: missing marker <!-- daiku:narrative -->"
    );
    assert_eq!(t.read(readme), "# No markers\n");

    #[cfg(unix)]
    {
        let (t, ws) = workspace("plan-spec-ready");
        let outside = Temp::new();
        std::fs::write(outside.p("README.md"), "x").unwrap();
        std::fs::remove_file(t.p(readme)).unwrap();
        std::os::unix::fs::symlink(outside.p("README.md"), t.p(readme)).unwrap();
        let before = snapshot(&t.p("ws/plan/specs"));
        let state = t.read("ws/STATE.json");
        let err = ws
            .gate("spec", "approved", "", false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("README.md is a symlink"), "{err}");
        assert_eq!(t.read("ws/STATE.json"), state);
        assert_eq!(snapshot(&t.p("ws/plan/specs")).len(), before.len());
        assert_eq!(outside.read("README.md"), "x");
    }
}
