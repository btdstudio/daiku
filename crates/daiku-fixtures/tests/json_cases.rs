//! The `--json` contract (spec R5, ADR-0004), checked over every case in
//! `fixtures/json-cases/`. The cases themselves run in CI's `rust-fixtures` job,
//! against the Rust daiku only: Go has no `--json`.
//!
//! Each case's expected output must be one envelope with nothing on stderr: data
//! and warnings with exit 0 (`tickets check` exits 1 when its report has errors),
//! or an error with exit 1. Every read form must have a data case and an error case, so a form
//! can't lose its `--json` path without a test noticing.
//!
//! Each case `json-<name>` uses the inputs of the Go case `fixtures/cases/<name>`
//! plus `--json`, and is checked against it: the same exit code, an error message
//! that is Go's stderr, and warnings that are Go's `warning: ` lines (N-0002).

use std::collections::BTreeMap;
use std::path::Path;

use daiku_fixtures::{Case, load_cases};
use serde_json::Value;

/// Every read form in spec R5: its name, and why it has no data case or no error
/// case, when it hasn't.
const FORMS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("status", None, None),
    ("pr", None, None),
    ("status <plan>", None, None),
    ("switch", None, None),
    ("settings", None, None),
    ("artifacts", None, None),
    ("next", None, None),
    ("source list", None, None),
    ("source show", None, None),
    ("adr list", None, None),
    ("adr show", None, None),
    (
        "idea list",
        None,
        Some("Go's idea list has no failing path (ERROR-PATHS.md)"),
    ),
    ("idea show", None, None),
    ("tickets check", None, None),
    ("tickets status", None, None),
    ("tickets list", None, None),
    ("tickets order", None, None),
    ("repo list", None, None),
    ("references check", None, None),
    ("env", None, None),
    ("version", None, Some("version can't fail")),
    // A group with no verb answers with its usage block (N-0002).
    ("tickets", Some("a bare group is only a usage error"), None),
    ("idea", Some("a bare group is only a usage error"), None),
    ("repo", Some("a bare group is only a usage error"), None),
];

/// JSON cases with no Go case of the same name, and the Go case whose starting
/// files each one takes.
const NO_TWIN: &[(&str, &str)] = &[
    ("json-artifacts-usage", "artifacts-no-args"),
    ("json-next-usage", "next-extra-argument"),
    ("json-next-control-chars", "next"),
    ("json-tickets-list-usage", "tickets-list-extra-argument"),
    ("json-tickets-check-corrupt-state", "status-corrupt-state"),
    ("json-tickets-status-corrupt-state", "status-corrupt-state"),
];

fn fixtures() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn cases() -> Vec<Case> {
    load_cases(&fixtures().join("json-cases"), &[]).expect("load fixtures/json-cases")
}

/// The read form a case's arguments name.
fn form(args: &[String]) -> String {
    let args: Vec<&str> = args
        .iter()
        .map(String::as_str)
        .filter(|a| *a != "--json")
        .collect();
    match args.as_slice() {
        ["--version" | "-v" | "version", ..] => "version".into(),
        ["status", plan, ..] if !plan.starts_with('-') && args.len() == 2 => "status <plan>".into(),
        ["references", ..] => "references check".into(),
        ["source" | "adr", verb @ ("list" | "show"), ..] => format!("{} {verb}", args[0]),
        ["source" | "adr", ..] => format!("{} list", args[0]),
        ["idea" | "tickets" | "repo", verb, ..] => format!("{} {verb}", args[0]),
        [command, ..] => (*command).into(),
        [] => String::new(),
    }
}

#[test]
fn every_case_holds_one_envelope() {
    let cases = cases();
    assert!(!cases.is_empty(), "fixtures/json-cases holds no cases");
    let mut problems = Vec::new();
    for case in &cases {
        let name = &case.name;
        let Some(expect) = &case.spec.expect else {
            problems.push(format!("{name}: not captured"));
            continue;
        };
        if !case.spec.args.iter().any(|a| a == "--json") {
            problems.push(format!("{name}: args lack --json"));
        }
        if !expect.stderr.is_empty() || !expect.stderr_bytes.is_empty() {
            problems.push(format!("{name}: writes to stderr"));
        }
        if !expect.changes.is_empty() {
            problems.push(format!("{name}: a read changed the workspace"));
        }
        let doc: Value = match serde_json::from_str(&expect.stdout) {
            Ok(v) => v,
            Err(e) => {
                problems.push(format!("{name}: stdout isn't one JSON document: {e}"));
                continue;
            }
        };
        let Some(obj) = doc.as_object() else {
            problems.push(format!("{name}: the envelope isn't an object"));
            continue;
        };
        if obj.get("schema") != Some(&Value::from(1)) {
            problems.push(format!("{name}: schema isn't 1"));
        }
        let keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        match keys.as_slice() {
            ["data", "schema", "warnings"] => {
                let strings = obj["warnings"]
                    .as_array()
                    .is_some_and(|w| w.iter().all(Value::is_string));
                if !strings {
                    problems.push(format!("{name}: warnings isn't a list of strings"));
                }
                let has_errors = obj["data"]
                    .get("errors")
                    .and_then(Value::as_array)
                    .is_some_and(|e| !e.is_empty());
                let want = i32::from(form(&case.spec.args) == "tickets check" && has_errors);
                if expect.exit != want {
                    problems.push(format!("{name}: data with exit {}", expect.exit));
                }
            }
            ["error", "schema"] => {
                let error = obj["error"].as_object();
                let ok = error.is_some_and(|e| {
                    e.len() == 2
                        && e.get("kind").is_some_and(Value::is_string)
                        && e.get("message").is_some_and(Value::is_string)
                });
                if !ok {
                    problems.push(format!("{name}: error isn't {{kind, message}}"));
                }
                if expect.exit != 1 {
                    problems.push(format!("{name}: error with exit {}", expect.exit));
                }
            }
            _ => problems.push(format!("{name}: envelope keys {keys:?}")),
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_read_form_has_a_data_and_an_error_case() {
    let mut seen: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    for case in cases() {
        let Some(expect) = &case.spec.expect else {
            continue;
        };
        let entry = seen.entry(form(&case.spec.args)).or_default();
        if expect.stdout.contains("\"error\": {") {
            entry.1 = true;
        } else {
            entry.0 = true;
        }
    }
    let mut problems = Vec::new();
    for (f, no_data, no_error) in FORMS {
        let (data, error) = seen.remove(*f).unwrap_or_default();
        if !data && no_data.is_none() {
            problems.push(format!("{f}: no data case"));
        }
        if data && no_data.is_some() {
            problems.push(format!("{f}: a data case for a form that has none"));
        }
        if !error && no_error.is_none() {
            problems.push(format!("{f}: no error case"));
        }
    }
    for f in seen.keys() {
        problems.push(format!("{f}: a case for a form that isn't in FORMS"));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// The Go case's stderr, as text.
fn stderr_text(case: &Case) -> String {
    let expect = case.spec.expect.as_ref().expect("captured");
    String::from_utf8_lossy(&expect.stderr_bytes().expect("stderr")).into_owned()
}

#[test]
fn every_case_agrees_with_its_go_twin() {
    let go: BTreeMap<String, Case> = load_cases(&fixtures().join("cases"), &[])
        .expect("load fixtures/cases")
        .into_iter()
        .map(|c| (c.name.clone(), c))
        .collect();
    let json = cases();
    let names: Vec<&str> = json.iter().map(|c| c.name.as_str()).collect();
    let mut problems = Vec::new();
    for (case, source) in NO_TWIN {
        if !names.contains(case) {
            problems.push(format!("NO_TWIN names {case}, which isn't a JSON case"));
        }
        if !go.contains_key(*source) {
            problems.push(format!("NO_TWIN: {case}'s source {source} isn't a Go case"));
        }
    }
    for case in &json {
        let name = &case.name;
        let listed = NO_TWIN.iter().any(|(c, _)| c == name);
        let twin = name.strip_prefix("json-").and_then(|n| go.get(n));
        let Some(twin) = twin else {
            if !listed {
                problems.push(format!("{name}: no Go twin, and not in NO_TWIN"));
            }
            continue;
        };
        if listed {
            problems.push(format!("{name}: in NO_TWIN, but {} exists", twin.name));
            continue;
        }
        let (Some(mine), Some(theirs)) = (&case.spec.expect, &twin.spec.expect) else {
            problems.push(format!("{name}: it or its twin isn't captured"));
            continue;
        };
        if mine.exit != theirs.exit {
            problems.push(format!(
                "{name}: exit {} where {} exits {}",
                mine.exit, twin.name, theirs.exit
            ));
        }
        let Ok(doc) = serde_json::from_str::<Value>(&mine.stdout) else {
            continue; // every_case_holds_one_envelope reports it
        };
        let stderr = stderr_text(twin);
        if let Some(message) = doc.pointer("/error/message").and_then(Value::as_str) {
            let want = stderr.strip_suffix('\n').unwrap_or(&stderr);
            if message != want {
                problems.push(format!(
                    "{name}: message {message:?} isn't {}'s stderr {want:?}",
                    twin.name
                ));
            }
        } else if let Some(warnings) = doc.get("warnings").and_then(Value::as_array) {
            let mine: Vec<String> = warnings
                .iter()
                .map(|w| format!("warning: {}", w.as_str().unwrap_or_default()))
                .collect();
            let theirs: Vec<&str> = stderr
                .lines()
                .filter(|l| l.starts_with("warning: "))
                .collect();
            if mine != theirs {
                problems.push(format!(
                    "{name}: warnings {mine:?} aren't {}'s {theirs:?}",
                    twin.name
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
