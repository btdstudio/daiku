//! Comparing an observation with a case's expectation, and writing it.

use std::path::Path;

use similar::TextDiff;

use crate::case::EXPECT_HEADER;
use crate::stored::stored_path;
use crate::{Case, CaseSpec, Error, Expect, Observation};

/// One part of a case that didn't match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Difference {
    /// `expect`, `exit`, `stdout`, `stderr`, `changes`, `tree/<path>` or `git-log`.
    pub part: String,
    /// What differs: a line diff, or a short description.
    pub detail: String,
}

fn diff(part: impl Into<String>, detail: impl Into<String>) -> Difference {
    Difference {
        part: part.into(),
        detail: detail.into(),
    }
}

/// Compares the observation with the case's `[expect]` and `expect/`, collecting every
/// difference.
pub fn compare(case: &Case, obs: &Observation) -> Vec<Difference> {
    let Some(want) = &case.spec.expect else {
        return vec![diff(
            "expect",
            "case.toml has no [expect] table; run capture",
        )];
    };
    let mut out = Vec::new();
    if want.exit != obs.exit {
        out.push(diff(
            "exit",
            format!("expected {}, got {}", want.exit, obs.exit),
        ));
    }
    let (mut stdout, mut got) = (lf(&want.stdout), lf(&obs.stdout));
    if case.spec.unordered_stdout {
        stdout = sorted_lines(&stdout);
        got = sorted_lines(&got);
    }
    text_diff("stdout", &stdout, &got, &mut out);
    text_diff("stderr", &lf(&want.stderr), &lf(&obs.stderr), &mut out);
    compare_changes(&case.expect_dir(), &want.changes, obs, &mut out);
    if case.spec.git {
        let got = obs.git_log.as_deref().unwrap_or("");
        text_diff("git-log", &lf(&want.git_log), &lf(got), &mut out);
    }
    out
}

fn compare_changes(dir: &Path, want: &[String], obs: &Observation, out: &mut Vec<Difference>) {
    let lines = |v: &[String]| v.iter().map(|l| format!("{l}\n")).collect::<String>();
    text_diff("changes", &lines(want), &lines(&obs.changes), out);
    for entry in want {
        let Some(path) = written_file(entry) else {
            continue;
        };
        if !obs.changes.contains(entry) {
            continue;
        }
        let part = format!("expect/{path}");
        let expected = match read_inside(dir, &stored_path(path)) {
            Ok(Some(b)) => b,
            Ok(None) => {
                out.push(diff(part, format!("expect/{path} is missing")));
                continue;
            }
            Err(e) => {
                out.push(diff(part, e));
                continue;
            }
        };
        let actual = obs.files.get(path).map(Vec::as_slice).unwrap_or_default();
        compare_file(&part, path, &expected, actual, out);
    }
}

/// The path of a file the run added or changed, from a change entry.
fn written_file(entry: &str) -> Option<&str> {
    let rest = entry
        .strip_prefix("+ f ")
        .or_else(|| entry.strip_prefix("~ f "))?;
    Some(rest)
}

fn compare_file(part: &str, path: &str, want: &[u8], got: &[u8], out: &mut Vec<Difference>) {
    if path.ends_with(".json")
        && let (Ok(w), Ok(g)) = (
            serde_json::from_slice::<serde_json::Value>(want),
            serde_json::from_slice::<serde_json::Value>(got),
        )
    {
        if w != g {
            let pretty =
                |v: &serde_json::Value| serde_json::to_string_pretty(v).unwrap_or_default();
            text_diff(part, &pretty(&w), &pretty(&g), out);
        }
        return;
    }
    match (std::str::from_utf8(want), std::str::from_utf8(got)) {
        (Ok(w), Ok(g)) => text_diff(part, &lf(w), &lf(g), out),
        _ if want != got => out.push(diff(part, "binary content differs")),
        _ => {}
    }
}

fn text_diff(part: &str, want: &str, got: &str, out: &mut Vec<Difference>) {
    if want == got {
        return;
    }
    let d = TextDiff::from_lines(want, got)
        .unified_diff()
        .context_radius(3)
        .header("expected", "actual")
        .to_string();
    out.push(diff(part, d));
}

/// Reads `rel` (slash-separated) under `base`, refusing a symlink at the file or any
/// folder on the way, so `expect/` can't point the comparison at an outside file.
/// `Ok(None)` when it doesn't exist.
fn read_inside(base: &Path, rel: &str) -> Result<Option<Vec<u8>>, String> {
    let mut path = base.to_path_buf();
    for part in rel.split('/') {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(format!(
                    "{} is a symlink; a case may not link outside itself",
                    path.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    std::fs::read(&path)
        .map(Some)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn lf(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn sorted_lines(s: &str) -> String {
    let mut lines: Vec<&str> = s.lines().collect();
    lines.sort_unstable();
    lines.iter().map(|l| format!("{l}\n")).collect()
}

/// Writes the case's expectation from the observation: `case.toml`'s `[expect]`
/// table (the inputs above it are kept as written) and a fresh `expect/` holding the
/// files the run added or changed. Fails, writing nothing, if the new `case.toml`
/// wouldn't read back as the same inputs and expectation.
pub fn write_expect(case: &Case, obs: &Observation) -> Result<(), Error> {
    let expect = Expect {
        exit: obs.exit,
        stdout: obs.stdout.clone(),
        stderr: obs.stderr.clone(),
        changes: obs.changes.clone(),
        git_log: if case.spec.git {
            obs.git_log.clone().unwrap_or_default()
        } else {
            String::new()
        },
    };
    let file = case.dir.join("case.toml");
    let text = format!("{}\n\n{}", case.inputs.trim_end(), expect_table(&expect));
    let mut want_spec = case.spec.clone();
    want_spec.expect = Some(expect);
    match toml::from_str::<CaseSpec>(&text) {
        Ok(spec) if spec == want_spec => {}
        Ok(_) => {
            return Err(Error::at(
                &file,
                "the rewritten case.toml reads back differently; keep [expect] as the last table",
            ));
        }
        Err(e) => {
            return Err(Error::at(
                &file,
                format!("the rewritten case.toml doesn't parse: {e}"),
            ));
        }
    }

    let dir = case.expect_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| Error::at(&dir, e))?;
    }
    for path in obs.changes.iter().filter_map(|e| written_file(e)) {
        let bytes = obs.files.get(path).map(Vec::as_slice).unwrap_or_default();
        let dst = dir.join(stored_path(path));
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::at(parent, e))?;
        }
        std::fs::write(&dst, bytes).map_err(|e| Error::at(&dst, e))?;
    }
    std::fs::write(&file, text).map_err(|e| Error::at(&file, e))
}

/// The `[expect]` table as TOML, with empty parts left out.
fn expect_table(e: &Expect) -> String {
    let mut t = format!("{EXPECT_HEADER}\nexit = {}\n", e.exit);
    for (key, value) in [
        ("stdout", &e.stdout),
        ("stderr", &e.stderr),
        ("git_log", &e.git_log),
    ] {
        if !value.is_empty() {
            t.push_str(&format!("{key} = {}\n", toml_string(value)));
        }
    }
    if !e.changes.is_empty() {
        t.push_str("changes = [\n");
        for c in &e.changes {
            t.push_str(&format!("  {},\n", basic_string(c)));
        }
        t.push_str("]\n");
    }
    t
}

/// A TOML string for `s`: a multi-line literal string when `s` has more than one line
/// and can be
/// one (no `'''`, no control characters but tab and newline), else a basic string.
fn toml_string(s: &str) -> String {
    let literal_ok = !s.contains("'''")
        && !s.ends_with('\'')
        && s.chars().all(|c| c == '\n' || c == '\t' || !c.is_control());
    let one_line = s.trim_end_matches('\n').lines().count() <= 1 && !s.ends_with("\n\n");
    if !one_line && literal_ok {
        format!("'''\n{s}'''")
    } else {
        basic_string(s)
    }
}

/// A TOML basic string, with escapes.
fn basic_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{:04X}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
