//! Intake sources: the store under the `sourceDir` setting, one `<id>.md` each. A
//! port of Go's `internal/plan/source`.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::state::store::{at, read_file, shown};
use crate::{Error, gofmt, write};

pub const ISSUE: &str = "issue";
pub const REPO: &str = "repo";

/// One intake source.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub id: i64,
    pub title: String,
    /// `issue` or `repo`.
    pub source: String,
    /// A path or URL when the source is a repo.
    pub repo: String,
    /// An optional pin.
    pub rev: String,
    /// The idea this was promoted from, 0 for none.
    pub idea: i64,
    pub body: String,
    /// Where it was read from or written to, as shown.
    pub path: String,
}

static SOURCE_FILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+)\.md$").expect("valid pattern"));
static TITLE_WS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\t\n\x0C\r ]*\r?\n+[\t\n\x0C\r ]*").expect("valid pattern"));

fn dir_rel(source_dir: &str) -> &str {
    if source_dir.is_empty() {
        "plan/00_intake"
    } else {
        source_dir
    }
}

/// Go's `ParseFrontmatter`: `key: value` lines between `---` lines, keys lower
/// cased; the body without one leading and one trailing newline.
pub fn parse_frontmatter(raw: &str) -> (std::collections::HashMap<String, String>, String) {
    let raw = raw.replace("\r\n", "\n");
    let lines: Vec<&str> = raw.split('\n').collect();
    let mut fm = std::collections::HashMap::new();
    if lines[0].trim() != "---" {
        return (fm, raw);
    }
    let Some(end) = lines
        .iter()
        .skip(1)
        .position(|l| *l == "---")
        .map(|i| i + 1)
    else {
        return (fm, raw);
    };
    if end < 2 {
        return (fm, raw);
    }
    for line in &lines[1..end] {
        if let Some((k, v)) = line.split_once(':') {
            fm.insert(go_lower(k.trim()), v.trim().to_string());
        }
    }
    let body = lines[end + 1..].join("\n");
    let body = body.strip_prefix('\n').unwrap_or(&body);
    let body = body.strip_suffix('\n').unwrap_or(body);
    (fm, body.to_string())
}

/// Go's `strings.ToLower`: rune by rune, with the simple mappings only, so `İ`
/// (whose full lower case is two runes) lowers to `i`.
fn go_lower(s: &str) -> String {
    s.chars()
        .map(|c| {
            let mut l = c.to_lowercase();
            match (l.next(), l.next()) {
                (Some(one), None) => one,
                (Some(first), Some(_)) => first,
                _ => c,
            }
        })
        .collect()
}

/// Go's `strconv.Atoi`, 0 when it fails.
fn atoi(s: &str) -> i64 {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    s.parse().unwrap_or(0)
}

/// Go's `ParseSource`.
pub fn parse_source(raw: &str, fallback_id: i64) -> Source {
    let (fm, body) = parse_frontmatter(raw);
    let get = |k: &str| fm.get(k).cloned().unwrap_or_default();
    let id = match atoi(&get("id")) {
        0 => fallback_id,
        n => n,
    };
    Source {
        id,
        title: get("title"),
        source: if get("source") == REPO { REPO } else { ISSUE }.to_string(),
        repo: get("repo"),
        rev: get("rev"),
        idea: atoi(&get("idea")),
        body,
        path: String::new(),
    }
}

/// Go's `ToFrontmatter`.
pub fn to_frontmatter(s: &Source) -> String {
    let title = TITLE_WS_RE.replace_all(&s.title, " ");
    let title = title.trim_matches(|c: char| c.is_whitespace());
    let mut out = format!("---\nid: {}\ntitle: {title}\nsource: {}", s.id, s.source);
    if !s.repo.is_empty() {
        out.push_str(&format!("\nrepo: {}", s.repo));
    }
    if !s.rev.is_empty() {
        out.push_str(&format!("\nrev: {}", s.rev));
    }
    if s.idea > 0 {
        out.push_str(&format!("\nidea: {}", s.idea));
    }
    out.push_str(&format!("\n---\n\n{}", s.body));
    out
}

fn max_file_id(root: &Path, source_dir: &str) -> i64 {
    let Ok(entries) = std::fs::read_dir(at(root, dir_rel(source_dir))) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            SOURCE_FILE_RE
                .captures(&name)
                .and_then(|m| m[1].parse::<i64>().ok())
        })
        .fold(0, i64::max)
}

/// Go's `MaxRecordedId`: the highest intake id among recorded source paths that
/// sit directly in the source folder.
pub fn max_recorded_id(source_dir: &str, recorded: &[String]) -> i64 {
    let dir = gofmt::clean(&gofmt::to_slash(dir_rel(source_dir)));
    let mut max = 0;
    for r in recorded {
        let p = gofmt::clean(&gofmt::to_slash(r));
        let (parent, base) = match p.rsplit_once('/') {
            Some((d, b)) => (if d.is_empty() { "/" } else { d }, b),
            None => (".", p.as_str()),
        };
        if parent != dir {
            continue;
        }
        if let Some(n) = SOURCE_FILE_RE
            .captures(base)
            .and_then(|m| m[1].parse::<i64>().ok())
        {
            max = max.max(n);
        }
    }
    max
}

/// Go's `ListSources`: every `<n>.md` in the folder, by id. A file that can't be
/// read is skipped; a missing folder is empty.
pub fn list(root: &Path, source_dir: &str) -> Vec<Source> {
    let dir = dir_rel(source_dir);
    let Ok(entries) = std::fs::read_dir(at(root, dir)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(m) = SOURCE_FILE_RE.captures(&name) else {
            continue;
        };
        // Go's Atoi: a number too large for an id reads as 0.
        let id: i64 = m[1].parse().unwrap_or(0);
        let rel = format!("{dir}/{name}");
        let path = shown(root, &rel);
        let raw = match read_file(&at(root, &rel)) {
            Ok(Some(r)) => r,
            _ => continue,
        };
        let mut s = parse_source(&String::from_utf8_lossy(&raw), id);
        s.path = path;
        out.push(s);
    }
    out.sort_by_key(|s| s.id);
    out
}

/// Go's `ReadSource`.
pub fn read_source(root: &Path, source_dir: &str, id: i64) -> Result<Source, Error> {
    let rel = format!("{}/{id}.md", dir_rel(source_dir));
    let path = shown(root, &rel);
    match read_file(&at(root, &rel)) {
        Ok(Some(raw)) => {
            let mut s = parse_source(&String::from_utf8_lossy(&raw), id);
            s.path = path;
            Ok(s)
        }
        _ => Err(Error::SourceNotFound { id, path }),
    }
}

/// What a new intake holds.
#[derive(Debug, Clone, Default)]
pub struct NewSource {
    pub title: String,
    pub source: String,
    pub repo: String,
    pub rev: String,
    pub body: String,
    pub idea: i64,
    /// Ids up to this one are taken even without a file (recorded by a change).
    pub after: i64,
}

/// Go's `WriteNewSource`: the next id past both the files and `after`, written
/// without overwriting.
pub fn write_new(root: &Path, source_dir: &str, opts: &NewSource) -> Result<Source, Error> {
    let hi = max_file_id(root, source_dir).max(opts.after);
    if hi == i64::MAX {
        return Err(Error::IntakeIdExhausted { highest: hi });
    }
    let s = Source {
        id: hi + 1,
        title: opts.title.clone(),
        source: if opts.source == REPO { REPO } else { ISSUE }.to_string(),
        repo: opts.repo.clone(),
        rev: opts.rev.clone(),
        idea: opts.idea,
        body: opts.body.clone(),
        path: String::new(),
    };
    write_source(root, source_dir, &s)
}

/// Go's `WriteSource`: creates `<id>.md`, refusing to overwrite.
pub fn write_source(root: &Path, source_dir: &str, s: &Source) -> Result<Source, Error> {
    let dir_rel = dir_rel(source_dir);
    write::create_dir_all(root, &at(root, dir_rel))?;
    let rel = format!("{dir_rel}/{}.md", s.id);
    let path = shown(root, &rel);
    let full = at(root, &rel);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    let mut f = match write::open_file(root, &full, &opts) {
        Err(Error::Io { source, .. }) if source.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(Error::SourceExists { id: s.id, path });
        }
        r => r?,
    };
    use std::io::Write;
    if let Err(source) = f.write_all(format!("{}\n", to_frontmatter(s)).as_bytes()) {
        drop(f);
        let _ = std::fs::remove_file(&full);
        return Err(Error::Io {
            op: "write",
            path,
            source,
        });
    }
    drop(f);
    let raw = std::fs::read(&full).unwrap_or_default();
    let mut written = parse_source(&String::from_utf8_lossy(&raw), s.id);
    written.path = path;
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Temp;

    #[test]
    fn writes_and_reads_an_intake() {
        let t = Temp::new();
        let opts = NewSource {
            title: "  Auth\n  flow ".into(),
            source: REPO.into(),
            repo: "../api".into(),
            rev: "v1".into(),
            body: "Why.".into(),
            ..NewSource::default()
        };
        let s = write_new(t.path(), "", &opts).unwrap();
        assert_eq!(s.id, 1);
        assert_eq!(
            t.read("plan/00_intake/1.md"),
            "---\nid: 1\ntitle: Auth flow\nsource: repo\nrepo: ../api\nrev: v1\n---\n\nWhy.\n"
        );
        assert_eq!((s.title.as_str(), s.body.as_str()), ("Auth flow", "Why."));
        let next = write_new(
            t.path(),
            "",
            &NewSource {
                title: "x".into(),
                after: 6,
                ..NewSource::default()
            },
        )
        .unwrap();
        assert_eq!(next.id, 7);
        assert_eq!(
            t.read("plan/00_intake/7.md"),
            "---\nid: 7\ntitle: x\nsource: issue\n---\n\n\n"
        );
        assert_eq!(read_source(t.path(), "", 7).unwrap().source, ISSUE);
        assert_eq!(
            write_source(
                t.path(),
                "",
                &Source {
                    id: 7,
                    ..Source::default()
                }
            )
            .unwrap_err()
            .to_string(),
            format!(
                "daiku: source 7 already exists at {}; not overwriting",
                shown(t.path(), "plan/00_intake/7.md")
            )
        );
        assert_eq!(
            read_source(t.path(), "", 9).unwrap_err().to_string(),
            format!(
                "daiku: source 9 not found at {}",
                shown(t.path(), "plan/00_intake/9.md")
            )
        );
    }

    #[test]
    fn recorded_ids_count_only_in_the_folder() {
        let rec = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            max_recorded_id(
                "",
                &rec(&[
                    "plan/00_intake/3.md",
                    "plan/00_intake/x/9.md",
                    "other/8.md",
                    "./plan/00_intake/5.md"
                ])
            ),
            5
        );
        assert_eq!(
            max_recorded_id("intake", &rec(&["intake/2.md", "plan/00_intake/5.md"])),
            2
        );
    }

    #[test]
    fn parses_frontmatter_as_go() {
        let (fm, body) =
            parse_frontmatter("---\r\nID: 4\r\nTitle: a: b\r\n---\r\n\r\nbody\r\n\r\n");
        assert_eq!(fm.get("id").map(String::as_str), Some("4"));
        assert_eq!(fm.get("title").map(String::as_str), Some("a: b"));
        assert_eq!(body, "body\n");
        let (fm, body) = parse_frontmatter("---\n---\nx");
        assert!(fm.is_empty());
        assert_eq!(body, "---\n---\nx");
    }

    /// Go's `ListSources`: every `<n>.md` by id, skipping other names; a missing
    /// folder is empty.
    #[test]
    fn list_orders_by_id_and_skips_other_files() {
        let t = Temp::new();
        assert!(list(t.path(), "").is_empty());
        t.write(
            "plan/00_intake/10.md",
            "---\ntitle: Ten\nsource: issue\n---\n",
        );
        t.write(
            "plan/00_intake/2.md",
            "---\ntitle: Two\nsource: repo\nrepo: ../app\n---\n",
        );
        t.write("plan/00_intake/context.md", "the guide");
        t.write("plan/00_intake/3.txt", "not a source");
        let got: Vec<(i64, String, String)> = list(t.path(), "")
            .into_iter()
            .map(|s| (s.id, s.title, s.source))
            .collect();
        assert_eq!(
            got,
            [
                (2, "Two".into(), REPO.into()),
                (10, "Ten".into(), ISSUE.into())
            ]
        );
        assert_eq!(
            list(t.path(), "")[0].path,
            shown(t.path(), "plan/00_intake/2.md")
        );
    }
}
