//! Template placeholders in markdown (R10). A port of Go's
//! `internal/core/placeholder`.
//!
//! Two slot styles, found outside code:
//! - an ellipsis, `…` or `...`, standing alone with whitespace or the line's ends on
//!   both sides (`etc...` and `wait…` are prose);
//! - `<…>` on one line with at least one letter and no `<` or `>` inside, except
//!   closing tags, comments and doctypes, autolinks, and common HTML elements.
//!
//! Nothing in fenced code, inline code spans or HTML comments is reported. Unclosed
//! fences and comments run to the end of the document.

/// One placeholder: its 1-based line and the text as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub line: usize,
    pub text: String,
}

/// The placeholders in a markdown document, in order.
pub fn find(markdown: &str) -> Vec<Match> {
    let mut out = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut in_comment = false;
    for (i, line) in markdown.split('\n').enumerate() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some((c, n)) = fence {
            if fence_close(line, c, n) {
                fence = None;
            }
            continue;
        }
        if !in_comment && let Some(open) = fence_open(line) {
            fence = Some(open);
            continue;
        }
        let (mut masked, still) = mask(line, in_comment);
        in_comment = still;
        for (a, b) in slots(line, &mut masked) {
            out.push(Match {
                line: i + 1,
                text: line[a..b].to_string(),
            });
        }
    }
    out
}

/// Three or more backticks or tildes after leading whitespace; a backtick fence's
/// info string may not contain a backtick.
pub(crate) fn fence_open(line: &str) -> Option<(u8, usize)> {
    let s = line.trim_start_matches([' ', '\t']).as_bytes();
    let c = *s.first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = run(s, 0, c);
    if n < 3 || (c == b'`' && s[n..].contains(&b'`')) {
        return None;
    }
    Some((c, n))
}

/// At least `n` of the fence character, then only whitespace.
pub(crate) fn fence_close(line: &str, c: u8, n: usize) -> bool {
    let s = line.trim_start_matches([' ', '\t']);
    let m = run(s.as_bytes(), 0, c);
    m >= n && s[m..].trim().is_empty()
}

fn run(s: &[u8], i: usize, c: u8) -> usize {
    s[i..].iter().take_while(|&&b| b == c).count()
}

/// `line` with inline code spans and HTML comments replaced by spaces, byte for
/// byte, and whether the line ends inside a comment.
fn mask(line: &str, mut in_comment: bool) -> (Vec<u8>, bool) {
    let l = line.as_bytes();
    let mut b = l.to_vec();
    let blank = |b: &mut Vec<u8>, from: usize, to: usize| b[from..to].fill(b' ');
    let mut i = 0;
    while i < l.len() {
        if in_comment {
            match find_bytes(&l[i..], b"-->") {
                None => {
                    blank(&mut b, i, l.len());
                    return (b, true);
                }
                Some(j) => {
                    blank(&mut b, i, i + j + 3);
                    i += j + 3;
                    in_comment = false;
                }
            }
            continue;
        }
        let rest = &l[i..];
        if l[i] == b'`' {
            let n = run(l, i, b'`');
            match closing_ticks(l, i + n, n) {
                Some(end) => {
                    blank(&mut b, i, end);
                    i = end;
                }
                None => i += n, // an unmatched run is literal
            }
        } else if rest.starts_with(b"<!-->") || rest.starts_with(b"<!--->") {
            // Empty comments: the closing "-->" overlaps the opener.
            let n = rest.iter().position(|&c| c == b'>').unwrap_or(0) + 1;
            blank(&mut b, i, i + n);
            i += n;
        } else if rest.starts_with(b"<!--") {
            blank(&mut b, i, i + 4);
            i += 4;
            in_comment = true;
        } else {
            i += 1;
        }
    }
    (b, in_comment)
}

fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// The offset just past the next run of exactly `n` backticks from `i`.
fn closing_ticks(l: &[u8], mut i: usize, n: usize) -> Option<usize> {
    while i < l.len() {
        if l[i] != b'`' {
            i += 1;
            continue;
        }
        let m = run(l, i, b'`');
        if m == n {
            return Some(i + m);
        }
        i += m;
    }
    None
}

/// The byte ranges of the placeholders in `line`, in column order.
fn slots(line: &str, masked: &mut [u8]) -> Vec<(usize, usize)> {
    let angles = angle_slots(masked);
    for &(a, b) in &angles {
        masked[a..b].fill(b' '); // an ellipsis inside a slot isn't reported twice
    }
    let dots = ellipses(line, masked);
    let mut out = Vec::with_capacity(angles.len() + dots.len());
    let (mut a, mut d) = (angles.into_iter().peekable(), dots.into_iter().peekable());
    loop {
        match (a.peek(), d.peek()) {
            (None, None) => break,
            (Some(x), Some(y)) if x.0 < y.0 => out.push(a.next().unwrap_or_default()),
            (Some(_), None) => out.push(a.next().unwrap_or_default()),
            _ => out.push(d.next().unwrap_or_default()),
        }
    }
    out
}

fn angle_slots(m: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < m.len() {
        if m[i] != b'<' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < m.len() && m[j] != b'>' && m[j] != b'<' {
            j += 1;
        }
        if j == m.len() {
            break;
        }
        if m[j] == b'<' {
            i = j;
            continue;
        }
        if is_slot(&String::from_utf8_lossy(&m[i + 1..j])) {
            out.push((i, j + 1));
        }
        i = j + 1;
    }
    out
}

const HTML_ELEMENTS: [&str; 26] = [
    "a", "b", "br", "code", "details", "div", "em", "hr", "i", "img", "kbd", "li", "ol", "p",
    "pre", "span", "strong", "sub", "summary", "sup", "table", "td", "th", "tr", "u", "ul",
];

fn is_slot(inner: &str) -> bool {
    if !inner.chars().any(char::is_alphabetic) {
        return false;
    }
    if inner.starts_with(['/', '!']) {
        return false;
    }
    let lower = inner.to_lowercase();
    if inner.contains("://") || inner.contains('@') || lower.starts_with("mailto:") {
        return false;
    }
    let k = lower
        .bytes()
        .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .count();
    if k > 0 && HTML_ELEMENTS.contains(&&lower[..k]) {
        let rest = &inner[k..];
        if rest.is_empty() || rest.starts_with(['/', ' ', '\t']) {
            return false;
        }
    }
    true
}

/// Standalone `...` and `…` tokens. Tokens are found in the masked line, but
/// standing alone is judged against the original, so "`x`..." stays prose.
fn ellipses(line: &str, masked: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let m = masked;
    let mut i = 0;
    while i < m.len() {
        let (r, size) = decode(&m[i..]);
        if r.is_whitespace() {
            i += size;
            continue;
        }
        let mut j = i;
        while j < m.len() {
            let (r, size) = decode(&m[j..]);
            if r.is_whitespace() {
                break;
            }
            j += size;
        }
        let tok = &m[i..j];
        if (tok == b"..." || tok == "…".as_bytes()) && space_before(line, i) && space_after(line, j)
        {
            out.push((i, j));
        }
        i = j;
    }
    out
}

/// Go's `utf8.DecodeRune`: an invalid byte reads as U+FFFD, one byte long.
fn decode(b: &[u8]) -> (char, usize) {
    let len = match b[0] {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => return ('\u{fffd}', 1),
    };
    match b.get(..len).and_then(|s| std::str::from_utf8(s).ok()) {
        Some(s) => (s.chars().next().unwrap_or('\u{fffd}'), len),
        None => ('\u{fffd}', 1),
    }
}

fn space_before(s: &str, i: usize) -> bool {
    i == 0 || s[..i].chars().next_back().is_some_and(char::is_whitespace)
}

fn space_after(s: &str, j: usize) -> bool {
    j == s.len() || s[j..].chars().next().is_some_and(char::is_whitespace)
}

#[cfg(test)]
mod tests;
