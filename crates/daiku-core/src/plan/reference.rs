//! The references check: `references/index.md` against the `references/` folder.
//! A port of Go's `internal/plan/reference`. The index is a user-owned markdown
//! file, so it's parsed leniently: anything that isn't an entry is ignored.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::state::store::{at, read_file, shown};
use crate::{Error, gofmt};

/// The references room, relative to the workspace root.
pub const DIR: &str = "references";

/// The fields every index entry must have.
pub const REQUIRED_FIELDS: [&str; 3] = ["purpose", "from", "added"];

static ENTRY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^- `([^`]+)`(.*)$").expect("valid"));
static FIELD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s+- (purpose|from|added|summary):[ \t]*(.*)$").expect("valid"));

/// One reference in the index.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Relative to `references/`, cleaned, slash-separated.
    pub path: String,
    /// Optional.
    pub title: String,
    /// `purpose`, `from`, `added`, `summary`.
    pub fields: BTreeMap<String, String>,
    /// The 1-based line of the entry in the index.
    pub line: usize,
}

/// One warning from [`check`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub message: String,
}

/// What [`check`] found. `present` is false when there's no `references/` folder.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub present: bool,
    /// Files in the room, not counting `index.md`, `context.md` and dotfiles.
    pub files: usize,
    /// How many of them the index lists.
    pub indexed: usize,
    pub findings: Vec<Finding>,
}

/// Go's `ParseIndex`: a top-level list item whose first element is a backticked
/// path, then an optional `— title`, followed by indented `- purpose:`,
/// `- from:`, `- added:` and `- summary:` items. Other lines are ignored, and a
/// non-indented, non-blank line ends an entry.
pub fn parse_index(text: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut cur: Option<usize> = None;
    for (i, line) in text.split('\n').enumerate() {
        let line = line.trim_end_matches('\r');
        if let Some(m) = ENTRY_RE.captures(line) {
            let title = m[2].trim().trim_start_matches(['—', '–', '-']).trim();
            entries.push(Entry {
                path: gofmt::clean(&gofmt::to_slash(m[1].trim())),
                title: title.to_string(),
                fields: BTreeMap::new(),
                line: i + 1,
            });
            cur = Some(entries.len() - 1);
            continue;
        }
        if let (Some(m), Some(c)) = (FIELD_RE.captures(line), cur) {
            entries[c]
                .fields
                .insert(m[1].to_string(), m[2].trim().to_string());
            continue;
        }
        if !line.trim().is_empty() && !line.starts_with([' ', '\t']) {
            cur = None;
        }
    }
    entries
}

/// Go's `Check`: warns about each file the index doesn't list, each entry whose
/// file is missing, each entry missing a required field (an empty value counts
/// as missing), and each entry whose path is absolute or leaves `references/`,
/// which isn't looked up. With no `index.md`, every file is unindexed. With no
/// `references/` folder it finds nothing and `present` is false.
pub fn check(root: &Path) -> Result<Check, Error> {
    let dir = at(root, DIR);
    match std::fs::metadata(&dir) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => return Ok(Check::default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Check::default()),
        Err(source) => {
            return Err(Error::Io {
                op: "stat",
                path: shown(root, DIR),
                source,
            });
        }
    }
    let mut res = Check {
        present: true,
        ..Check::default()
    };

    let mut files = BTreeSet::new();
    // Go's WalkDir doesn't follow a symlinked root, or any symlink below it.
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.is_dir()) {
        walk(root, "", &mut files)?;
    }

    let index = read_file(&at(root, &format!("{DIR}/index.md")))
        .map_err(|text| Error::PathError { text })?
        .map(|d| String::from_utf8_lossy(&d).into_owned())
        .unwrap_or_default();
    let mut listed = BTreeSet::new();
    for e in parse_index(&index) {
        // Go's path.IsAbs: slash-separated, on every OS.
        if e.path.starts_with('/') || e.path == ".." || e.path.starts_with("../") {
            res.findings.push(Finding {
                message: format!("{}: outside {DIR}/", e.path),
            });
            continue;
        }
        for field in REQUIRED_FIELDS {
            if e.fields.get(field).is_none_or(String::is_empty) {
                res.findings.push(Finding {
                    message: format!("{}: missing {field}", e.path),
                });
            }
        }
        if std::fs::metadata(at(root, &format!("{DIR}/{}", e.path))).is_err() {
            res.findings.push(Finding {
                message: format!("{}: listed in the index, but the file is missing", e.path),
            });
        }
        listed.insert(e.path);
    }

    res.files = files.len();
    for name in files {
        if listed.contains(&name) {
            res.indexed += 1;
        } else {
            res.findings.push(Finding {
                message: format!("{name}: not in {DIR}/index.md"),
            });
        }
    }
    Ok(res)
}

/// The files under `references/<rel>`, as Go's `WalkDir` visits them: dotfiles
/// and dot-folders are skipped, a symlink is a file and never followed, and the
/// top-level `index.md` and `context.md` aren't references.
fn walk(root: &Path, rel: &str, files: &mut BTreeSet<String>) -> Result<(), Error> {
    let shown_rel = if rel.is_empty() {
        DIR.to_string()
    } else {
        format!("{DIR}/{rel}")
    };
    let entries = std::fs::read_dir(at(root, &shown_rel)).map_err(|source| Error::Io {
        op: "open",
        path: shown(root, &shown_rel),
        source,
    })?;
    let mut names: Vec<(String, bool)> = Vec::new();
    for e in entries {
        let e = e.map_err(|source| Error::Io {
            op: "readdirent",
            path: shown(root, &shown_rel),
            source,
        })?;
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        names.push((e.file_name().to_string_lossy().into_owned(), is_dir));
    }
    names.sort();
    for (name, is_dir) in names {
        if name.starts_with('.') {
            continue;
        }
        let child = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if is_dir {
            walk(root, &child, files)?;
        } else if child != "index.md" && child != "context.md" {
            files.insert(child);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Temp;

    const GOOD_ENTRY: &str = "  - purpose: p\n  - from: me\n  - added: 2026-11-12\n";

    fn messages(res: &Check) -> Vec<&str> {
        res.findings.iter().map(|f| f.message.as_str()).collect()
    }

    // Go's TestParseIndex.
    #[test]
    fn parse_index_reads_entries_and_fields() {
        let text = concat!(
            "# References\n\nSome prose.\n\n",
            "- `design/a.md` — Design A\n",
            "  - purpose: why it matters\n",
            "  - from: Brian\n",
            "  - added: 2026-11-12\n",
            "  - summary: a short one\n",
            "  - colour: ignored\n",
            "\n## More\n\n",
            "- `b.png`\n",
            "  - purpose: a picture\n",
            "- `./c//d.txt` - Plain title\n",
            "  - from: AI chat, 2026-11-10\n",
            "- not an entry\n",
            "  - purpose: orphan field after a plain item\n",
        );
        let got = parse_index(text);
        assert_eq!(got.len(), 3, "{got:?}");
        let a = &got[0];
        assert_eq!(
            (a.path.as_str(), a.title.as_str(), a.line),
            ("design/a.md", "Design A", 5)
        );
        assert_eq!(a.fields["purpose"], "why it matters");
        assert_eq!(a.fields["from"], "Brian");
        assert_eq!(a.fields["added"], "2026-11-12");
        assert_eq!(a.fields["summary"], "a short one");
        assert_eq!(a.fields.len(), 4);
        let b = &got[1];
        assert_eq!((b.path.as_str(), b.title.as_str()), ("b.png", ""));
        assert_eq!(b.fields["purpose"], "a picture");
        assert_eq!(b.fields.len(), 1);
        let c = &got[2];
        assert_eq!(
            (c.path.as_str(), c.title.as_str()),
            ("c/d.txt", "Plain title")
        );
        assert_eq!(c.fields["from"], "AI chat, 2026-11-10");
        assert_eq!(c.fields.len(), 1);
        assert!(parse_index("").is_empty());
        assert!(parse_index("just words\n- plain\n").is_empty());
    }

    /// `\r\n` endings parse the same as `\n`.
    #[test]
    fn parse_index_trims_carriage_returns() {
        let got = parse_index("- `a.md` — A\r\n  - purpose: p\r\n");
        assert_eq!(
            (got[0].title.as_str(), got[0].fields["purpose"].as_str()),
            ("A", "p")
        );
    }

    // Go's TestCheck.
    #[test]
    fn check_finds_unlisted_and_missing_files() {
        let t = Temp::new();
        t.write(
            "references/index.md",
            &format!(
                "# Index\n\n- `notes.md` — Notes\n{GOOD_ENTRY}- `design/diagram.png` — Diagram\n{GOOD_ENTRY}  - summary: a flow with three boxes\n- `gone.md` — Gone\n{GOOD_ENTRY}"
            ),
        );
        t.write("references/context.md", "the guide");
        t.write("references/notes.md", "notes");
        t.write("references/design/diagram.png", "png");
        t.write("references/stray.pdf", "pdf");
        t.write("references/.DS_Store", "x");
        t.write("references/.hidden/inside.md", "x");
        let res = check(t.path()).unwrap();
        assert_eq!(
            messages(&res),
            [
                "gone.md: listed in the index, but the file is missing",
                "stray.pdf: not in references/index.md",
            ]
        );
        assert!(res.present);
        assert_eq!((res.files, res.indexed), (3, 2));
    }

    // Go's TestCheckFieldsAndMissingIndex.
    #[test]
    fn check_fields_and_missing_index() {
        let t = Temp::new();
        t.write(
            "references/index.md",
            "- `a.md`\n  - purpose: p\n  - added: 2026-11-12\n- `b.md`\n  - purpose:\n  - from: me\n  - added: x\n",
        );
        t.write("references/a.md", "a");
        t.write("references/b.md", "b");
        let res = check(t.path()).unwrap();
        assert_eq!(
            messages(&res),
            ["a.md: missing from", "b.md: missing purpose"]
        );

        let bare = Temp::new();
        bare.write("references/context.md", "guide");
        bare.write("references/one.md", "1");
        bare.write("references/sub/two.md", "2");
        bare.write(
            "references/sub/index.md",
            "a nested index.md is a user file",
        );
        let res = check(bare.path()).unwrap();
        assert_eq!(
            messages(&res),
            [
                "one.md: not in references/index.md",
                "sub/index.md: not in references/index.md",
                "sub/two.md: not in references/index.md",
            ]
        );
        assert_eq!((res.files, res.indexed), (3, 0));
    }

    // Go's TestCheckOutsideAndAbsentFolder.
    #[test]
    fn check_outside_and_absent_folder() {
        let t = Temp::new();
        let res = check(t.path()).unwrap();
        assert!(!res.present && res.findings.is_empty());
        t.write("specs/secret.md", "not a reference");
        t.write(
            "references/index.md",
            &format!("- `../specs/secret.md`\n{GOOD_ENTRY}- `/etc/passwd`\n{GOOD_ENTRY}- `..`\n{GOOD_ENTRY}"),
        );
        let got = messages(&check(t.path()).unwrap()).join("|");
        for want in [
            "../specs/secret.md: outside references/",
            "/etc/passwd: outside references/",
            "..: outside references/",
        ] {
            assert!(got.contains(want), "{got} lacks {want}");
        }
        assert!(
            !got.contains("missing"),
            "an outside path was looked up: {got}"
        );
    }

    /// A symlinked folder under `references/` is one file, as Go's `WalkDir` sees
    /// it, and is never walked into.
    #[cfg(unix)]
    #[test]
    fn check_does_not_follow_symlinks() {
        let t = Temp::new();
        t.write("outside/a.md", "a");
        t.write("outside/b.md", "b");
        t.write("references/one.md", "1");
        std::os::unix::fs::symlink(t.p("outside"), t.p("references/linked")).unwrap();
        std::os::unix::fs::symlink(t.p("references"), t.p("references/loop")).unwrap();
        let res = check(t.path()).unwrap();
        assert_eq!(
            messages(&res),
            [
                "linked: not in references/index.md",
                "loop: not in references/index.md",
                "one.md: not in references/index.md",
            ]
        );
    }

    /// An index that's a folder is Go's read error, not a finding. Unix only, as
    /// `idea::tests::a_summary_that_is_a_folder_is_a_read_error`.
    #[cfg(unix)]
    #[test]
    fn an_index_that_is_a_folder_is_a_read_error() {
        let t = Temp::new();
        t.write("references/index.md/x", "x");
        let e = check(t.path()).unwrap_err().to_string();
        let want = format!(
            "read {}: is a directory",
            shown(t.path(), "references/index.md")
        );
        assert_eq!(e, want);
    }
}
