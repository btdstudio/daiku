//! Ideas: the `ideas/` room, one `ideas/<n>-<slug>/` folder per idea, with a
//! `summary.md` whose frontmatter holds the idea's status and an append-only
//! `log.md`. A port of Go's `internal/plan/idea`.
//!
//! They're plain files a person can edit without daiku. The store rewrites only
//! the frontmatter lines it owns, never an idea's body, and never removes a
//! folder: ideas are dropped, not deleted.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use crate::state::store::{at, read_all, shown};
use crate::text::Text;
use crate::{Error, plan::source, scaffold, state, write};

/// The ideas room, relative to the workspace root.
pub const DIR: &str = "ideas";

pub const OPEN: &str = "open";
pub const PARKED: &str = "parked";
pub const PROMOTED: &str = "promoted";
pub const DROPPED: &str = "dropped";

const KNOWN_STATUSES: [&str; 4] = [OPEN, PARKED, PROMOTED, DROPPED];

/// The built-in summary template.
const TEMPLATE: &str = "root/ideas/templates/summary.md";

/// An idea's folder. The room also holds `context.md`, `personas/` and
/// `templates/`, which aren't ideas.
static FOLDER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)-").expect("valid"));

/// One idea, read from its `summary.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Idea {
    pub id: i64,
    pub slug: String,
    /// As the summary holds it (`idea list` prints it).
    pub title: Text,
    pub status: String,
    pub reason: Text,
    /// `YYYY-MM-DD`.
    pub updated: String,
    /// The changes promoted from it, in order.
    pub changes: Vec<String>,
    /// `ideas/<n>-<slug>`, slash-separated, relative to the root.
    pub dir: String,
    /// `summary.md` below its frontmatter, as bytes (`idea promote` copies it).
    pub body: Text,
}

impl Idea {
    /// Go's `CanPromote`: an open idea, or one already promoted (which is how an
    /// idea is split). A parked or dropped idea has to be reopened first.
    pub fn can_promote(&self) -> Result<(), Error> {
        if self.status == OPEN || self.status == PROMOTED {
            return Ok(());
        }
        Err(Error::IdeaWrongStatus {
            id: self.id,
            status: self.status.clone(),
            rule: "reopen it first".into(),
        })
    }
}

#[derive(Debug, Clone)]
struct Folder {
    id: i64,
    name: String,
}

fn room_rel(f: &Folder) -> String {
    format!("{DIR}/{}", f.name)
}

fn summary_path(root: &Path, f: &Folder) -> PathBuf {
    at(root, &format!("{}/summary.md", room_rel(f)))
}

/// The idea folders in the room, by id. A missing room is empty. As Go's
/// `DirEntry.IsDir`, a symlink is never a folder.
fn folders(root: &Path) -> Result<Vec<Folder>, Error> {
    let entries = match std::fs::read_dir(at(root, DIR)) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(Error::Io {
                op: "open",
                path: shown(root, DIR),
                source,
            });
        }
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        let Some(m) = FOLDER_RE.captures(&name).filter(|_| is_dir) else {
            continue;
        };
        if let Ok(id) = m[1].parse::<i64>() {
            out.push(Folder { id, name });
        }
    }
    out.sort_by_key(|f| f.id);
    Ok(out)
}

/// Go's `oneLine`: runs of whitespace, newlines included, become one space, and
/// the ends are trimmed, so a value can't add a frontmatter line of its own.
fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Go's `New`: creates `ideas/<n>-<slug>/` with a summary from the template and a
/// log. The id is the highest folder number plus one, so it's never reused while
/// a folder exists. The folder is made exclusively, so two runs can't share an id.
pub fn new(root: &Path, title: &str, today: &str) -> Result<Idea, Error> {
    let title = one_line(title);
    if title.is_empty() {
        return Err(Error::IdeaTitleRequired);
    }
    let highest = folders(root)?.last().map_or(0, |f| f.id);
    if highest == i64::MAX {
        return Err(Error::IdeaIdOutOfRange);
    }
    let id = highest + 1;
    let mut slug = state::slugify(&title);
    if slug.is_empty() {
        slug = "idea".into();
    }
    let name = format!("{id}-{slug}");
    let summary = fill(
        scaffold::template(TEMPLATE).expect("the idea template is embedded"),
        &[
            ("<id>", &id.to_string()),
            ("<title>", &title),
            ("<date>", today),
        ],
    );

    write::create_dir_all(root, &at(root, DIR))?;
    let rel = format!("{DIR}/{name}");
    let dir = at(root, &rel);
    write::check_path(root, &dir)?;
    if let Err(source) = std::fs::create_dir(&dir) {
        if source.kind() == std::io::ErrorKind::AlreadyExists {
            return Err(Error::IdeaExists { name: rel });
        }
        return Err(Error::Io {
            op: "mkdir",
            path: shown(root, &rel),
            source,
        });
    }
    let log = format!("# Log\n\n- {today} created\n");
    for (file, content) in [("summary.md", summary.as_str()), ("log.md", log.as_str())] {
        if let Err(e) = create_file(root, &format!("{rel}/{file}"), content) {
            // Ours: the create_dir above just made it.
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
    }
    load(root, id)
}

/// Go's `strings.NewReplacer(...).Replace`: one pass over `template`, so text
/// that's put in (a title holding `<date>`) is never replaced again.
fn fill(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while !rest.is_empty() {
        match pairs.iter().find(|(old, _)| rest.starts_with(old)) {
            Some((old, new)) => {
                out.push_str(new);
                rest = &rest[old.len()..];
            }
            None => {
                let c = rest.chars().next().expect("not empty");
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
        }
    }
    out
}

/// Go's `createFile`: writes a new file and refuses to touch an existing one.
fn create_file(root: &Path, rel: &str, content: &str) -> Result<(), Error> {
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    let mut f = write::open_file(root, &at(root, rel), &opts)?;
    f.write_all(content.as_bytes()).map_err(|source| Error::Io {
        op: "write",
        path: shown(root, rel),
        source,
    })
}

/// Go's `find`: the one folder for `id`.
fn find(root: &Path, id: i64) -> Result<Folder, Error> {
    let mut matches: Vec<Folder> = folders(root)?.into_iter().filter(|f| f.id == id).collect();
    match matches.len() {
        0 => Err(Error::IdeaNotFound { id }),
        1 => Ok(matches.remove(0)),
        _ => Err(Error::IdeaAmbiguous { id }),
    }
}

/// Go's `Load`: idea `id`, from its `summary.md`.
pub fn load(root: &Path, id: i64) -> Result<Idea, Error> {
    let f = find(root, id)?;
    load_folder(root, &f)
}

fn load_folder(root: &Path, f: &Folder) -> Result<Idea, Error> {
    let raw = read_all(&summary_path(root, f)).map_err(|e| Error::IdeaUnreadable {
        name: room_rel(f),
        cause: e.to_string(),
    })?;
    let (fm, body) = source::parse_frontmatter_bytes(&raw);
    if fm.is_empty() {
        return Err(Error::IdeaNoFrontmatter { name: room_rel(f) });
    }
    let get = |k: &str| fm.get(k).cloned().unwrap_or_default();
    let text = |k: &str| get(k).to_string_lossy().into_owned();
    let changes = text("changes")
        .split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(String::from)
        .collect();
    Ok(Idea {
        id: f.id,
        slug: FOLDER_RE.replace(&f.name, "").into_owned(),
        title: get("title"),
        status: text("status"),
        reason: get("reason"),
        updated: text("updated"),
        changes,
        dir: room_rel(f),
        body,
    })
}

/// Go's `List`: every idea, by id. An idea whose summary can't be read is an
/// error that names it, so it's fixed and not skipped.
pub fn list(root: &Path) -> Result<Vec<Idea>, Error> {
    folders(root)?
        .iter()
        .map(|f| load_folder(root, f))
        .collect()
}

/// The `summary.md` of idea `id`, byte for byte as it is on disk (Go's
/// `ideaShow`).
pub fn summary(root: &Path, id: i64) -> Result<Vec<u8>, Error> {
    let i = load(root, id)?;
    read_all(&at(root, &format!("{}/summary.md", i.dir)))
}

/// One status change: the statuses it needs, what it sets, and what the log says.
struct Change<'a> {
    from: &'a [&'a str],
    /// The rule a refusal names, after `idea <id> is <status>: `.
    refuse: &'a str,
    status: &'a str,
    /// Empty clears it.
    reason: &'a str,
    extra: Vec<(&'a str, String)>,
    log: String,
}

/// Go's `apply`: rewrites the summary's frontmatter, then appends to the log. A
/// refusal changes and logs nothing.
fn apply(root: &Path, id: i64, today: &str, c: Change) -> Result<Idea, Error> {
    let f = find(root, id)?;
    let i = load_folder(root, &f)?;
    if !KNOWN_STATUSES.contains(&i.status.as_str()) {
        return Err(Error::IdeaUnknownStatus {
            id,
            status: i.status,
        });
    }
    if !c.from.contains(&i.status.as_str()) {
        return Err(Error::IdeaWrongStatus {
            id,
            status: i.status,
            rule: c.refuse.to_string(),
        });
    }
    let rel = format!("{}/summary.md", room_rel(&f));
    let path = at(root, &rel);
    let raw = read_all(&path)?;
    let mut updates = vec![
        ("status", c.status.to_string()),
        ("reason", c.reason.to_string()),
        ("updated", today.to_string()),
    ];
    updates.extend(c.extra);
    let updated = set_frontmatter(&raw, &updates)
        .map_err(|NoFrontmatter| Error::IdeaFrontmatter { name: room_rel(&f) })?;
    write::write_file(root, &path, &updated)?;
    if let Err(e) = append_log(root, &f, today, &c.log) {
        return Err(Error::IdeaLogFailed {
            id,
            status: c.status.to_string(),
            cause: e.to_string(),
        });
    }
    load_folder(root, &f)
}

fn require_reason(reason: &str) -> Result<String, Error> {
    let reason = one_line(reason);
    if reason.is_empty() {
        return Err(Error::ReasonRequired);
    }
    Ok(reason)
}

/// Go's `Park`: sets an open idea aside, with a reason.
pub fn park(root: &Path, id: i64, reason: &str, today: &str) -> Result<Idea, Error> {
    let reason = require_reason(reason)?;
    let change = Change {
        from: &[OPEN],
        refuse: "only an open idea can be parked",
        status: PARKED,
        reason: &reason,
        extra: Vec::new(),
        log: format!("parked: {reason}"),
    };
    apply(root, id, today, change)
}

/// Go's `Drop`: drops an open idea, with a reason. The folder stays.
pub fn drop(root: &Path, id: i64, reason: &str, today: &str) -> Result<Idea, Error> {
    let reason = require_reason(reason)?;
    let change = Change {
        from: &[OPEN],
        refuse: "only an open idea can be dropped",
        status: DROPPED,
        reason: &reason,
        extra: Vec::new(),
        log: format!("dropped: {reason}"),
    };
    apply(root, id, today, change)
}

/// Go's `Reopen`: returns a parked or dropped idea to open, with a reason. The
/// reason it was set aside for is cleared from the summary, and stays in the log.
pub fn reopen(root: &Path, id: i64, reason: &str, today: &str) -> Result<Idea, Error> {
    let reason = require_reason(reason)?;
    let change = Change {
        from: &[PARKED, DROPPED],
        refuse: "only a parked or dropped idea can be reopened",
        status: OPEN,
        reason: "",
        extra: Vec::new(),
        log: format!("reopened: {reason}"),
    };
    apply(root, id, today, change)
}

/// Go's `MarkPromoted`: records that a change was promoted from the idea, which
/// has to be open or already promoted.
pub fn mark_promoted(root: &Path, id: i64, change_name: &str, today: &str) -> Result<Idea, Error> {
    let change_name = one_line(change_name);
    if change_name.is_empty() {
        return Err(Error::IdeaChangeNameRequired);
    }
    let mut changes = load(root, id)?.changes;
    changes.push(change_name.clone());
    let change = Change {
        from: &[OPEN, PROMOTED],
        refuse: "reopen it first",
        status: PROMOTED,
        reason: "",
        extra: vec![("changes", changes.join(", "))],
        log: format!("promoted to {change_name}"),
    };
    apply(root, id, today, change)
}

/// Go's `appendLog`: one dated line, with the log opened only for append. A
/// leading newline goes first when a hand edit left none at the end.
fn append_log(root: &Path, f: &Folder, today: &str, text: &str) -> Result<(), Error> {
    let rel = format!("{}/log.md", room_rel(f));
    let path = at(root, &rel);
    let prefix = match std::fs::read(&path) {
        Ok(existing) if existing.last().is_some_and(|b| *b != b'\n') => "\n",
        _ => "",
    };
    let mut opts = std::fs::OpenOptions::new();
    opts.append(true).create(true);
    let mut file = write::open_file(root, &path, &opts)?;
    file.write_all(format!("{prefix}- {today} {text}\n").as_bytes())
        .map_err(|source| Error::Io {
            op: "write",
            path: shown(root, &rel),
            source,
        })
}

/// A text with no frontmatter block, or one that's never closed.
#[derive(Debug, PartialEq, Eq)]
pub struct NoFrontmatter;

/// Go's `setFrontmatter`: replaces the value on the first line whose key matches
/// each update (case-insensitively, keeping the line's own spelling and any `\r`),
/// and adds a missing key before the closing `---`. Every other line, and
/// everything after the block, is kept byte for byte, valid UTF-8 or not, as Go's
/// strings keep it. `updates`' keys are lower case.
pub fn set_frontmatter(raw: &[u8], updates: &[(&str, String)]) -> Result<Vec<u8>, NoFrontmatter> {
    let is_fence = |l: &[u8]| l.strip_suffix(b"\r").unwrap_or(l) == b"---";
    let mut lines: Vec<Vec<u8>> = raw.split(|b| *b == b'\n').map(<[u8]>::to_vec).collect();
    if !is_fence(&lines[0]) {
        return Err(NoFrontmatter);
    }
    let mut end = lines
        .iter()
        .skip(1)
        .position(|l| is_fence(l))
        .map(|i| i + 1)
        .ok_or(NoFrontmatter)?;
    let value_text = |v: &str| {
        if v.is_empty() {
            String::new()
        } else {
            format!(" {v}")
        }
    };
    for (key, value) in updates {
        // Only the comparison decodes: Go's TrimSpace and ToLower on the key. A key
        // with invalid UTF-8 never equals an owned key, as in Go.
        let found = (1..end).find_map(|i| {
            let j = lines[i].iter().position(|b| *b == b':')?;
            let k = String::from_utf8_lossy(&lines[i][..j]);
            (k.trim().to_lowercase() == *key).then_some((i, j))
        });
        match found {
            Some((i, j)) => {
                let cr = lines[i].ends_with(b"\r");
                let line = &mut lines[i];
                line.truncate(j + 1);
                line.extend_from_slice(value_text(value).as_bytes());
                if cr {
                    line.push(b'\r');
                }
            }
            None => {
                lines.insert(end, format!("{key}:{}", value_text(value)).into_bytes());
                end += 1;
            }
        }
    }
    Ok(lines.join(&b'\n'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Temp;

    const DAY1: &str = "2026-11-12";
    const DAY2: &str = "2026-11-13";
    const DAY3: &str = "2026-11-14";

    type Op = fn(&Path, i64, &str, &str) -> Result<Idea, Error>;

    /// Everything after the frontmatter block.
    fn body_of(raw: &str) -> String {
        raw.split_once("\n---\n")
            .unwrap_or_else(|| panic!("no frontmatter in {raw:?}"))
            .1
            .to_string()
    }

    fn new_idea(t: &Temp, title: &str) -> Idea {
        new(t.path(), title, DAY1).unwrap()
    }

    fn err<T: std::fmt::Debug>(r: Result<T, Error>) -> String {
        r.unwrap_err().to_string()
    }

    // Go's TestNewCreatesTheFolderSummaryAndLog.
    #[test]
    fn new_creates_the_folder_summary_and_log() {
        let t = Temp::new();
        let i = new_idea(&t, "Faster imports");
        assert_eq!(
            (i.id, i.slug.as_str(), i.dir.as_str(), i.status.as_str()),
            (1, "faster-imports", "ideas/1-faster-imports", OPEN)
        );
        assert_eq!(
            (&*i.title.to_string_lossy(), i.updated.as_str()),
            ("Faster imports", DAY1)
        );
        let summary = t.read("ideas/1-faster-imports/summary.md");
        let front = "---\nid: 1\ntitle: Faster imports\nstatus: open\nreason:\nupdated: 2026-11-12\nchanges:\n---\n";
        assert!(summary.starts_with(front), "{summary}");
        for section in [
            "## Problem",
            "## Options considered",
            "## Cuts and no-gos",
            "## Open questions",
            "## References cited",
            "## Resulting changes",
        ] {
            assert!(
                summary.contains(&format!("\n{section}\n")),
                "lacks {section}"
            );
        }
        let lower = summary.to_lowercase();
        assert!(!lower.contains("appetite") && !lower.contains("budget"));
        assert_eq!(
            t.read("ideas/1-faster-imports/log.md"),
            "# Log\n\n- 2026-11-12 created\n"
        );
    }

    // Go's TestNewIdsAreSequentialAndNeverReused.
    #[test]
    fn ids_are_sequential_and_never_reused() {
        let t = Temp::new();
        let a = new_idea(&t, "Same title");
        let b = new_idea(&t, "Same title");
        let c = new_idea(&t, "Third");
        assert_eq!((a.id, b.id, c.id), (1, 2, 3));
        assert_eq!(b.dir, "ideas/2-same-title");
        std::fs::remove_dir_all(t.p("ideas/2-same-title")).unwrap();
        assert_eq!(new_idea(&t, "Fourth").id, 4);
    }

    // Go's TestTheRoomsOwnFilesAreNotIdeas.
    #[test]
    fn the_rooms_own_files_are_not_ideas() {
        let t = Temp::new();
        t.write("ideas/context.md", "the guide");
        t.write("ideas/personas/facilitator.md", "persona");
        t.write("ideas/templates/summary.md", "template");
        t.write("ideas/9-notes.txt", "a file, not a folder");
        t.write("ideas/drafts/x.md", "a folder with no number");
        assert!(list(t.path()).unwrap().is_empty());
        assert_eq!(new_idea(&t, "First").id, 1);
    }

    /// A symlinked folder isn't an idea, and a new idea isn't written through one.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_folder_is_not_an_idea_and_is_not_written_through() {
        let t = Temp::new();
        t.write("elsewhere/summary.md", "---\nid: 1\nstatus: open\n---\n");
        std::fs::create_dir_all(t.p("ideas")).unwrap();
        std::os::unix::fs::symlink(t.p("elsewhere"), t.p("ideas/1-x")).unwrap();
        assert!(list(t.path()).unwrap().is_empty());
        let e = err(new(t.path(), "X", DAY1));
        assert!(e.contains("is a symlink"), "{e}");
        assert_eq!(std::fs::read_dir(t.p("elsewhere")).unwrap().count(), 1);
    }

    // Go's TestNewEdgeCases.
    #[test]
    fn new_edge_cases() {
        let t = Temp::new();
        let i = new_idea(&t, "!!!");
        assert_eq!(
            (i.dir.as_str(), &*i.title.to_string_lossy()),
            ("ideas/1-idea", "!!!")
        );
        for title in ["", "  \n\t "] {
            assert!(err(new(t.path(), title, DAY1)).contains("needs a title"));
        }
        let big = Temp::new();
        big.write(
            "ideas/9223372036854775807-x/summary.md",
            "---\nid: 1\n---\n",
        );
        assert!(err(new(big.path(), "Next", DAY1)).contains("out of range"));
        assert_eq!(std::fs::read_dir(big.p("ideas")).unwrap().count(), 1);
        let forged = new_idea(&t, "Real\nstatus: dropped");
        let raw = t.read(&format!("{}/summary.md", forged.dir));
        assert_eq!(forged.title, "Real status: dropped");
        assert_eq!(forged.status, OPEN);
        assert_eq!(raw.matches("\nstatus:").count(), 1);
        let log = format!("{}/log.md", forged.dir);
        assert!(
            create_file(t.path(), &log, "x").is_err(),
            "overwrote a file"
        );
    }

    // Go's TestParkAndDrop.
    #[test]
    fn park_and_drop() {
        for (name, op, status) in [("park", park as Op, PARKED), ("drop", drop as Op, DROPPED)] {
            let t = Temp::new();
            let i = new_idea(&t, "An idea");
            let summary = format!("{}/summary.md", i.dir);
            let log = format!("{}/log.md", i.dir);
            let body = body_of(&t.read(&summary));
            let (before_summary, before_log) = (t.read(&summary), t.read(&log));
            for reason in ["", "  \n "] {
                assert!(err(op(t.path(), 1, reason, DAY2)).contains("reason is required"));
            }
            assert_eq!(t.read(&summary), before_summary, "{name}");
            assert_eq!(t.read(&log), before_log, "{name}");

            let got = op(t.path(), 1, "later", DAY2).unwrap();
            assert_eq!(got.status, status, "{name}");
            assert_eq!(
                (&*got.reason.to_string_lossy(), got.updated.as_str()),
                ("later", DAY2)
            );
            let raw = t.read(&summary);
            for want in [
                format!("status: {status}"),
                "reason: later".into(),
                format!("updated: {DAY2}"),
            ] {
                assert!(raw.contains(&want), "{name}: lacks {want}");
            }
            assert_eq!(body_of(&raw), body);
            assert_eq!(
                t.read(&log),
                format!("{before_log}- {DAY2} {status}: later\n")
            );

            let before_log = t.read(&log);
            let e = err(op(t.path(), 1, "again", DAY3));
            let want = format!("idea 1 is {status}: only an open idea can be");
            assert!(e.contains(&want), "{e}");
            assert_eq!((t.read(&summary), t.read(&log)), (raw, before_log));
        }
    }

    // Go's TestAReasonCantInjectAFrontmatterLine.
    #[test]
    fn a_reason_cant_inject_a_frontmatter_line() {
        let t = Temp::new();
        let i = new_idea(&t, "An idea");
        let got = park(t.path(), 1, "x\nstatus: open\nchanges: 9-forged", DAY2).unwrap();
        let raw = t.read(&format!("{}/summary.md", i.dir));
        let front = raw.split_once("\n---\n").unwrap().0;
        assert_eq!(got.status, PARKED);
        assert_eq!(got.reason, "x status: open changes: 9-forged");
        assert!(got.changes.is_empty());
        assert_eq!(front.matches("\nstatus:").count(), 1);
        assert_eq!(front.matches("\nchanges:").count(), 1);
    }

    // Go's TestReopen.
    #[test]
    fn reopen_returns_to_open() {
        let t = Temp::new();
        let i = new_idea(&t, "An idea");
        let summary = format!("{}/summary.md", i.dir);
        let e = err(reopen(t.path(), 1, "now", DAY2));
        assert!(e.contains("idea 1 is open: only a parked or dropped idea can be reopened"));
        for set_aside in [park as Op, drop as Op] {
            set_aside(t.path(), 1, "not yet", DAY2).unwrap();
            let body = body_of(&t.read(&summary));
            assert!(err(reopen(t.path(), 1, "", DAY3)).contains("reason is required"));
            let got = reopen(t.path(), 1, "it matters again", DAY3).unwrap();
            let raw = t.read(&summary);
            assert_eq!(
                (got.status.as_str(), &*got.reason.to_string_lossy()),
                (OPEN, "")
            );
            assert_eq!(got.updated, DAY3);
            assert!(raw.contains("\nreason:\n"), "{raw}");
            assert_eq!(body_of(&raw), body);
            let log = t.read(&format!("{}/log.md", i.dir));
            assert!(log.ends_with(&format!("- {DAY3} reopened: it matters again\n")));
        }
    }

    // Go's TestMarkPromoted.
    #[test]
    fn mark_promoted_records_each_change() {
        let t = Temp::new();
        let i = new_idea(&t, "An idea");
        let summary = format!("{}/summary.md", i.dir);
        let body = body_of(&t.read(&summary));
        assert!(mark_promoted(t.path(), 1, "", DAY2).is_err());
        let got = mark_promoted(t.path(), 1, "3-part-one", DAY2).unwrap();
        assert_eq!(got.status, PROMOTED);
        assert_eq!(got.changes, ["3-part-one"]);
        let got = mark_promoted(t.path(), 1, "4-part-two", DAY3).unwrap();
        assert_eq!(got.changes, ["3-part-one", "4-part-two"]);
        assert_eq!(got.updated, DAY3);
        let raw = t.read(&summary);
        assert!(raw.contains("\nchanges: 3-part-one, 4-part-two\n"), "{raw}");
        assert_eq!(body_of(&raw), body);
        let log = t.read(&format!("{}/log.md", i.dir));
        assert!(log.contains(&format!("- {DAY2} promoted to 3-part-one\n")));
        assert!(log.ends_with(&format!("- {DAY3} promoted to 4-part-two\n")));
        let e = err(park(t.path(), 1, "x", DAY3));
        assert!(
            e.contains("idea 1 is promoted: only an open idea can be parked"),
            "{e}"
        );

        for other in [park as Op, drop as Op] {
            let j = new_idea(&t, "Another");
            other(t.path(), j.id, "not now", DAY2).unwrap();
            let rel = format!("{}/summary.md", j.dir);
            let before = t.read(&rel);
            assert!(err(mark_promoted(t.path(), j.id, "9-x", DAY3)).contains("reopen it first"));
            assert_eq!(t.read(&rel), before);
        }
    }

    // Go's TestHandEditsSurviveAStatusChange.
    #[test]
    fn hand_edits_survive_a_status_change() {
        let t = Temp::new();
        t.write(
            "ideas/1-hand-made/summary.md",
            "---\nowner: brian\nid: 1\nTitle: Hand made\nstatus: open\nlabels: a, b\n---\n\n## Problem\n\nMine, with *markdown*.\n",
        );
        t.write("ideas/1-hand-made/log.md", "# Log\n\n- no final newline");
        let got = park(t.path(), 1, "waiting", DAY2).unwrap();
        assert_eq!(
            (
                &*got.title.to_string_lossy(),
                got.status.as_str(),
                got.slug.as_str()
            ),
            ("Hand made", PARKED, "hand-made")
        );
        assert_eq!(
            t.read("ideas/1-hand-made/summary.md"),
            "---\nowner: brian\nid: 1\nTitle: Hand made\nstatus: parked\nlabels: a, b\nreason: waiting\nupdated: 2026-11-13\n---\n\n## Problem\n\nMine, with *markdown*.\n"
        );
        assert_eq!(
            t.read("ideas/1-hand-made/log.md"),
            "# Log\n\n- no final newline\n- 2026-11-13 parked: waiting\n"
        );
    }

    /// AC3: a CRLF summary with extra keys, a hand-cased key and a hand-edited
    /// body, parked then reopened: at each step only the status, reason and
    /// updated lines differ, and the log gains exactly one line.
    #[test]
    fn ac3_crlf_hand_edits_change_only_owned_lines() {
        let t = Temp::new();
        let start = "---\r\nid: 1\r\nTitle: Hand made\r\nStatus: open\r\nowner: brian\r\nreason:\r\nupdated: 2026-01-01\r\nchanges:\r\n---\r\n\r\n## Problem\r\n\r\nEdited *by hand*.\r\n";
        t.write("ideas/1-hand-made/summary.md", start);
        t.write(
            "ideas/1-hand-made/log.md",
            "# Log\r\n\r\n- 2026-01-01 created\r\n",
        );
        let owned = |line: &str| {
            let key = line.split(':').next().unwrap_or("").to_lowercase();
            ["status", "reason", "updated"].contains(&key.as_str())
        };
        let mut before = start.to_string();
        for (step, status) in [("park", PARKED), ("reopen", OPEN)] {
            let log_before = t.read("ideas/1-hand-made/log.md");
            let got = match step {
                "park" => park(t.path(), 1, "after the beta", DAY2),
                _ => reopen(t.path(), 1, "beta shipped", DAY3),
            }
            .unwrap();
            assert_eq!(got.status, status);
            let after = t.read("ideas/1-hand-made/summary.md");
            let b: Vec<&str> = before.split('\n').collect();
            let a: Vec<&str> = after.split('\n').collect();
            assert_eq!(b.len(), a.len(), "{step}: a line was added or removed");
            for (x, y) in b.iter().zip(&a) {
                if x != y {
                    assert!(owned(x) && owned(y), "{step}: {x:?} became {y:?}");
                    assert!(y.ends_with('\r'), "{step}: {y:?} lost its \\r");
                }
            }
            assert!(
                after.contains("\r\nStatus: "),
                "{step}: the key was respelled"
            );
            let log = t.read("ideas/1-hand-made/log.md");
            let added = log.strip_prefix(&log_before).expect("appended");
            assert_eq!(added.matches('\n').count(), 1, "{step}: {added:?}");
            before = after;
        }
    }

    // Go's TestSetFrontmatter.
    #[test]
    fn set_frontmatter_cases() {
        for (name, raw, want) in [
            (
                "replaces in place",
                "---\na: 1\nb: 2\n---\nbody\n",
                "---\na: 1\nb: 9\n---\nbody\n",
            ),
            (
                "adds a missing key",
                "---\na: 1\n---\nbody",
                "---\na: 1\nb: 9\n---\nbody",
            ),
            ("clears to a bare key", "---\nb: 2\n---\n", "---\nb:\n---\n"),
            (
                "keeps CRLF endings",
                "---\r\nb: 2\r\n---\r\nx\r\n",
                "---\r\nb: 9\r\n---\r\nx\r\n",
            ),
            (
                "keeps the key's spelling",
                "---\nB: 2\n---\n",
                "---\nB: 9\n---\n",
            ),
            (
                "only the first matching line",
                "---\nb: 1\nb: 2\n---\n",
                "---\nb: 9\nb: 2\n---\n",
            ),
            (
                "a later --- isn't the block",
                "---\na: 1\n---\ntext\n---\nb: 5\n---\n",
                "---\na: 1\nb: 9\n---\ntext\n---\nb: 5\n---\n",
            ),
        ] {
            let value = if name.contains("clears") { "" } else { "9" };
            let got = set_frontmatter(raw.as_bytes(), &[("b", value.into())]);
            assert_eq!(got.as_deref(), Ok(want.as_bytes()), "{name}");
        }
        for raw in ["no frontmatter\n", "", "---\nunclosed: yes\n"] {
            let got = set_frontmatter(raw.as_bytes(), &[("b", "1".into())]);
            assert_eq!(got, Err(NoFrontmatter), "{raw:?}");
        }
    }

    // Go's TestLoadAndListErrors.
    #[test]
    fn load_and_list_errors() {
        let t = Temp::new();
        assert!(err(load(t.path(), 7)).contains("idea 7 not found"));
        t.write(
            "ideas/1-fine/summary.md",
            "---\nid: 1\ntitle: Fine\nstatus: open\n---\n",
        );
        t.write("ideas/2-broken/summary.md", "no frontmatter at all\n");
        t.write("ideas/3-a/summary.md", "---\ntitle: A\nstatus: open\n---\n");
        t.write("ideas/3-b/summary.md", "---\ntitle: B\nstatus: open\n---\n");
        t.write(
            "ideas/4-odd/summary.md",
            "---\ntitle: Odd\nstatus: archived\n---\n",
        );
        assert!(err(load(t.path(), 2)).contains("ideas/2-broken/summary.md has no frontmatter"));
        assert_eq!(
            err(load(t.path(), 3)),
            "daiku: idea 3 is ambiguous: more than one folder in ideas/ starts with 3-"
        );
        assert!(err(park(t.path(), 4, "x", DAY2)).contains(r#"unknown status "archived""#));
        assert!(err(list(t.path())).contains("ideas/2-broken"));
        assert!(load(t.path(), 1).is_ok());
    }

    // Go's TestListIsOrderedAndComplete.
    #[test]
    fn list_is_ordered_and_complete() {
        let t = Temp::new();
        for title in ["One", "Two", "Three", "Four"] {
            new_idea(&t, title);
        }
        park(t.path(), 2, "later", DAY2).unwrap();
        drop(t.path(), 3, "no", DAY2).unwrap();
        mark_promoted(t.path(), 4, "5-four", DAY2).unwrap();
        let got: Vec<String> = list(t.path())
            .unwrap()
            .iter()
            .map(|i| format!("{}|{}|{}", i.title, i.status, i.reason))
            .collect();
        assert_eq!(
            got,
            [
                "One|open|",
                "Two|parked|later",
                "Three|dropped|no",
                "Four|promoted|"
            ]
        );
    }

    /// A summary that's a folder reads as Go's `os.ReadFile` fails: the open
    /// succeeds and the read doesn't. Unix only: on Windows opening a folder
    /// fails at the open (`permission denied`), and Go's fixtures don't run there.
    #[cfg(unix)]
    #[test]
    fn a_summary_that_is_a_folder_is_a_read_error() {
        let t = Temp::new();
        t.write("ideas/1-a/summary.md/x", "x");
        assert_eq!(
            err(load(t.path(), 1)),
            format!(
                "daiku: cannot read ideas/1-a/summary.md: read {}: is a directory",
                shown(t.path(), "ideas/1-a/summary.md")
            )
        );
    }

    /// Review finding 1: bytes that aren't valid UTF-8, in a hand key, a value
    /// daiku doesn't own and the body, survive park and reopen unchanged, as Go
    /// keeps them; `summary` returns them as they are.
    #[test]
    fn non_utf8_bytes_survive_a_status_change() {
        let t = Temp::new();
        let start: &[u8] =
            b"---\nid: 1\ntitle: Caf\xe9\nstatus: open\nnot\xffe: \xa0x\n---\n\nBody \xff by hand.\n";
        std::fs::create_dir_all(t.p("ideas/1-x")).unwrap();
        std::fs::write(t.p("ideas/1-x/summary.md"), start).unwrap();
        t.write("ideas/1-x/log.md", "# Log\n");
        park(t.path(), 1, "later", DAY2).unwrap();
        let parked = std::fs::read(t.p("ideas/1-x/summary.md")).unwrap();
        let want: &[u8] = b"---\nid: 1\ntitle: Caf\xe9\nstatus: parked\nnot\xffe: \xa0x\nreason: later\nupdated: 2026-11-13\n---\n\nBody \xff by hand.\n";
        assert_eq!(parked, want);
        assert_eq!(summary(t.path(), 1).unwrap(), want);
        reopen(t.path(), 1, "now", DAY3).unwrap();
        let reopened = std::fs::read(t.p("ideas/1-x/summary.md")).unwrap();
        assert!(
            reopened.starts_with(b"---\nid: 1\ntitle: Caf\xe9\nstatus: open\nnot\xffe: \xa0x\n")
        );
        assert!(reopened.ends_with(b"---\n\nBody \xff by hand.\n"));
    }

    /// Review finding 2: the template is filled in one pass, as Go's
    /// `strings.NewReplacer`, so placeholders typed in a title stay as typed.
    #[test]
    fn placeholders_in_a_title_are_kept() {
        let t = Temp::new();
        let i = new_idea(&t, "Ship by <date>, <title> or <id>");
        assert_eq!(i.title, "Ship by <date>, <title> or <id>");
        let summary = t.read(&format!("{}/summary.md", i.dir));
        assert!(
            summary.contains("\ntitle: Ship by <date>, <title> or <id>\n"),
            "{summary}"
        );
        assert!(
            summary.contains(&format!("\nupdated: {DAY1}\n")),
            "{summary}"
        );
        assert_eq!(
            fill("<a><b>x<a", &[("<a>", "<b>"), ("<b>", "B")]),
            "<b>Bx<a"
        );
        assert_eq!(fill("é<a>ü", &[("<a>", "1")]), "é1ü");
    }

    /// N-0001: a title, reason and body that aren't valid UTF-8 load as bytes.
    #[test]
    fn load_keeps_bytes() {
        let t = Temp::new();
        std::fs::create_dir_all(t.p("ideas/1-x")).unwrap();
        std::fs::write(
            t.p("ideas/1-x/summary.md"),
            b"---\ntitle: Id\xe9e\nstatus: parked\nreason: apr\xe8s\n---\n\nBody \xff.\n",
        )
        .unwrap();
        let i = load(t.path(), 1).unwrap();
        assert_eq!(i.title.as_bytes(), b"Id\xe9e");
        assert_eq!(i.reason.as_bytes(), b"apr\xe8s");
        assert_eq!(i.body.as_bytes(), b"Body \xff.");
        assert_eq!(i.status, PARKED);
    }
}
