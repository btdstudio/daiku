//! ADRs: listing the decision records under the `adrDir` setting. A port of the
//! read side of Go's `internal/plan/adr`; the `adr` commands arrive in S-0006.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::state::store::{at, shown};

/// One decision record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    pub number: u32,
    pub slug: String,
    pub title: String,
    pub status: String,
    pub date: String,
    pub deciders: String,
    /// As shown: the root joined with the ADR folder and file name.
    pub path: String,
    /// The file's name.
    pub file: String,
}

static ADR_FILE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([0-9]{4})-([a-z0-9]+(?:-[a-z0-9]+)*)\.md$").expect("valid pattern")
});

fn dir_rel(adr_dir: &str) -> &str {
    if adr_dir.is_empty() {
        "plan/04_architecture/adr"
    } else {
        adr_dir
    }
}

/// Go's `parse`: the title from `# NNNN — Title`, and the status, date and
/// deciders lines.
fn parse(raw: &str, number: u32, slug: &str) -> Record {
    let mut rec = Record {
        number,
        slug: slug.to_string(),
        status: "proposed".into(),
        ..Record::default()
    };
    for line in raw.split('\n') {
        let t = line.trim();
        if let Some(title) = t.strip_prefix("# ") {
            rec.title = match title.find(' ') {
                Some(i) => title[i + 1..]
                    .trim()
                    .trim_start_matches(['—', '-', '\t', ' '])
                    .to_string(),
                None => title.to_string(),
            };
        } else if let Some(v) = t.strip_prefix("- Status:") {
            rec.status = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("- Date:") {
            rec.date = v.trim().to_string();
        } else if let Some(v) = t.strip_prefix("- Deciders:") {
            rec.deciders = v.trim().to_string();
        }
    }
    rec
}

/// Go's `List`: every `NNNN-slug.md` in the folder, by number.
pub fn list(root: &Path, adr_dir: &str) -> Vec<Record> {
    let rel = dir_rel(adr_dir);
    let Ok(entries) = std::fs::read_dir(at(root, rel)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let Some(m) = ADR_FILE_RE.captures(&name) else {
            continue;
        };
        let file_rel = format!("{rel}/{name}");
        let Ok(raw) = std::fs::read(at(root, &file_rel)) else {
            continue;
        };
        let mut rec = parse(
            &String::from_utf8_lossy(&raw),
            m[1].parse().unwrap_or(0),
            &m[2],
        );
        rec.path = shown(root, &file_rel);
        rec.file = name.clone();
        out.push(rec);
    }
    out.sort_by_key(|r| r.number);
    out
}
