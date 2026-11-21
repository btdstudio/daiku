//! ADRs: listing the decision records under the `adrDir` setting. A port of the
//! read side of Go's `internal/plan/adr`; the `adr` commands arrive in S-0006.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::state::store::{at, shown};
use crate::text::Text;
use crate::{Error, gofmt, write};

/// One decision record.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub number: i64,
    pub slug: String,
    /// As the file holds it (`adr list` prints it).
    pub title: Text,
    pub status: String,
    pub date: String,
    pub deciders: Text,
    /// As shown: the root joined with the ADR folder and file name.
    pub path: String,
    /// The file's name.
    pub file: String,
    /// The file's bytes as they are, valid UTF-8 or not (`adr show` prints them).
    /// Serialised as a string with U+FFFD for invalid bytes, as Go's
    /// `encoding/json` writes its `Raw`.
    #[serde(serialize_with = "lossy")]
    #[schemars(with = "String")]
    pub raw: Vec<u8>,
}

fn lossy<S: serde::Serializer>(raw: &[u8], s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&String::from_utf8_lossy(raw))
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
fn parse(raw: &[u8], number: i64, slug: &str) -> Record {
    let mut rec = Record {
        number,
        slug: slug.to_string(),
        status: "proposed".into(),
        raw: raw.to_vec(),
        ..Record::default()
    };
    let lossy = |b: &[u8]| String::from_utf8_lossy(gofmt::trim_space(b)).into_owned();
    for line in raw.split(|b| *b == b'\n') {
        let t = gofmt::trim_space(line);
        if let Some(title) = t.strip_prefix(b"# ") {
            rec.title = match title.iter().position(|b| *b == b' ') {
                Some(i) => {
                    gofmt::trim_left(gofmt::trim_space(&title[i + 1..]), &['—', '-', '\t', ' '])
                        .into()
                }
                None => title.into(),
            };
        } else if let Some(v) = t.strip_prefix(b"- Status:") {
            rec.status = lossy(v);
        } else if let Some(v) = t.strip_prefix(b"- Date:") {
            rec.date = lossy(v);
        } else if let Some(v) = t.strip_prefix(b"- Deciders:") {
            rec.deciders = gofmt::trim_space(v).into();
        }
    }
    rec
}

/// What `adr new` takes. `body`, when set, replaces the three sections.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub title: String,
    pub deciders: String,
    pub context: String,
    pub decision: String,
    pub consequences: String,
    /// Bytes, as read from `--file`: kept as they are, valid UTF-8 or not.
    pub body: Vec<u8>,
}

/// Go's `slugify`: lower-case ASCII letters and digits, other runs as one `-`,
/// cut to 40 runes without a trailing `-`.
pub fn slugify(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut s = String::new();
    let mut in_run = false;
    for c in lower.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
            in_run = false;
        } else if !in_run {
            s.push('-');
            in_run = true;
        }
    }
    let s = s.trim_matches('-').to_string();
    let cut: String = if s.chars().count() > 40 {
        s.chars().take(40).collect()
    } else {
        s
    };
    cut.trim_end_matches('-').to_string()
}

/// Go's `NextNumber`: the highest `NNNN-*.md` + 1, counting malformed names so
/// their numbers are never reused.
pub fn next_number(root: &Path, adr_dir: &str) -> i64 {
    let rel = dir_rel(adr_dir);
    let Ok(entries) = std::fs::read_dir(at(root, rel)) else {
        return 1;
    };
    let mut max = 0;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(m) = ADR_NUMBER_RE.captures(&name)
            && let Ok(n) = m[1].parse::<i64>()
        {
            max = max.max(n);
        }
    }
    max + 1
}

static ADR_NUMBER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]{4})-.*\.md$").expect("valid"));

/// Go's `render`.
pub fn render(n: i64, opts: &Options, date: &str) -> Vec<u8> {
    let header = format!(
        "# {n:04} — {}\n\n- Status: proposed\n- Date: {date}\n- Deciders: {}\n\n",
        opts.title, opts.deciders
    );
    if !opts.body.is_empty() {
        let mut out = header.into_bytes();
        out.extend_from_slice(gofmt::trim_space(&opts.body));
        out.push(b'\n');
        return out;
    }
    format!(
        "{}## Context\n\n{}\n\n## Decision\n\n{}\n\n## Consequences\n\n{}\n",
        header, opts.context, opts.decision, opts.consequences
    )
    .into_bytes()
}

/// Go's `Create`: the next number, status proposed, dated `date`. It never
/// overwrites.
pub fn create(root: &Path, adr_dir: &str, opts: &Options, date: &str) -> Result<Record, Error> {
    let rel = dir_rel(adr_dir);
    write::create_dir_all(root, &at(root, rel))?;
    let n = next_number(root, adr_dir);
    let mut slug = slugify(&opts.title);
    if slug.is_empty() {
        slug = "decision".into();
    }
    let name = format!("{n:04}-{slug}.md");
    let file_rel = format!("{rel}/{name}");
    let full = at(root, &file_rel);
    if full.exists() {
        return Err(Error::AdrExists { name });
    }
    let text = render(n, opts, date);
    write::write_file(root, &full, &text)?;
    Ok(Record {
        number: n,
        slug,
        title: opts.title.as_str().into(),
        status: "proposed".into(),
        date: date.to_string(),
        deciders: opts.deciders.as_str().into(),
        path: shown(root, &file_rel),
        file: name,
        raw: text,
    })
}

/// Go's `Show`: one ADR by number.
pub fn show(root: &Path, adr_dir: &str, number: i64) -> Result<Record, Error> {
    for rec in list(root, adr_dir) {
        if rec.number == number {
            return Ok(rec);
        }
    }
    Err(Error::AdrNotFound { number })
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
        let mut rec = parse(&raw, m[1].parse().unwrap_or(0), &m[2]);
        rec.path = shown(root, &file_rel);
        rec.file = name.clone();
        out.push(rec);
    }
    out.sort_by_key(|r| r.number);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Temp;

    const DATE: &str = "2026-11-19";

    fn create_titled(t: &Temp, title: &str) -> Record {
        let opts = Options {
            title: title.into(),
            ..Options::default()
        };
        create(t.path(), "", &opts, DATE).unwrap()
    }

    // Go's TestCreateWritesNumberedADR.
    #[test]
    fn create_writes_a_numbered_adr() {
        let t = Temp::new();
        let opts = Options {
            title: "Use Postgres".into(),
            deciders: "architect".into(),
            context: "need a store".into(),
            decision: "Postgres".into(),
            consequences: "ops burden".into(),
            body: Vec::new(),
        };
        let rec = create(t.path(), "", &opts, DATE).unwrap();
        assert_eq!(
            (rec.number, rec.slug.as_str(), rec.status.as_str()),
            (1, "use-postgres", "proposed")
        );
        assert!(
            rec.path
                .ends_with("plan/04_architecture/adr/0001-use-postgres.md")
        );
        let s = t.read("plan/04_architecture/adr/0001-use-postgres.md");
        assert_eq!(
            s,
            "# 0001 — Use Postgres\n\n- Status: proposed\n- Date: 2026-11-19\n- Deciders: architect\n\n## Context\n\nneed a store\n\n## Decision\n\nPostgres\n\n## Consequences\n\nops burden\n"
        );
    }

    // Go's TestNumbersAreSequential.
    #[test]
    fn numbers_are_sequential() {
        let t = Temp::new();
        let n: Vec<i64> = ["a", "b", "c"]
            .iter()
            .map(|x| create_titled(&t, x).number)
            .collect();
        assert_eq!(n, [1, 2, 3]);
    }

    // Go's TestListSortedSkippingNonNumeric.
    #[test]
    fn list_sorted_skipping_non_numeric() {
        let t = Temp::new();
        create_titled(&t, "first");
        create_titled(&t, "second");
        t.write("plan/04_architecture/adr/README.md", "x");
        let n: Vec<i64> = list(t.path(), "").iter().map(|r| r.number).collect();
        assert_eq!(n, [1, 2]);
    }

    // Go's TestShowAndMissing.
    #[test]
    fn show_and_missing() {
        let t = Temp::new();
        create_titled(&t, "T");
        assert_eq!(show(t.path(), "", 1).unwrap().title, "T");
        let e = show(t.path(), "", 99).unwrap_err().to_string();
        assert_eq!(e, "daiku: adr 0099 not found");
        // Go's %04d keeps the sign.
        let e = show(t.path(), "", -1).unwrap_err().to_string();
        assert_eq!(e, "daiku: adr -001 not found");
    }

    // Go's TestParseExtractsFields.
    #[test]
    fn parse_extracts_fields() {
        let raw = "# 0003 — Split the monolith\n\n- Status: accepted\n- Date: 2026-01-02\n- Deciders: architect\n\n## Context\n\n## Decision\n\n## Consequences\n";
        let rec = parse(raw.as_bytes(), 3, "split-the-monolith");
        assert_eq!(rec.title, "Split the monolith");
        assert_eq!(
            (
                rec.status.as_str(),
                rec.date.as_str(),
                &*rec.deciders.to_string_lossy()
            ),
            ("accepted", "2026-01-02", "architect")
        );
        assert_eq!(rec.raw, raw.as_bytes());
    }

    // Go's TestNextNumberEmpty and TestAdrDirDefault.
    #[test]
    fn next_number_empty_and_default_dir() {
        let t = Temp::new();
        assert_eq!(next_number(t.path(), ""), 1);
        assert_eq!(dir_rel(""), "plan/04_architecture/adr");
    }

    // Go's TestCreateWithBody: the body replaces the sections, trimmed.
    #[test]
    fn create_with_body() {
        let t = Temp::new();
        let opts = Options {
            title: "Big decision".into(),
            deciders: "architect".into(),
            body: "\n## Context\n\nwhy\n\n## Decision\n\nwhat\n\n".into(),
            ..Options::default()
        };
        create(t.path(), "", &opts, DATE).unwrap();
        let s = t.read("plan/04_architecture/adr/0001-big-decision.md");
        assert!(
            s.ends_with("- Deciders: architect\n\n## Context\n\nwhy\n\n## Decision\n\nwhat\n"),
            "{s}"
        );
    }

    // Go's TestSlugifyTrimsTrailingDashAfterCut.
    #[test]
    fn slugify_trims_a_trailing_dash_after_the_cut() {
        let long = format!("{} then more words", "a".repeat(39));
        for (title, want) in [
            (long.as_str(), "a".repeat(39)),
            (&"b".repeat(45), "b".repeat(40)),
            (
                "Canonical ticket content hash for freeze!",
                "canonical-ticket-content-hash-for-freeze".into(),
            ),
            ("Use Postgres", "use-postgres".into()),
            ("Café crème", "caf-cr-me".into()),
            ("!!!", String::new()),
        ] {
            assert_eq!(slugify(title), want, "{title:?}");
        }
    }

    // Go's TestCreateCutSlugMatchesAndNumbersOn.
    #[test]
    fn a_cut_slug_matches_and_numbers_on() {
        let t = Temp::new();
        let long = format!("{} then more words", "a".repeat(39));
        let a = create_titled(&t, &long);
        assert_eq!(a.file, format!("0001-{}.md", "a".repeat(39)));
        assert!(ADR_FILE_RE.is_match(&a.file));
        let b = create_titled(&t, &format!("{long} again"));
        assert_eq!((b.number, list(t.path(), "").len()), (2, 2));
    }

    // Go's TestNextNumberCountsMalformedNames.
    #[test]
    fn next_number_counts_malformed_names() {
        let t = Temp::new();
        t.write(
            "plan/04_architecture/adr/0003-some-title-.md",
            "# 0003 — Some title\n",
        );
        t.write("plan/04_architecture/adr/0009-notes.txt", "x");
        assert_eq!(create_titled(&t, "Next").number, 4);
        let n: Vec<i64> = list(t.path(), "").iter().map(|r| r.number).collect();
        assert_eq!(n, [4]);
    }

    /// An empty slug falls back to `decision`.
    #[test]
    fn create_falls_back_to_decision() {
        let t = Temp::new();
        assert_eq!(create_titled(&t, "!!!").file, "0001-decision.md");
    }

    /// Review finding 1: a `--file` body that isn't valid UTF-8 is written as
    /// it is, trimmed as Go's `TrimSpace` trims, and `show` returns its bytes.
    #[test]
    fn a_body_keeps_its_bytes() {
        let t = Temp::new();
        let opts = Options {
            title: "Latin".into(),
            body: b"\xc2\xa0\n Decision \xe9\r\n\t\n".to_vec(),
            ..Options::default()
        };
        create(t.path(), "", &opts, DATE).unwrap();
        let written = std::fs::read(t.p("plan/04_architecture/adr/0001-latin.md")).unwrap();
        assert!(
            written.ends_with(b"- Deciders: \n\nDecision \xe9\n"),
            "{written:?}"
        );
        assert_eq!(show(t.path(), "", 1).unwrap().raw, written);
    }

    /// N-0001: a title and deciders that aren't valid UTF-8 keep their bytes;
    /// the title's leading dashes go, as Go's `TrimLeft` takes them.
    #[test]
    fn parse_keeps_bytes() {
        let rec = parse(
            b"# 0004 \xe2\x80\x94 Caf\xe9 choice\n\n- Status: accepted\n- Deciders: Ren\xe9e\n",
            4,
            "x",
        );
        assert_eq!(rec.title.as_bytes(), b"Caf\xe9 choice");
        assert_eq!(rec.deciders.as_bytes(), b"Ren\xe9e");
        assert_eq!(rec.status, "accepted");
        let t = Temp::new();
        t.write("plan/04_architecture/adr/0001-x.md", "");
        std::fs::write(
            t.p("plan/04_architecture/adr/0001-x.md"),
            b"# 0001 - D\xe9\n",
        )
        .unwrap();
        assert_eq!(list(t.path(), "")[0].title.as_bytes(), b"D\xe9");
    }
}
