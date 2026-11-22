//! State tests: Go's `internal/core/state` and `internal/state` tests, by
//! behaviour, on daiku's two loops, plus S-0005's AC2 and AC3.

use std::collections::BTreeMap;
use std::rc::Rc;

use super::machine::*;
use super::store::SeedOptions;
use super::*;
use crate::env::{Cmd, Output};
use crate::fake::ScriptedProcess;
use crate::testutil::{self, RealProcess, Temp, git, git_repo};

fn gates(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn change(pairs: &[(&str, &str)]) -> Change {
    Change {
        name: "1-x".into(),
        loop_: PLAN_LOOP.into(),
        gates: gates(pairs),
        ..Change::default()
    }
}

fn local(t: &Temp) {
    t.write(".daiku/settings.json", "{\"vcs\": \"none\"}\n");
}

fn opts() -> SeedOptions {
    SeedOptions::default()
}

/// A scripted runner the test can still inspect after handing it to an `Env`.
struct Shared(Rc<ScriptedProcess>);

impl crate::env::Process for Shared {
    fn output(&self, cmd: &Cmd) -> std::io::Result<Output> {
        self.0.output(cmd)
    }
    fn run_attached(&self, cmd: &Cmd) -> std::io::Result<i32> {
        self.0.run_attached(cmd)
    }
}

fn scripted(t: &Temp) -> (crate::Env, Rc<ScriptedProcess>) {
    let p = Rc::new(ScriptedProcess::default());
    (testutil::env(t, Box::new(Shared(p.clone()))), p)
}

// --- the machine (state_test.go, reasons_test.go, loops_test.go) ---

#[test]
fn check_gate_order() {
    type Case<'a> = (&'a [(&'a str, &'a str)], &'a str, &'a str, Option<&'a str>);
    let cases: &[Case] = &[
        (&[], "intake", DRAFT, None),
        (
            &[("intake", AWAIT_APPROVAL)],
            "research",
            DRAFT,
            Some("daiku: research requires intake to be approved (currently \"await-approval\")"),
        ),
        (
            &[
                ("intake", APPROVED),
                ("research", SKIPPED),
                ("explore", SKIPPED),
                ("synthesis", SKIPPED),
            ],
            "spec",
            DRAFT,
            None,
        ),
        (
            &[("intake", APPROVED)],
            "spec",
            DRAFT,
            Some("daiku: spec requires research to be approved (currently \"\")"),
        ),
        (&[], "publish", "", None),
    ];
    for (g, phase, status, want) in cases {
        let got = PLAN
            .check_gate(&change(g), phase, status)
            .map(|e| e.to_string());
        assert_eq!(got.as_deref(), *want, "{phase} {status}");
    }
    let c = change(&[
        ("ticket", APPROVED),
        ("requirements", APPROVED),
        ("design", APPROVED),
        ("tasks", APPROVED),
        ("implement", IN_PROGRESS),
    ]);
    assert_eq!(
        BUILD.check_gate(&c, "test", DRAFT).unwrap().to_string(),
        "daiku: test requires implement to be complete (currently \"in-progress\")"
    );
}

#[test]
fn set_gate_validation() {
    let cases = [
        ("bogus", DRAFT, "", "unknown phase: bogus"),
        ("intake", "nope", "", "unknown gate status: nope"),
        (
            "intake",
            IN_PROGRESS,
            "",
            "unknown gate status: in-progress",
        ),
        ("publish", APPROVED, "", "unknown gate status: approved"),
        ("intake", SKIPPED, "r", "daiku: intake can't be skipped"),
        ("publish", SKIPPED, "r", "unknown gate status: skipped"),
        (
            "research",
            SKIPPED,
            "",
            "daiku: skipping research requires a reason",
        ),
    ];
    for (phase, status, reason, want) in cases {
        let mut c = Change::new("1-x");
        let err = PLAN.set_gate(&mut c, phase, status, reason).unwrap_err();
        assert_eq!(err.to_string(), want);
        assert_eq!(c, Change::new("1-x"), "a refused gate changed the record");
    }
    let mut c = Change::new("1-x");
    PLAN.set_gate(&mut c, "research", SKIPPED, "known").unwrap();
    assert_eq!((c.phase.as_str(), c.status.as_str()), ("research", SKIPPED));
    assert_eq!(c.reasons, gates(&[("research", "known")]));
    PLAN.set_gate(&mut c, "research", DRAFT, "ignored").unwrap();
    assert!(c.reasons.is_empty(), "only a skip keeps its reason");
    PLAN.set_gate(&mut c, "publish", IN_PROGRESS, "").unwrap();
    assert_eq!(c.gate("publish"), IN_PROGRESS);
}

#[test]
fn reopen_clears_later_approvals_but_not_skips() {
    let mut c = change(&[
        ("intake", APPROVED),
        ("research", SKIPPED),
        ("spec", APPROVED),
        ("architecture", APPROVED),
        ("publish", IN_PROGRESS),
    ]);
    c.reasons = gates(&[("research", "known")]);
    PLAN.reopen(&mut c, "intake", "requirements changed")
        .unwrap();
    assert_eq!(c.gates, gates(&[("intake", DRAFT), ("research", SKIPPED)]));
    assert_eq!((c.phase.as_str(), c.status.as_str()), ("intake", DRAFT));
    assert_eq!(c.reasons, gates(&[("research", "known")]));

    let mut b = change(&[
        ("ticket", APPROVED),
        ("implement", COMPLETE),
        ("test", AWAIT_APPROVAL),
    ]);
    BUILD.reopen(&mut b, "implement", "bug").unwrap();
    assert_eq!(
        b.gates,
        gates(&[("ticket", APPROVED), ("implement", IN_PROGRESS)])
    );

    let mut u = change(&[("intake", APPROVED), ("research", SKIPPED)]);
    u.reasons = gates(&[("research", "known")]);
    PLAN.reopen(&mut u, "research", "there's research after all")
        .unwrap();
    assert_eq!(u.gate("research"), DRAFT);
    assert!(u.reasons.is_empty());
}

#[test]
fn reopen_refusals() {
    let mut c = change(&[("intake", APPROVED)]);
    for (phase, reason, want) in [
        ("intake", "", "daiku: reopening intake requires a reason"),
        ("spec", "r", "daiku: spec hasn't started"),
        ("bogus", "r", "unknown phase: bogus"),
    ] {
        assert_eq!(
            PLAN.reopen(&mut c, phase, reason).unwrap_err().to_string(),
            want
        );
    }
}

#[test]
fn close_refuses_further_transitions() {
    let mut c = change(&[("intake", APPROVED)]);
    assert_eq!(
        PLAN.close(&mut c, "").unwrap_err().to_string(),
        "daiku: closing 1-x requires a reason"
    );
    PLAN.close(&mut c, "abandoned").unwrap();
    assert!(c.is_closed());
    assert_eq!(c.gate("intake"), APPROVED);
    assert_eq!(
        PLAN.set_gate(&mut c, "research", DRAFT, "")
            .unwrap_err()
            .to_string(),
        "daiku: 1-x is closed"
    );
    assert_eq!(
        PLAN.reopen(&mut c, "intake", "r").unwrap_err().to_string(),
        "daiku: 1-x is closed"
    );
    assert_eq!(
        PLAN.close(&mut c, "again").unwrap_err().to_string(),
        "daiku: 1-x is already closed"
    );
}

#[test]
fn machines_match_go() {
    let plan: Vec<_> = PLAN.names().collect();
    assert_eq!(
        plan,
        [
            "intake",
            "research",
            "explore",
            "synthesis",
            "spec",
            "architecture",
            "breakdown",
            "publish"
        ]
    );
    let build: Vec<_> = BUILD.names().collect();
    assert_eq!(
        build,
        [
            "ticket",
            "requirements",
            "design",
            "tasks",
            "implement",
            "test",
            "review",
            "security",
            "ship"
        ]
    );
    let all = || PLAN.phases.iter().chain(BUILD.phases);
    let skippable: Vec<_> = all().filter(|p| p.skippable).map(|p| p.name).collect();
    assert_eq!(skippable, ["research", "explore", "synthesis", "security"]);
    let run: Vec<_> = all()
        .filter(|p| p.kind == Kind::Run)
        .map(|p| p.name)
        .collect();
    assert_eq!(run, ["publish", "implement", "ship"]);
    // loops_test.go: no phase name is in both loops.
    assert!(plan.iter().all(|p| !build.contains(p)));
}

#[test]
fn slugify_matches_go() {
    for (input, want) in [
        ("Auth Flow!", "auth-flow"),
        ("  ", ""),
        ("Refactor DB layer", "refactor-db-layer"),
        (
            "Bump kiso to v0.9.0 and release tomo v0.11.0",
            "bump-kiso-to-v0-9-0-and-release-tomo-v0",
        ),
        ("İstanbul \u{212A}iosk", "istanbul-kiosk"),
        ("café", "caf"),
    ] {
        assert_eq!(slugify(input), want, "{input:?}");
    }
    assert_eq!(slugify(&"x".repeat(100)).len(), 40);
}

// --- STATE.json decoding against tests/state-go.txt (Go's behaviour, captured from
// v0.2.0 and kept as daiku's expected behaviour) ---

/// The decoded file as Go's `json.Marshal` prints `stateFile`.
fn go_json(f: &file::RawFile) -> String {
    fn s(v: &str) -> String {
        let mut out = String::from("\"");
        for c in v.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\u{8}' => out.push_str("\\b"),
                '\u{c}' => out.push_str("\\f"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                '<' | '>' | '&' | '\u{2028}' | '\u{2029}' => {
                    out.push_str(&format!("\\u{:04x}", c as u32))
                }
                c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                c => out.push(c),
            }
        }
        out.push('"');
        out
    }
    fn map(m: &Option<BTreeMap<String, String>>) -> String {
        match m {
            None => "null".into(),
            Some(m) => {
                let parts: Vec<_> = m
                    .iter()
                    .map(|(k, v)| format!("{}:{}", s(k), s(v)))
                    .collect();
                format!("{{{}}}", parts.join(","))
            }
        }
    }
    fn slice(v: &crate::gojson::decode::GoSlice) -> String {
        if v.is_nil() {
            return "null".into();
        }
        let parts: Vec<_> = v.items().iter().map(|x| s(x)).collect();
        format!("[{}]", parts.join(","))
    }
    fn one(c: &file::RawChange) -> String {
        let mut fields = vec![
            format!("\"loop\":{}", s(&c.loop_)),
            format!("\"phase\":{}", s(&c.phase)),
            format!("\"status\":{}", s(&c.status)),
            format!("\"gates\":{}", map(&c.gates)),
        ];
        if c.reasons.as_ref().is_some_and(|r| !r.is_empty()) {
            fields.push(format!("\"reasons\":{}", map(&c.reasons)));
        }
        fields.push(format!("\"sources\":{}", slice(&c.sources)));
        if !c.repos.items().is_empty() {
            fields.push(format!("\"repos\":{}", slice(&c.repos)));
        }
        fields.push(format!("\"updatedAt\":{}", s(&c.updated_at)));
        format!("{{{}}}", fields.join(","))
    }
    let changes = match &f.changes {
        None => "null".into(),
        Some(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, c)| format!("{}:{}", s(k), c.as_ref().map_or("null".into(), one)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    };
    format!("{{\"schema\":{},\"changes\":{changes}}}", f.schema)
}

#[test]
fn state_json_decodes_as_go() {
    let table = include_str!("../../tests/state-go.txt");
    let mut n = 0;
    for line in table.lines() {
        let (hex, want) = line.split_once('\t').unwrap();
        let input: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let got = match file::decode(&input) {
            Ok(f) => go_json(&f),
            Err(e) => format!("error: {e}"),
        };
        assert_eq!(got, want, "input {}", String::from_utf8_lossy(&input));
        n += 1;
    }
    assert!(n > 60, "the table should cover Go's decoding rules");
}

// --- the store ---

#[test]
fn seed_records_skips_and_logs() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    let o = SeedOptions {
        sources: vec!["plan/00_intake/1.md".into()],
        skip: vec![
            ("explore", NO_REPO_REASON),
            ("synthesis", SINGLE_REPO_REASON),
        ],
        ..opts()
    };
    let c = s.seed("1-x", PLAN_LOOP, o).unwrap();
    assert_eq!((c.phase.as_str(), c.status.as_str()), ("intake", DRAFT));
    assert_eq!(c.gate("explore"), SKIPPED);
    assert!(t.exists("plan/specs/1-x"));
    assert!(
        !t.exists("plan/specs/1-x/.gitkeep"),
        "vcs none writes no .gitkeep"
    );
    assert_eq!(
        t.read("LOG.md"),
        "- 2026-11-07T01:02:03.004Z 1-x: created\n\
         - 2026-11-07T01:02:03.004Z 1-x: explore → skipped (no repo in the intake)\n\
         - 2026-11-07T01:02:03.004Z 1-x: synthesis → skipped (fewer than two repos in scope)\n"
    );
    assert_eq!(t.read(".daiku/active"), "1-x\n");
    assert_eq!(s.load().unwrap().sources, ["plan/00_intake/1.md"]);
    assert_eq!(
        t.read("STATE.json"),
        r#"{
  "schema": 1,
  "changes": {
    "1-x": {
      "loop": "plan",
      "phase": "intake",
      "status": "draft",
      "gates": {
        "explore": "skipped",
        "intake": "draft",
        "synthesis": "skipped"
      },
      "reasons": {
        "explore": "no repo in the intake",
        "synthesis": "fewer than two repos in scope"
      },
      "sources": [
        "plan/00_intake/1.md"
      ],
      "updatedAt": "2026-11-07T01:02:03.004Z"
    }
  }
}
"#
    );
}

#[test]
fn seed_refusals_write_nothing() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    assert_eq!(
        s.seed("../evil", PLAN_LOOP, opts())
            .unwrap_err()
            .to_string(),
        "daiku: invalid change name \"../evil\""
    );
    let o = SeedOptions {
        skip: vec![("spec", "r")],
        ..opts()
    };
    assert_eq!(
        s.seed("1-x", PLAN_LOOP, o).unwrap_err().to_string(),
        "daiku: spec can't be skipped"
    );
    assert!(!t.exists("plan") && !t.exists("STATE.json"));
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    assert_eq!(
        s.seed("1-x", PLAN_LOOP, opts()).unwrap_err().to_string(),
        format!("daiku: change \"1-x\" already exists in {}", s.state_path())
    );
    // A corrupt file is never overwritten.
    t.write("STATE.json", "{\"changes\":{");
    assert!(s.seed("2-y", PLAN_LOOP, opts()).is_err());
    assert_eq!(t.read("STATE.json"), "{\"changes\":{");
}

#[test]
fn reads_refuse_bad_files() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    let s = Store::reader(t.path(), &env);
    let path = s.state_path();
    for (data, want) in [
        (
            "{\"schema\":99,\"changes\":{}}",
            format!(
                "daiku: {path} has schema 99, newer than this version supports (1) — upgrade daiku"
            ),
        ),
        (
            "{\"changes\":{\"a\":{}}}",
            format!("daiku: {path}: change \"a\" records no loop"),
        ),
        (
            "{\"changes\":{\"a\":{\"loop\":\"zumen\"}}}",
            format!("daiku: {path}: change \"a\" records unknown loop \"zumen\""),
        ),
        (
            "{ not json\n",
            format!(
                "daiku: cannot parse {path}: invalid character 'n' looking for beginning of object key string"
            ),
        ),
    ] {
        t.write("STATE.json", data);
        assert_eq!(s.read_changes().unwrap_err().to_string(), want);
    }
    t.write(
        "STATE.json",
        r#"{"changes":{"a":null,"b":{"loop":"plan","gates":{"x":"","y":"draft"}}}}"#,
    );
    let changes = s.read_changes().unwrap();
    assert_eq!(changes.keys().collect::<Vec<_>>(), ["b"]);
    assert_eq!(changes["b"].gates, gates(&[("y", "draft")]));
    std::fs::remove_file(t.p("STATE.json")).unwrap();
    std::fs::create_dir(t.p("STATE.json")).unwrap();
    let err = s.read_changes().unwrap_err().to_string();
    #[cfg(unix)]
    assert!(
        err.starts_with(&format!("daiku: cannot read {path}: read {path}: ")),
        "{err}"
    );
    #[cfg(windows)]
    assert!(
        err.starts_with(&format!("daiku: cannot read {path}: ")),
        "{err}"
    );
}

#[test]
fn legacy_workspaces_are_refused_on_every_read() {
    for tool in ["zumen", "tomo"] {
        let t = Temp::new();
        std::fs::create_dir(t.p(&format!(".{tool}"))).unwrap();
        let (env, _) = scripted(&t);
        let s = Store::reader(t.path(), &env);
        let want = format!(
            "daiku: this is a {tool} workspace (found .{tool}/) — use {tool}, or start a new daiku workspace"
        );
        assert_eq!(s.read_changes().unwrap_err().to_string(), want);
        assert_eq!(s.load().unwrap_err().to_string(), want);
        assert_eq!(s.known().unwrap_err().to_string(), want);
        assert_eq!(s.switch("1-x").unwrap_err().to_string(), want);
        assert_eq!(s.next_plan_id().unwrap_err().to_string(), want);
    }
}

#[test]
fn switch_and_known() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    for name in ["nope", "", "1-../x", "1-Weird_Name"] {
        assert_eq!(
            s.switch(name).unwrap_err().to_string(),
            format!("daiku: unknown change \"{name}\"")
        );
    }
    std::fs::create_dir_all(t.p("plan/specs/10-later")).unwrap();
    std::fs::create_dir_all(t.p("plan/specs/2-earlier")).unwrap();
    t.write("plan/specs/README.md", "x");
    // An unrecorded folder is recorded on switch.
    s.switch("10-later").unwrap();
    s.advance("intake", APPROVED, "", &[]).unwrap();
    let before = t.read("STATE.json");
    s.switch("10-later").unwrap();
    assert_eq!(
        t.read("STATE.json"),
        before,
        "switching to a recorded change rewrote STATE.json"
    );
    let known = s.known().unwrap();
    let rows: Vec<_> = known
        .iter()
        .map(|k| (k.name.as_str(), k.id, k.status.as_str()))
        .collect();
    assert_eq!(rows, [("2-earlier", 2, ""), ("10-later", 10, APPROVED)]);
}

#[test]
fn next_plan_id_counts_folders_and_records() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    let s = Store::reader(t.path(), &env);
    assert_eq!(s.next_plan_id().unwrap(), 1);
    std::fs::create_dir_all(t.p("plan/specs/3-a")).unwrap();
    std::fs::create_dir_all(t.p("plan/specs/notes")).unwrap();
    assert_eq!(s.next_plan_id().unwrap(), 4);
    t.write(
        "STATE.json",
        r#"{"changes":{"7-gone":{"loop":"plan"},"S-0009":{"loop":"build","repos":["api"]}}}"#,
    );
    assert_eq!(s.next_plan_id().unwrap(), 8);
}

#[test]
fn advance_and_reopen_need_an_active_folder() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    assert_eq!(
        s.advance("intake", APPROVED, "", &[])
            .unwrap_err()
            .to_string(),
        "daiku: no active change — run `daiku switch <change>` first"
    );
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    std::fs::remove_dir_all(t.p("plan/specs/1-x")).unwrap();
    let want =
        "daiku: active change \"1-x\" has no folder plan/specs/1-x/ — run `daiku switch <change>`";
    assert_eq!(
        s.advance("intake", APPROVED, "", &[])
            .unwrap_err()
            .to_string(),
        want
    );
    assert_eq!(s.reopen("intake", "r", &[]).unwrap_err().to_string(), want);
    t.write(".daiku/active", "9-unrecorded\n");
    assert_eq!(
        s.advance("intake", APPROVED, "", &[])
            .unwrap_err()
            .to_string(),
        "daiku: active change \"9-unrecorded\" has no folder — run `daiku switch <change>`"
    );
}

/// N-0006 (security review): the store folds any line it logs, so a caller that
/// skips the Workspace fold still can't add a line to LOG.md or the commit message.
#[test]
fn the_store_keeps_every_log_line_and_subject_on_one_line() {
    let t = Temp::new();
    git_repo(&t);
    let env = testutil::env(&t, Box::new(RealProcess));
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    let before = t.read("LOG.md").lines().count();
    s.advance("intake", APPROVED, "", &["override: a\nb".into()])
        .unwrap();
    s.reopen("intake", "c\r\n- 2026-01-01 1-x: ship → complete", &[])
        .unwrap();
    let log = t.read("LOG.md");
    assert_eq!(log.lines().count(), before + 3, "{log}");
    assert!(log.contains("1-x: override: a b\n"), "{log}");
    assert!(
        log.contains("1-x: reopened intake (c - 2026-01-01 1-x: ship → complete)\n"),
        "{log}"
    );
    assert!(!log.contains('\r'));
    let message = git(t.path(), &["log", "-1", "--format=%B"]);
    assert_eq!(
        message.trim_end(),
        "daiku: 1-x: reopened intake (c - 2026-01-01 1-x: ship → complete)"
    );
}

#[test]
fn advance_logs_notes_and_close_clears_only_its_pointer() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    s.advance("spec", APPROVED, "", &["override: forced".into()])
        .unwrap();
    let log = t.read("LOG.md");
    assert!(
        log.contains("1-x: spec → approved\n") && log.contains("1-x: override: forced\n"),
        "{log}"
    );
    s.seed("2-y", PLAN_LOOP, opts()).unwrap();
    s.close("1-x", "dropped").unwrap();
    assert_eq!(s.load().unwrap().name, "2-y");
    s.close("2-y", "done").unwrap();
    assert!(!t.exists(".daiku/active"));
    let changes = s.read_changes().unwrap();
    assert!(changes["1-x"].is_closed() && changes["2-y"].is_closed());
    assert!(t.read("LOG.md").ends_with("2-y: closed (done)\n"));
    assert_eq!(
        s.close("9-missing", "r").unwrap_err().to_string(),
        "daiku: unknown change \"9-missing\""
    );
}

#[cfg(unix)]
#[test]
fn state_files_through_symlinks_are_refused() {
    let t = Temp::new();
    let out = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    std::fs::remove_file(t.p("LOG.md")).unwrap();
    std::os::unix::fs::symlink(out.p("LOG.md"), t.p("LOG.md")).unwrap();
    let before = t.read("STATE.json");
    let err = s
        .advance("intake", APPROVED, "", &[])
        .unwrap_err()
        .to_string();
    assert!(err.contains("LOG.md is a symlink"), "{err}");
    assert_eq!(t.read("STATE.json"), before);
    assert!(!out.exists("LOG.md"));
    // A symlinked plan folder can't receive a seed.
    std::fs::remove_file(t.p("LOG.md")).unwrap();
    std::fs::create_dir_all(out.p("specs")).unwrap();
    std::fs::remove_dir_all(t.p("plan/specs")).unwrap();
    std::os::unix::fs::symlink(out.p("specs"), t.p("plan/specs")).unwrap();
    let err = s.seed("2-y", PLAN_LOOP, opts()).unwrap_err().to_string();
    assert!(
        err.contains("won't create plan/specs/2-y") && err.contains("is a symlink"),
        "{err}"
    );
    assert!(!out.exists("specs/2-y"));
}

#[test]
fn build_changes_live_under_their_repo() {
    let t = Temp::new();
    let (env, _) = scripted(&t);
    local(&t);
    let s = Store::at(t.path(), &env).unwrap();
    let o = SeedOptions {
        repos: vec!["api".into()],
        sources: vec!["plan/specs/1-x/tickets/S-0001.md".into()],
        ..opts()
    };
    let c = s.seed("S-0001", BUILD_LOOP, o).unwrap();
    assert_eq!((c.phase.as_str(), c.status.as_str()), ("ticket", DRAFT));
    assert!(t.exists("build/api/changes/S-0001"));
    assert_eq!(
        s.seed("1-x", BUILD_LOOP, opts()).unwrap_err().to_string(),
        "daiku: invalid change name \"1-x\""
    );
    let bad = SeedOptions {
        repos: vec!["../x".into()],
        ..opts()
    };
    assert_eq!(
        s.seed("S-0002", BUILD_LOOP, bad).unwrap_err().to_string(),
        "build change \"S-0002\" has an invalid repo name \"../x\""
    );
    assert_eq!(
        s.seed("S-0003", BUILD_LOOP, opts())
            .unwrap_err()
            .to_string(),
        "build change \"S-0003\" has no repo"
    );
    // Each change follows its own machine.
    s.advance("ticket", APPROVED, "", &[]).unwrap();
    assert_eq!(
        s.advance("intake", APPROVED, "", &[])
            .unwrap_err()
            .to_string(),
        "unknown phase: intake"
    );
    let known = s.known().unwrap();
    assert_eq!(
        (known[0].name.as_str(), known[0].loop_.as_str()),
        ("S-0001", BUILD_LOOP)
    );
}

// --- commits (state_test.go's git tests, refresh_test.go) ---

fn last_subject(t: &Temp) -> String {
    git(t.path(), &["log", "-1", "--format=%s"])
        .trim()
        .to_string()
}

fn head_files(t: &Temp) -> Vec<String> {
    git(t.path(), &["show", "--name-only", "--format=", "HEAD"])
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn transitions_commit_state_and_log() {
    let t = Temp::new();
    git_repo(&t);
    let env = testutil::env(&t, Box::new(RealProcess));
    let s = Store::at(t.path(), &env).unwrap();
    t.write("plan/00_intake/1.md", "x");
    let o = SeedOptions {
        paths: vec!["plan/00_intake/1.md".into()],
        ..opts()
    };
    s.seed("1-x", PLAN_LOOP, o).unwrap();
    assert_eq!(last_subject(&t), "daiku: 1-x: created");
    assert_eq!(
        head_files(&t),
        [
            "LOG.md",
            "STATE.json",
            "plan/00_intake/1.md",
            "plan/specs/1-x/.gitkeep"
        ]
    );
    s.advance("intake", APPROVED, "", &[]).unwrap();
    assert_eq!(last_subject(&t), "daiku: 1-x: intake → approved");
    s.reopen("intake", "changed", &[]).unwrap();
    assert_eq!(last_subject(&t), "daiku: 1-x: reopened intake (changed)");
    s.close("1-x", "done").unwrap();
    assert_eq!(last_subject(&t), "daiku: 1-x: closed (done)");
    assert_eq!(git(t.path(), &["status", "--porcelain"]), "");
    assert!(s.take_warnings().is_empty());
}

#[test]
fn a_gate_commits_the_change_folders_and_nothing_else() {
    let t = Temp::new();
    git_repo(&t);
    let env = testutil::env(&t, Box::new(RealProcess));
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    t.write("plan/specs/1-x/functional-spec.md", "reqs\n");
    t.write("plan/01_research/1-x/notes.md", "n\n");
    t.write("notes.md", "wip\n");
    t.write("other.md", "staged\n");
    git(t.path(), &["add", "other.md"]);
    s.advance("intake", APPROVED, "", &[]).unwrap();
    assert_eq!(
        head_files(&t),
        [
            "LOG.md",
            "STATE.json",
            "plan/01_research/1-x/notes.md",
            "plan/specs/1-x/README.md",
            "plan/specs/1-x/functional-spec.md"
        ]
    );
    let status = git(t.path(), &["status", "--porcelain"]);
    assert!(
        status.contains("?? notes.md") && status.contains("A  other.md"),
        "{status}"
    );
    // A deletion in the folder is committed too.
    std::fs::remove_file(t.p("plan/specs/1-x/functional-spec.md")).unwrap();
    s.advance("research", DRAFT, "", &[]).unwrap();
    assert!(head_files(&t).contains(&"plan/specs/1-x/functional-spec.md".to_string()));
}

#[test]
fn outside_git_warns_and_still_applies() {
    let t = Temp::new();
    let env = testutil::env(&t, Box::new(RealProcess));
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    let w = s.take_warnings();
    assert_eq!(w.len(), 1);
    assert!(w[0].message.ends_with(
        " is not in a git repo — this state change was not committed; set \"vcs\": \"none\" if this workspace is meant to be local"
    ));
    assert_eq!(s.load().unwrap().name, "1-x");
}

#[cfg(unix)]
#[test]
fn a_failed_commit_is_an_error_but_the_state_stands() {
    use std::os::unix::fs::PermissionsExt;
    let t = Temp::new();
    git_repo(&t);
    let env = testutil::env(&t, Box::new(RealProcess));
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    t.write(".git/hooks/pre-commit", "#!/bin/sh\nexit 1\n");
    std::fs::set_permissions(
        t.p(".git/hooks/pre-commit"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    let err = s
        .advance("intake", APPROVED, "", &[])
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("daiku: state saved but not committed: git commit: exit status 1"),
        "{err}"
    );
    assert_eq!(s.load().unwrap().gate("intake"), APPROVED);
}

#[test]
fn vcs_none_in_a_repo_commits_and_stages_nothing() {
    let t = Temp::new();
    git_repo(&t);
    local(&t);
    let env = testutil::env(&t, Box::new(RealProcess));
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    s.advance("intake", AWAIT_APPROVAL, "", &[]).unwrap();
    assert_eq!(last_subject(&t), "init");
    assert_eq!(
        git(t.path(), &["diff", "--cached", "--name-only"]).trim(),
        ""
    );
}

fn exit(code: i32) -> Output {
    Output {
        code: Some(code),
        ..Output::default()
    }
}

/// AC2: under git, with a test clock and a scripted runner, a gate logs the
/// clock's time and makes Go's git calls for exactly its paths (STATE.json,
/// LOG.md, the change's folder and the regenerated README), in Go's order.
#[test]
fn ac2_a_gate_makes_gos_git_calls() {
    let t = Temp::new();
    let (env, process) = scripted(&t);
    std::fs::create_dir_all(t.p("plan/specs/1-x")).unwrap();
    t.write(".daiku/active", "1-x\n");
    t.write(
        "STATE.json",
        r#"{"schema":1,"changes":{"1-x":{"loop":"plan","phase":"intake","status":"draft","gates":{"intake":"draft"},"sources":[],"updatedAt":"x"}}}"#,
    );
    // rev-parse and add succeed; STATE.json, LOG.md and the regenerated README
    // have staged changes, the folder (as its own path) doesn't; commit succeeds.
    for code in [0, 0, 1, 1, 0, 1, 0] {
        process.reply(exit(code));
    }
    let s = Store::at(t.path(), &env).unwrap();
    s.advance("intake", APPROVED, "", &[]).unwrap();
    assert_eq!(
        t.read("LOG.md"),
        "- 2026-11-07T01:02:03.004Z 1-x: intake → approved\n"
    );
    let calls: Vec<Vec<String>> = process
        .calls
        .borrow()
        .iter()
        .map(|c| {
            assert_eq!(c.program, "git");
            assert_eq!(c.dir.as_deref(), Some(t.path()));
            c.args.clone()
        })
        .collect();
    let want: Vec<Vec<&str>> = vec![
        vec!["rev-parse", "--is-inside-work-tree"],
        vec![
            "add",
            "--",
            "STATE.json",
            "LOG.md",
            "plan/specs/1-x",
            "plan/specs/1-x/README.md",
        ],
        vec!["diff", "--cached", "--quiet", "--", "STATE.json"],
        vec!["diff", "--cached", "--quiet", "--", "LOG.md"],
        vec!["diff", "--cached", "--quiet", "--", "plan/specs/1-x"],
        vec![
            "diff",
            "--cached",
            "--quiet",
            "--",
            "plan/specs/1-x/README.md",
        ],
        vec![
            "commit",
            "--quiet",
            "--only",
            "-m",
            "daiku: 1-x: intake → approved",
            "--",
            "STATE.json",
            "LOG.md",
            "plan/specs/1-x/README.md",
        ],
    ];
    assert_eq!(calls, want);
}

/// AC3: under `vcs: none`, no transition runs any process.
#[test]
fn ac3_vcs_none_runs_no_process() {
    let t = Temp::new();
    local(&t);
    let (env, process) = scripted(&t);
    let s = Store::at(t.path(), &env).unwrap();
    s.seed("1-x", PLAN_LOOP, opts()).unwrap();
    s.advance("intake", APPROVED, "", &[]).unwrap();
    s.reopen("intake", "r", &[]).unwrap();
    s.seed("2-y", PLAN_LOOP, opts()).unwrap();
    s.switch("1-x").unwrap();
    s.close("2-y", "r").unwrap();
    assert!(process.calls.borrow().is_empty());
    assert!(s.take_warnings().is_empty());
}

/// Review finding 1: joining onto the CLI's root `.` adds nothing, as Go's
/// `filepath.Join` does, so messages read `STATE.json`, not `./STATE.json`.
#[test]
fn paths_from_the_dot_root_have_no_prefix() {
    use super::store::{at, read_file};
    use std::path::{Path, PathBuf};
    assert_eq!(
        at(Path::new("."), "STATE.json"),
        PathBuf::from("STATE.json")
    );
    assert_eq!(
        at(Path::new("."), "./plan/specs/1-x"),
        PathBuf::from("plan").join("specs").join("1-x")
    );
    assert_eq!(at(Path::new("."), ""), PathBuf::from("."));
    assert_eq!(
        at(Path::new("/w"), "a/b"),
        Path::new("/w").join("a").join("b")
    );
    // A message built from the joined path, here a read of a folder.
    let t = Temp::new();
    std::fs::create_dir(t.p("STATE.json")).unwrap();
    let err = read_file(&at(t.path(), "STATE.json")).unwrap_err();
    let shown = crate::gofmt::to_slash(&t.p("STATE.json").to_string_lossy());
    assert!(
        err.starts_with(&format!("read {shown}: ")) || err.starts_with(&format!("open {shown}: ")),
        "{err}"
    );
}
