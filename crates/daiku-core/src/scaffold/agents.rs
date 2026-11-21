//! `AGENTS.md`: assembled from the scaffold's fragments and merged into a user's
//! edited copy (D9, R7). A port of Go's `internal/agents`.

use std::path::Path;

use crate::scaffold::engine;
use crate::{Error, write};

/// The rooms with fragments, in the order they're joined.
pub const ROOMS: [&str; 2] = ["plan", "build"];

/// The `##` sections a user fills in; a merge keeps their text.
const USER_OWNED: [&str; 2] = ["Tech stack & standards", "Model routing"];

/// Headings an earlier scaffold had and the current one dropped. None yet.
const RETIRED: [&str; 0] = [];

/// `root.md` plus the fragments of the given rooms, in [`ROOMS`] order, each with its
/// trailing newlines trimmed, joined by a blank line, with a final newline.
pub fn assemble(rooms: &[&str]) -> String {
    let mut parts = vec![
        engine::agents_file("root.md")
            .unwrap_or_default()
            .trim_end_matches('\n'),
    ];
    for r in ROOMS {
        if rooms.contains(&r)
            && let Some(text) = engine::agents_file(&format!("{r}.md"))
        {
            parts.push(text.trim_end_matches('\n'));
        }
    }
    format!("{}\n", parts.join("\n\n"))
}

/// One line, as the file holds it: bytes, valid UTF-8 or not (N-0001).
type Line = Vec<u8>;

#[derive(Debug, Clone)]
struct Section {
    /// Decoded and trimmed, for matching; the lines keep the bytes.
    heading: String,
    /// The heading line through the last non-blank line.
    body: Vec<Line>,
    /// The blank lines after it.
    trail: Vec<Line>,
}

/// Go's `strings.TrimSpace(line) == ""` on bytes.
fn blank(line: &[u8]) -> bool {
    crate::gofmt::trim_space(line).is_empty()
}

#[derive(Debug, Clone, Default)]
struct Doc {
    preamble: Vec<Line>,
    sections: Vec<Section>,
    final_newline: bool,
    unclosed_fence: bool,
}

/// Up to three spaces, then three or more backticks or tildes: the character and
/// run length.
fn fence_of(line: &[u8]) -> Option<(u8, usize)> {
    let b = trim_start(line, b' ');
    if line.len() - b.len() > 3 || b.len() < 3 || (b[0] != b'`' && b[0] != b'~') {
        return None;
    }
    let n = b.iter().take_while(|&&c| c == b[0]).count();
    (n >= 3).then_some((b[0], n))
}

/// `b` without leading `c` bytes.
fn trim_start(b: &[u8], c: u8) -> &[u8] {
    let n = b.iter().take_while(|&&x| x == c).count();
    &b[n..]
}

fn parse(text: &[u8]) -> Doc {
    let mut doc = Doc {
        final_newline: text.ends_with(b"\n"),
        ..Doc::default()
    };
    if text.is_empty() {
        return doc;
    }
    let mut fence: Option<(u8, usize)> = None;
    let mut cur: Option<Section> = None;
    let flush = |cur: &mut Option<Section>, doc: &mut Doc| {
        if let Some(mut s) = cur.take() {
            let mut end = s.body.len();
            while end > 1 && blank(&s.body[end - 1]) {
                end -= 1;
            }
            s.trail = s.body.split_off(end);
            doc.sections.push(s);
        }
    };
    for line in text
        .strip_suffix(b"\n")
        .unwrap_or(text)
        .split(|b| *b == b'\n')
    {
        if fence.is_none()
            && let Some(rest) = line.strip_prefix(b"## ")
        {
            flush(&mut cur, &mut doc);
            let heading = String::from_utf8_lossy(rest);
            cur = Some(Section {
                heading: heading.trim_end_matches('\r').trim().to_string(),
                body: vec![line.to_vec()],
                trail: Vec::new(),
            });
            continue;
        }
        if let Some((ch, n)) = fence_of(line) {
            match fence {
                None => fence = Some((ch, n)),
                Some((fc, fl)) => {
                    let after = trim_start(trim_start(line, b' '), ch);
                    if ch == fc && n >= fl && blank(after) {
                        fence = None;
                    }
                }
            }
        }
        match cur.as_mut() {
            None => doc.preamble.push(line.to_vec()),
            Some(s) => s.body.push(line.to_vec()),
        }
    }
    flush(&mut cur, &mut doc);
    doc.unclosed_fence = fence.is_some();
    doc
}

impl Doc {
    fn render(&self) -> Vec<u8> {
        let mut lines: Vec<&[u8]> = self.preamble.iter().map(Vec::as_slice).collect();
        for s in &self.sections {
            lines.extend(s.body.iter().map(Vec::as_slice));
            lines.extend(s.trail.iter().map(Vec::as_slice));
        }
        let mut out = lines.join(&b'\n');
        if self.final_newline {
            out.push(b'\n');
        }
        out
    }
}

/// daiku's sections take the scaffold's text; a missing scaffold section, daiku's or
/// user-owned, goes after its nearest preceding scaffold section (N-0010); everything
/// else is kept as written.
/// `Err(reason)` when it can't merge. The user's lines are kept byte for byte,
/// valid UTF-8 or not (N-0001).
pub fn merge(user: &[u8], scaffold: &str) -> Result<Vec<u8>, String> {
    merge_with(user, scaffold, &USER_OWNED, &RETIRED)
}

fn merge_with(
    user: &[u8],
    scaffold: &str,
    user_owned: &[&str],
    retired: &[&str],
) -> Result<Vec<u8>, String> {
    let (u, s) = (parse(user), parse(scaffold.as_bytes()));
    if u.unclosed_fence {
        return Err("unclosed code fence".into());
    }
    if u.sections.is_empty() {
        return Err("no ## headings".into());
    }
    let owned = |h: &str| s.sections.iter().any(|x| x.heading == h) && !user_owned.contains(&h);
    let scaf_by = |h: &str| s.sections.iter().find(|x| x.heading == h);
    let mut seen = std::collections::HashSet::new();
    for sec in &u.sections {
        if owned(&sec.heading) && !seen.insert(sec.heading.clone()) {
            return Err(format!(
                "duplicated heading {}",
                crate::gofmt::quote(&sec.heading)
            ));
        }
    }
    let mut out = Doc {
        sections: Vec::new(),
        ..u.clone()
    };
    for sec in &u.sections {
        if retired.contains(&sec.heading.as_str()) {
            continue;
        }
        if owned(&sec.heading) {
            out.sections.push(Section {
                heading: sec.heading.clone(),
                body: scaf_by(&sec.heading)
                    .map(|x| x.body.clone())
                    .unwrap_or_default(),
                trail: sec.trail.clone(),
            });
        } else {
            out.sections.push(sec.clone());
        }
    }
    let index_of = |out: &Doc, h: &str| out.sections.iter().position(|x| x.heading == h);
    for (i, sec) in s.sections.iter().enumerate() {
        if index_of(&out, &sec.heading).is_some() {
            continue;
        }
        let at = (0..i)
            .rev()
            .find_map(|j| index_of(&out, &s.sections[j].heading))
            .map_or(0, |k| k + 1);
        let mut ins = sec.clone();
        if at == 0 {
            if out.preamble.last().is_some_and(|l| !blank(l)) {
                out.preamble.push(Vec::new());
            }
        } else if out.sections[at - 1].trail.is_empty() {
            out.sections[at - 1].trail = vec![Vec::new()];
        }
        if at < out.sections.len() && ins.trail.is_empty() {
            ins.trail = vec![Vec::new()];
        }
        out.sections.insert(at, ins);
    }
    Ok(out.render())
}

/// What happened to `AGENTS.md`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentsResult {
    Written,
    Merged,
    Unchanged,
    /// Left alone: Go's reason.
    Unmergeable(String),
}

/// Go's `WriteAgents`: write the assembled file when it's missing (or with
/// `force`), else merge it.
pub fn write_agents(root: &Path, rooms: &[&str], force: bool) -> Result<AgentsResult, Error> {
    if let Some(link) = write::through_symlink(root, "AGENTS.md") {
        return Err(Error::WriteThroughSymlink {
            rel: "AGENTS.md".into(),
            link,
        });
    }
    let assembled = assemble(rooms);
    let path = root.join("AGENTS.md");
    let shown = engine::shown(root, "AGENTS.md");
    let io = |op: &'static str| {
        let shown = shown.clone();
        move |source| Error::Io {
            op,
            path: shown,
            source,
        }
    };
    let user = match std::fs::read(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(io("open")(e)),
        Ok(b) => Some(b),
    };
    let user = match user {
        Some(b) if !force => b,
        _ => {
            std::fs::write(&path, &assembled).map_err(io("open"))?;
            return Ok(AgentsResult::Written);
        }
    };
    match merge(&user, &assembled) {
        Err(reason) => Ok(AgentsResult::Unmergeable(reason)),
        Ok(out) if out == user => Ok(AgentsResult::Unchanged),
        Ok(out) => {
            std::fs::write(&path, out).map_err(io("open"))?;
            Ok(AgentsResult::Merged)
        }
    }
}

#[cfg(test)]
mod tests;
