//! Comparing an observation with `expect/` (R7), and writing `expect/` (D13).

use std::path::Path;

use similar::TextDiff;

use crate::stored::stored_path;
use crate::{Case, Error, Observation};

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

/// Compares the observation with the case's `expect/`, collecting every difference.
pub fn compare(case: &Case, obs: &Observation) -> Vec<Difference> {
    let dir = case.expect_dir();
    if !dir.is_dir() {
        return vec![diff("expect", "no expect/ folder; run capture")];
    }
    let mut out = Vec::new();
    let read = |name: &str, out: &mut Vec<Difference>| match read_inside(&dir, name) {
        Ok(Some(b)) => Some(b),
        Ok(None) => {
            out.push(diff(name, format!("expect/{name} is missing")));
            None
        }
        Err(e) => {
            out.push(diff(name, e));
            None
        }
    };

    if let Some(b) = read("exit", &mut out) {
        let want = String::from_utf8_lossy(&b).trim().to_string();
        let got = obs.exit.to_string();
        if want != got {
            out.push(diff("exit", format!("expected {want}, got {got}")));
        }
    }
    if let Some(b) = read("stdout", &mut out) {
        let (mut want, mut got) = (lf(&String::from_utf8_lossy(&b)), lf(&obs.stdout));
        if case.spec.unordered_stdout {
            want = sorted_lines(&want);
            got = sorted_lines(&got);
        }
        text_diff("stdout", &want, &got, &mut out);
    }
    if let Some(b) = read("stderr", &mut out) {
        text_diff(
            "stderr",
            &lf(&String::from_utf8_lossy(&b)),
            &lf(&obs.stderr),
            &mut out,
        );
    }
    if let Some(b) = read("changes.txt", &mut out) {
        let want: Vec<String> = String::from_utf8_lossy(&b)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect();
        compare_changes(&dir, &want, obs, &mut out);
    }
    if case.spec.git
        && let Some(b) = read("git-log", &mut out)
    {
        let got = obs.git_log.as_deref().unwrap_or("");
        text_diff(
            "git-log",
            &lf(&String::from_utf8_lossy(&b)),
            &lf(got),
            &mut out,
        );
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
        let part = format!("tree/{path}");
        let expected = match read_inside(dir, &format!("tree/{}", stored_path(path))) {
            Ok(Some(b)) => b,
            Ok(None) => {
                out.push(diff(part, format!("expect/tree/{path} is missing")));
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

/// Deletes the case's `expect/` and writes it fresh from the observation.
pub fn write_expect(case: &Case, obs: &Observation) -> Result<(), Error> {
    let dir = case.expect_dir();
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| Error::at(&dir, e))?;
    }
    let put = |rel: &str, bytes: &[u8]| -> Result<(), Error> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::at(parent, e))?;
        }
        std::fs::write(&path, bytes).map_err(|e| Error::at(&path, e))
    };
    put("exit", format!("{}\n", obs.exit).as_bytes())?;
    put("stdout", obs.stdout.as_bytes())?;
    put("stderr", obs.stderr.as_bytes())?;
    let changes: String = obs.changes.iter().map(|e| format!("{e}\n")).collect();
    put("changes.txt", changes.as_bytes())?;
    for path in obs.changes.iter().filter_map(|e| written_file(e)) {
        let bytes = obs.files.get(path).map(Vec::as_slice).unwrap_or_default();
        put(&format!("tree/{}", stored_path(path)), bytes)?;
    }
    if case.spec.git {
        put("git-log", obs.git_log.as_deref().unwrap_or("").as_bytes())?;
    }
    Ok(())
}
