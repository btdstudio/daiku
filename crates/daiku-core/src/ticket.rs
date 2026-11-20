//! Tickets: Go's `internal/core/ticket` (D8): parsing, loading a folder,
//! validation, dependency order, and the freeze and readiness hashes. The write
//! side (creation, the readiness and publish marks, ids) is in `write.rs` (S-0007).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::frontmatter::{Document, Kind};
use crate::{Error, gofmt, gojson, placeholder};

/// The ticket schema this daiku reads.
pub const SCHEMA: i64 = 1;

pub const EPIC: &str = "epic";
pub const FEATURE: &str = "feature";
pub const STORY: &str = "story";
pub const TASK: &str = "task";

pub const STATUS_DRAFT: &str = "draft";
pub const STATUS_READY: &str = "ready";
pub const STATUS_PUBLISHED: &str = "published";
/// Native tickets only.
pub const STATUS_CLOSED: &str = "closed";

/// Marks a ticket created in the build room, so its id can't collide with a planned one.
pub const NATIVE_PREFIX: &str = "N";
/// Marks a ticket pulled from a PM tool.
pub const PULLED_PREFIX: &str = "X";

/// A type's rank: an epic holds features, a feature stories, a story tasks.
fn rank(typ: &str) -> Option<u8> {
    match typ {
        EPIC => Some(4),
        FEATURE => Some(3),
        STORY => Some(2),
        TASK => Some(1),
        _ => None,
    }
}

fn prefix_of(typ: &str) -> Option<&'static str> {
    match typ {
        EPIC => Some("E"),
        FEATURE => Some("F"),
        STORY => Some("S"),
        TASK => Some("T"),
        _ => None,
    }
}

/// Planned and native ids: a prefix and a four-digit sequence.
static PLANNED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([EFSTN])-([0-9]{4})$").expect("valid pattern"));

/// Pulled ids: `X-` and hyphen-separated runs of letters and digits.
static PULLED_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^X-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*$").expect("valid pattern"));

/// Any valid ticket id: planned (`S-0001`), native (`N-0001`) or pulled (`X-…`).
/// A build change is named by one, so it doubles as a safe folder name.
pub static ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:([EFSTN])-([0-9]{4})|X-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*)$")
        .expect("valid pattern")
});

/// Where a native ticket was promoted from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Origin {
    pub workspace: String,
    pub path: String,
}

/// A ticket file.
#[derive(Debug, Clone)]
pub struct Ticket {
    pub schema: i64,
    pub id: String,
    pub typ: String,
    pub title: String,
    pub parent: String,
    pub repo: String,
    pub depends_on: Vec<String>,
    pub supersedes: String,
    pub change: String,
    pub spec: String,
    pub adrs: Vec<String>,
    /// Ids from the change's spec; the ticket model doesn't check them.
    pub requirements: Vec<String>,
    pub labels: Vec<String>,
    pub estimate: String,
    pub status: String,
    pub external: BTreeMap<String, String>,
    pub published_hash: String,
    /// The readiness mark: the content hash when it was readied.
    pub ready_hash: String,
    pub origin: Origin,
    pub body: String,
    /// Where it was loaded from (slash-separated, as shown), if anywhere.
    pub path: String,
    doc: Document,
    /// Fields whose bytes in the file weren't valid UTF-8.
    invalid_utf8: Vec<String>,
    /// The file's bytes after the frontmatter, kept so a write doesn't replace a
    /// body byte that isn't UTF-8 (S-0007 D2). `None` for a ticket built in memory,
    /// or one whose frontmatter isn't valid UTF-8, which can't be written.
    raw_body: Option<Vec<u8>>,
    /// The frontmatter wasn't valid UTF-8, so writing it back would change it.
    lossy_frontmatter: bool,
}

/// One thing wrong with a ticket.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Problem {
    /// The ticket, or its path if the id isn't usable.
    pub id: String,
    pub message: String,
    /// For a missing reference, the id it names.
    pub reference: String,
}

impl Problem {
    pub fn new(id: &str, message: String) -> Problem {
        Problem {
            id: id.to_string(),
            message,
            reference: String::new(),
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.id, self.message)
    }
}

impl Ticket {
    /// Go's `Parse`. `data` may hold bytes that aren't UTF-8: in the frontmatter
    /// each is read as U+FFFD, as Go's JSON encoding of the hashed fields writes
    /// it, and the fields they were in are remembered for validation. The body's
    /// own bytes are kept, so it hashes as Go's does and is written back as read.
    pub fn parse(data: &[u8]) -> Result<Ticket, Error> {
        let (text, lossy) = match std::str::from_utf8(data) {
            Ok(s) => (s.to_string(), false),
            Err(_) => (gojson::decode::lossy(data), true),
        };
        let doc = Document::parse(&text)?;
        let raw = doc.get("schema");
        let schema = if raw.is_empty() {
            0
        } else {
            go_atoi(&raw).ok_or_else(|| Error::Frontmatter {
                message: format!("ticket: invalid schema {}", gofmt::quote(&raw)),
            })?
        };
        let origin = doc.map("origin").map(|o| Origin {
            workspace: o.get("workspace").cloned().unwrap_or_default(),
            path: o.get("path").cloned().unwrap_or_default(),
        });
        let invalid_utf8 = if lossy {
            let bad = |s: &String| s.contains('\u{FFFD}');
            doc.fields()
                .into_iter()
                .filter(|f| bad(&f.value) || f.items.iter().any(bad) || f.subs.values().any(bad))
                .map(|f| f.key)
                .collect()
        } else {
            Vec::new()
        };
        // The frontmatter is the same bytes in `data` and `text` exactly when it
        // held nothing invalid; then the body starts at the same offset in both.
        let start = text.len() - doc.body.len();
        let lossy_frontmatter = lossy && data.get(..start) != Some(&text.as_bytes()[..start]);
        let raw_body = (!lossy_frontmatter).then(|| data[start..].to_vec());
        Ok(Ticket {
            schema,
            id: doc.get("id"),
            typ: doc.get("type"),
            title: doc.get("title"),
            parent: doc.get("parent"),
            repo: doc.get("repo"),
            depends_on: doc.list("depends_on").unwrap_or_default(),
            supersedes: doc.get("supersedes"),
            change: doc.get("change"),
            spec: doc.get("spec"),
            adrs: doc.list("adrs").unwrap_or_default(),
            requirements: doc.list("requirements").unwrap_or_default(),
            labels: doc.list("labels").unwrap_or_default(),
            estimate: doc.get("estimate"),
            status: doc.get("status"),
            external: doc.map("external").unwrap_or_default(),
            published_hash: doc.get("published_hash"),
            ready_hash: doc.get("ready_hash"),
            origin: origin.unwrap_or_default(),
            body: doc.body.clone(),
            path: String::new(),
            doc,
            invalid_utf8,
            raw_body,
            lossy_frontmatter,
        })
    }

    /// The body as the file holds it: its bytes when they were read from a file,
    /// even those that aren't UTF-8 (S-0007 D2).
    pub fn body_bytes(&self) -> &[u8] {
        self.raw_body.as_deref().unwrap_or(self.body.as_bytes())
    }

    /// Frozen at publish: it has a `published_hash`.
    pub fn frozen(&self) -> bool {
        !self.published_hash.is_empty()
    }

    /// Go's `Drift`: whether the body and the frontmatter changed since the
    /// freeze. A legacy freeze covers the body only; a malformed v2 one counts as
    /// both changed.
    pub fn drift(&self) -> (bool, bool) {
        if !self.frozen() {
            return (false, false);
        }
        if !self.published_hash.starts_with(V2_PREFIX) {
            return (hash_bytes(self.body_bytes()) != self.published_hash, false);
        }
        compare_v2(&self.published_hash, self).unwrap_or((true, true))
    }

    pub fn drifted(&self) -> bool {
        let (b, f) = self.drift();
        b || f
    }

    fn malformed_freeze(&self) -> bool {
        self.published_hash.starts_with(V2_PREFIX) && parse_v2(&self.published_hash).is_none()
    }

    fn readiness_problem(&self) -> String {
        if self.ready_hash.is_empty() {
            return "not readied through the CLI".into();
        }
        match compare_v2(&self.ready_hash, self) {
            None => format!(
                "ready_hash {} isn't a valid v2 hash",
                gofmt::quote(&self.ready_hash)
            ),
            Some((b, f)) if b || f => format!("{} since readied", drift_message(b, f)),
            Some(_) => String::new(),
        }
    }
}

/// Go's `strconv.Atoi`: an optional sign and decimal digits that fit.
pub(crate) fn go_atoi(s: &str) -> Option<i64> {
    let digits = s.strip_prefix(['+', '-']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

const V2_PREFIX: &str = "v2:";

/// Go's `Hash`: SHA-256 of the body with CRLF as LF and exactly one trailing
/// newline, in hex.
pub fn hash(body: &str) -> String {
    hash_bytes(body.as_bytes())
}

/// [`hash`] over bytes, as Go's string hash sees a body that isn't UTF-8.
pub fn hash_bytes(body: &[u8]) -> String {
    let mut normalized = Vec::with_capacity(body.len() + 1);
    let mut i = 0;
    while i < body.len() {
        if body[i] == b'\r' && body.get(i + 1) == Some(&b'\n') {
            i += 1;
            continue;
        }
        normalized.push(body[i]);
        i += 1;
    }
    while normalized.last() == Some(&b'\n') {
        normalized.pop();
    }
    normalized.push(b'\n');
    hex(&Sha256::digest(&normalized))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Go's `ContentHash`: `v2:<body hash[:32]>.<frontmatter hash[:32]>`.
pub fn content_hash(t: &Ticket) -> String {
    let fm = hex(&Sha256::digest(canonical_frontmatter(&t.doc).as_bytes()));
    format!(
        "{V2_PREFIX}{}.{}",
        &hash_bytes(t.body_bytes())[..32],
        &fm[..32]
    )
}

/// Go's `canonicalFrontmatter`: the hashed fields sorted by key, empty values
/// left out, each as `key=<JSON>` with lists sorted.
fn canonical_frontmatter(doc: &Document) -> String {
    let mut fields = doc.fields();
    fields.sort_by(|a, b| a.key.cmp(&b.key));
    let mut out = String::new();
    for f in fields {
        if matches!(
            f.key.as_str(),
            "external" | "published_hash" | "ready_hash" | "status" | "origin"
        ) {
            continue;
        }
        let encoded = match f.kind {
            Kind::Scalar if f.value.is_empty() => continue,
            Kind::Scalar => gojson::marshal_string(&f.value),
            Kind::List if f.items.is_empty() => continue,
            Kind::List => {
                let mut items = f.items.clone();
                items.sort();
                let parts: Vec<_> = items.iter().map(|i| gojson::marshal_string(i)).collect();
                format!("[{}]", parts.join(","))
            }
            Kind::Map => {
                let subs: Vec<_> = f
                    .subs
                    .iter()
                    .filter(|(_, v)| !v.is_empty())
                    .map(|(k, v)| {
                        format!(
                            "{}:{}",
                            gojson::marshal_string(k),
                            gojson::marshal_string(v)
                        )
                    })
                    .collect();
                if subs.is_empty() {
                    continue;
                }
                format!("{{{}}}", subs.join(","))
            }
        };
        out.push_str(&format!("{}={encoded}\n", f.key));
    }
    out
}

fn is_hex32(s: &str) -> bool {
    s.len() == 32
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn parse_v2(v: &str) -> Option<(&str, &str)> {
    let (body, fm) = v.strip_prefix(V2_PREFIX)?.split_once('.')?;
    (is_hex32(body) && is_hex32(fm)).then_some((body, fm))
}

/// Whether the body and the frontmatter differ from a stored v2 hash; `None` when
/// it isn't one.
fn compare_v2(stored: &str, t: &Ticket) -> Option<(bool, bool)> {
    let (b, f) = parse_v2(stored)?;
    let now = content_hash(t);
    let (nb, nf) = parse_v2(&now).expect("content_hash is v2");
    Some((nb != b, nf != f))
}

fn drift_message(body: bool, fm: bool) -> &'static str {
    match (body, fm) {
        (true, true) => "body and frontmatter changed",
        (false, true) => "frontmatter changed",
        _ => "body changed",
    }
}

// Go's `\s` is ASCII whitespace; spelled out, since the regex crate's is Unicode.
static ACCEPTANCE_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^##[\t\n\x0C\r ]+Acceptance criteria[\t\n\x0C\r ]*$").expect("valid pattern")
});
static CHECKBOX_LINE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[\t\n\x0C\r ]*[-*][\t\n\x0C\r ]+\[[ xX]\][\t\n\x0C\r ]+[^\t\n\x0C\r ]")
        .expect("valid pattern")
});
static HEADING_LINE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^##[\t\n\x0C\r ]").expect("valid pattern"));

/// Go's `criteria`: how many acceptance criteria have no placeholder, and those
/// that do (the line, trimmed, and its first placeholder).
fn criteria(body: &str) -> (usize, Vec<(String, String)>) {
    let lines: Vec<&str> = body.split('\n').collect();
    let unr = |l: &&str| -> String { l.strip_suffix('\r').unwrap_or(l).to_string() };
    let Some(start) = lines
        .iter()
        .position(|l| ACCEPTANCE_LINE_RE.is_match(&unr(l)))
        .map(|i| i + 1)
    else {
        return (0, Vec::new());
    };
    let mut first: HashMap<usize, String> = HashMap::new();
    for m in placeholder::find(body) {
        first.entry(m.line).or_insert(m.text);
    }
    let (mut real, mut flagged) = (0, Vec::new());
    for (i, line) in lines.iter().enumerate().skip(start) {
        let line = unr(line);
        if HEADING_LINE_RE.is_match(&line) {
            break;
        }
        if !CHECKBOX_LINE_RE.is_match(&line) {
            continue;
        }
        match first.get(&(i + 1)) {
            Some(p) => flagged.push((line.trim().to_string(), p.clone())),
            None => real += 1,
        }
    }
    (real, flagged)
}

/// Go's `HasAcceptanceCriteria`.
pub fn has_acceptance_criteria(body: &str) -> bool {
    criteria(body).0 > 0
}

/// The last element of a slash path (paths here are shown with `/`).
fn base(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    trimmed.rsplit('/').next().unwrap_or(trimmed)
}

/// Go's `ValidateFile`: what's wrong with one ticket on its own.
pub fn validate_file(t: &Ticket) -> Vec<Problem> {
    let label = if t.id.is_empty() {
        t.path.clone()
    } else {
        t.id.clone()
    };
    let mut problems = Vec::new();
    let mut add = |message: String| problems.push(Problem::new(&label, message));
    let q = gofmt::quote;
    if t.schema != SCHEMA {
        add(format!("schema must be {SCHEMA}, got {}", t.schema));
    }
    if t.title.is_empty() {
        add("title is required".into());
    }
    if rank(&t.typ).is_none() {
        add(format!("unknown type {}", q(&t.typ)));
    }
    let planned = PLANNED_RE.captures(&t.id);
    let pulled = PULLED_RE.is_match(&t.id);
    let no_origin = t.origin == Origin::default();
    if pulled {
        if t.external.len() != 1 {
            add(format!(
                "a pulled {PULLED_PREFIX}- ticket needs exactly one external id, got {}",
                t.external.len()
            ));
        }
        if !no_origin {
            add(format!(
                "a pulled {PULLED_PREFIX}- ticket can't have an origin"
            ));
        }
    } else if let Some(m) = &planned {
        if &m[1] == NATIVE_PREFIX {
            if t.typ != TASK {
                add(format!("a native {NATIVE_PREFIX}- ticket must be a task"));
            }
            if !no_origin {
                add(format!(
                    "a native {NATIVE_PREFIX}- ticket can't have an origin"
                ));
            }
        } else if let Some(want) = prefix_of(&t.typ)
            && &m[1] != want
        {
            add(format!(
                "id prefix {} doesn't match type {} (expected {want})",
                &m[1], t.typ
            ));
        }
    } else {
        add(format!("id {} must look like S-0001 or X-1234", q(&t.id)));
    }
    if !t.path.is_empty() && !t.id.is_empty() {
        let name = base(&t.path);
        if name.strip_suffix(".md").unwrap_or(name) != t.id {
            add(format!("file name {name} doesn't match id"));
        }
    }
    if (t.typ == STORY || t.typ == TASK) && t.repo.is_empty() {
        add(format!("repo is required for a {}", t.typ));
    }
    match t.status.as_str() {
        STATUS_DRAFT => {}
        STATUS_READY => {
            let (real, flagged) = criteria(&t.body);
            if real == 0 {
                add(if flagged.is_empty() {
                    "status ready requires at least one acceptance criterion".into()
                } else {
                    "status ready requires at least one acceptance criterion without a placeholder"
                        .into()
                });
            }
            for (line, p) in &flagged {
                add(format!(
                    "acceptance criterion {} has a placeholder {}",
                    q(line),
                    q(p)
                ));
            }
            let p = t.readiness_problem();
            if !p.is_empty() {
                add(p);
            }
        }
        STATUS_PUBLISHED => {
            if t.external.is_empty() {
                add("status published requires an external id".into());
            }
        }
        STATUS_CLOSED => {
            if pulled || planned.as_ref().is_some_and(|m| &m[1] != NATIVE_PREFIX) {
                add(format!(
                    "only a native {NATIVE_PREFIX}- ticket can be closed; the PM tool owns a published ticket's status"
                ));
            }
        }
        other => add(format!("unknown status {}", q(other))),
    }
    if !matches!(t.estimate.as_str(), "" | "S" | "M" | "L") {
        add(format!(
            "estimate must be S, M or L, got {}",
            q(&t.estimate)
        ));
    }
    if !t.published_hash.is_empty() && t.external.is_empty() {
        add("published_hash requires an external id".into());
    }
    for key in &t.invalid_utf8 {
        add(format!("frontmatter field {key} isn't valid UTF-8"));
    }
    if t.malformed_freeze() {
        add(format!(
            "published_hash {} isn't a valid v2 hash",
            q(&t.published_hash)
        ));
    } else {
        let (b, f) = t.drift();
        if b || f {
            add(format!(
                "drifted: {} since it was published",
                drift_message(b, f)
            ));
        }
    }
    problems
}

/// Indexes of the tickets sorted by id, stably (Go's `sorted`).
fn sorted(tickets: &[&Ticket]) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..tickets.len()).collect();
    idx.sort_by(|&a, &b| tickets[a].id.cmp(&tickets[b].id));
    idx
}

/// Go's `ValidateWorkspace`: every file's problems, duplicate ids, missing
/// references, parents that aren't higher, unregistered repos, and cycles.
pub fn validate_workspace(tickets: &[&Ticket], repos: &[String]) -> Vec<Problem> {
    let mut problems = Vec::new();
    let order = sorted(tickets);
    let mut by_id: HashMap<&str, &Ticket> = HashMap::new();
    for &i in &order {
        let t = tickets[i];
        problems.extend(validate_file(t));
        if by_id.contains_key(t.id.as_str()) && !t.id.is_empty() {
            problems.push(Problem::new(&t.id, "duplicate id".into()));
        }
        by_id.insert(&t.id, t);
    }
    let registered: BTreeSet<&str> = repos.iter().map(String::as_str).collect();
    for &i in &order {
        let t = tickets[i];
        let missing = |problems: &mut Vec<Problem>, field: &str, r: &str| {
            problems.push(Problem {
                id: t.id.clone(),
                message: format!("{field} {r} doesn't exist"),
                reference: r.to_string(),
            })
        };
        if !t.parent.is_empty() {
            match by_id.get(t.parent.as_str()) {
                None => missing(&mut problems, "parent", &t.parent),
                Some(p) if rank(&p.typ) <= rank(&t.typ) => problems.push(Problem::new(
                    &t.id,
                    format!(
                        "parent {} is a {}, which isn't higher than a {}",
                        t.parent, p.typ, t.typ
                    ),
                )),
                Some(_) => {}
            }
        }
        for dep in &t.depends_on {
            if !by_id.contains_key(dep.as_str()) {
                missing(&mut problems, "depends_on", dep);
            }
        }
        if !t.supersedes.is_empty() && !by_id.contains_key(t.supersedes.as_str()) {
            missing(&mut problems, "supersedes", &t.supersedes);
        }
        if !t.repo.is_empty() && !registered.contains(t.repo.as_str()) {
            problems.push(Problem::new(
                &t.id,
                format!("repo {} isn't registered", gofmt::quote(&t.repo)),
            ));
        }
    }
    if let Err(e) = order_tickets(tickets) {
        problems.push(Problem::new("tickets", e));
    }
    problems
}

/// Go's `Order`: dependencies (depends_on, parent, supersedes) first, ties by
/// id; a cycle is an error naming the tickets in it. Returns indexes into
/// `tickets`.
pub fn order_tickets(tickets: &[&Ticket]) -> Result<Vec<usize>, String> {
    let list = sorted(tickets);
    // As in Go, the last ticket sorted with an id stands for it.
    let mut by_id: BTreeMap<&str, usize> = BTreeMap::new();
    for &i in &list {
        by_id.insert(&tickets[i].id, i);
    }
    let mut waiting: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    for &i in &list {
        let t = tickets[i];
        let mut seen = BTreeSet::new();
        let refs = t
            .depends_on
            .iter()
            .map(String::as_str)
            .chain([t.parent.as_str(), t.supersedes.as_str()]);
        for r in refs {
            if r.is_empty() || !seen.insert(r) {
                continue;
            }
            if by_id.contains_key(r) {
                *waiting.entry(&t.id).or_default() += 1;
                dependents.entry(r).or_default().push(&t.id);
            }
        }
    }
    let mut ready: Vec<&str> = list
        .iter()
        .map(|&i| tickets[i].id.as_str())
        .filter(|id| waiting.get(id).copied().unwrap_or(0) == 0)
        .collect();
    let mut placed: BTreeSet<&str> = BTreeSet::new();
    let mut out = Vec::new();
    while !ready.is_empty() {
        ready.sort();
        let id = ready.remove(0);
        if !placed.insert(id) {
            continue;
        }
        out.push(by_id[id]);
        for &next in dependents.get(id).map(Vec::as_slice).unwrap_or(&[]) {
            let w = waiting.entry(next).or_default();
            *w = w.saturating_sub(1);
            if *w == 0 {
                ready.push(next);
            }
        }
    }
    if placed.len() < by_id.len() {
        let stuck: Vec<&str> = by_id
            .keys()
            .copied()
            .filter(|id| !placed.contains(id))
            .collect();
        return Err(format!("dependency cycle among {}", stuck.join(", ")));
    }
    Ok(out)
}

/// Go's `LoadDir`: every `*.md` file in `dir`, by name; files that don't parse
/// are problems. A missing folder is no tickets. `shown` is `dir` as messages
/// print it.
pub fn load_dir(dir: &Path, shown: &str) -> Result<(Vec<Ticket>, Vec<Problem>), Error> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), Vec::new()));
        }
        Err(source) => {
            return Err(Error::Io {
                op: "open",
                path: shown.to_string(),
                source,
            });
        }
    };
    let mut names: Vec<(String, bool)> = entries
        .flatten()
        .map(|e| {
            let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
            (e.file_name().to_string_lossy().into_owned(), is_dir)
        })
        .collect();
    names.sort();
    let (mut tickets, mut problems) = (Vec::new(), Vec::new());
    for (name, is_dir) in names {
        if is_dir || !name.ends_with(".md") {
            continue;
        }
        let path = gofmt::join(&[shown, name.as_str()]);
        let data = std::fs::read(dir.join(&name)).map_err(|source| Error::Io {
            op: "open",
            path: path.clone(),
            source,
        })?;
        match Ticket::parse(&data) {
            Ok(mut t) => {
                t.path = path;
                tickets.push(t);
            }
            Err(e) => problems.push(Problem::new(&path, e.to_string())),
        }
    }
    Ok((tickets, problems))
}

mod write;
pub use write::*;

#[cfg(test)]
mod tests;
