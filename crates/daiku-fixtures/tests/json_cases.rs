//! The `--json` contract (spec R5, ADR-0004), checked over every case in
//! `fixtures/json-cases/`. The cases themselves run in CI's `rust-fixtures` job,
//! against the Rust daiku only: Go has no `--json`.
//!
//! Each case's expected output must be one envelope with nothing on stderr: data
//! and warnings with exit 0 (`tickets check` exits 1 when its report has errors),
//! or an error with exit 1. Every read form must have a data case and an error case, so a form
//! can't lose its `--json` path without a test noticing.

use std::collections::BTreeMap;
use std::path::Path;

use daiku_fixtures::{Case, load_cases};
use serde_json::Value;

/// Every read form in spec R5, and why one has no error case, if it hasn't.
const FORMS: &[(&str, Option<&str>)] = &[
    ("status", None),
    ("status <plan>", None),
    ("switch", None),
    ("settings", None),
    ("source list", None),
    ("source show", None),
    ("adr list", None),
    ("adr show", None),
    (
        "idea list",
        Some("Go's idea list has no failing path (ERROR-PATHS.md)"),
    ),
    ("idea show", None),
    ("tickets check", None),
    ("tickets status", None),
    ("tickets order", None),
    ("repo list", None),
    ("references check", None),
    ("env", None),
    ("version", Some("version can't fail")),
];

fn cases() -> Vec<Case> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/json-cases");
    load_cases(&dir, &[]).expect("load fixtures/json-cases")
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
    for (f, exempt) in FORMS {
        let (data, error) = seen.remove(*f).unwrap_or_default();
        if !data {
            problems.push(format!("{f}: no data case"));
        }
        if !error && exempt.is_none() {
            problems.push(format!("{f}: no error case"));
        }
    }
    for f in seen.keys() {
        problems.push(format!("{f}: a case for a form that isn't in FORMS"));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
