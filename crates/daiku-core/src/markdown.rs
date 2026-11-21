//! Markdown lines, with fenced code and HTML comments marked (R10). A port of Go's
//! `internal/plan/markdown.go`, using the same rules as [`crate::placeholder`] so
//! the two agree on what's hidden.

/// One line of a markdown document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// 1-based.
    pub n: usize,
    /// The line without its line ending.
    pub text: String,
    /// The fence's opening and closing lines and every line between them.
    pub in_fence: bool,
    /// The line starts inside an HTML comment.
    pub in_comment: bool,
    pub open: bool,
    pub close: bool,
    /// An opening fence line's info string.
    pub info: String,
}

impl Line {
    /// Neither fenced code nor inside an HTML comment, so its text counts.
    pub fn prose(&self) -> bool {
        !self.in_fence && !self.in_comment
    }
}

/// Splits markdown into lines and marks fenced code and HTML comments. A fence
/// opens with any indentation and three or more backticks or tildes (a backtick
/// fence's info string can't contain a backtick) and closes with at least as many
/// of the same character and nothing else; it can't open inside a comment. Fences
/// and comments left unclosed run to the end.
pub fn scan(markdown: &str) -> Vec<Line> {
    let mut out = Vec::new();
    let mut fence: Option<(u8, usize)> = None;
    let mut in_comment = false;
    for (i, text) in markdown.split('\n').enumerate() {
        let text = text.strip_suffix('\r').unwrap_or(text);
        let mut l = Line {
            n: i + 1,
            text: text.to_string(),
            in_fence: false,
            in_comment,
            open: false,
            close: false,
            info: String::new(),
        };
        if let Some((fc, fl)) = fence {
            l.in_fence = true;
            if let Some((c, n, rest)) = fence_marker(text)
                && c == fc
                && n >= fl
                && rest.trim().is_empty()
            {
                l.close = true;
                fence = None;
            }
        } else if let Some((c, n, rest)) = fence_marker(text)
            && !in_comment
            && (c == b'~' || !rest.contains('`'))
        {
            l.in_fence = true;
            l.open = true;
            l.info = rest.trim().to_string();
            fence = Some((c, n));
        } else {
            in_comment = ends_in_comment(text, in_comment);
        }
        out.push(l);
    }
    out
}

/// Any indentation, then a run of three or more backticks or tildes; returns the
/// character, the run's length and what follows.
fn fence_marker(text: &str) -> Option<(u8, usize, &str)> {
    let s = text.trim_start_matches([' ', '\t']);
    let c = *s.as_bytes().first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = s.bytes().take_while(|&b| b == c).count();
    if n < 3 {
        return None;
    }
    Some((c, n, &s[n..]))
}

/// Whether a line ends inside an HTML comment, given whether it starts inside one.
/// It reads left to right, and an inline code span hides a `<!--` inside it.
pub fn ends_in_comment(text: &str, mut in_comment: bool) -> bool {
    let t = text.as_bytes();
    let mut i = 0;
    while i < t.len() {
        if in_comment {
            match t[i..].windows(3).position(|w| w == b"-->") {
                None => return true,
                Some(j) => {
                    i += j + 3;
                    in_comment = false;
                }
            }
            continue;
        }
        let rest = &t[i..];
        if t[i] == b'`' {
            let n = rest.iter().take_while(|&&b| b == b'`').count();
            i = closing_ticks(t, i + n, n).unwrap_or(i + n);
        } else if rest.starts_with(b"<!-->") {
            i += 5;
        } else if rest.starts_with(b"<!--->") {
            i += 6;
        } else if rest.starts_with(b"<!--") {
            i += 4;
            in_comment = true;
        } else {
            i += 1;
        }
    }
    in_comment
}

fn closing_ticks(t: &[u8], from: usize, n: usize) -> Option<usize> {
    let mut i = from;
    while i < t.len() {
        if t[i] != b'`' {
            i += 1;
            continue;
        }
        let m = t[i..].iter().take_while(|&&b| b == b'`').count();
        if m == n {
            return Some(i + m);
        }
        i += m;
    }
    None
}

/// A level-1 or level-2 heading outside fences and comments, which ends a section.
pub fn ends_section(l: &Line) -> bool {
    l.prose() && (l.text.starts_with("# ") || l.text.starts_with("## "))
}

/// Exactly the given heading, outside fences and comments; trailing spaces allowed.
pub fn is_heading(l: &Line, heading: &str) -> bool {
    l.prose() && l.text.trim_end_matches([' ', '\t']) == heading
}

/// The lines of every section headed by `heading`, up to the next `# ` or `## `
/// heading outside fences and comments, without the heading lines.
pub fn section_lines<'a>(lines: &'a [Line], heading: &str) -> Vec<&'a Line> {
    let mut out = Vec::new();
    let mut inside = false;
    for l in lines {
        if is_heading(l, heading) {
            inside = true;
            continue;
        }
        if ends_section(l) {
            inside = false;
            continue;
        }
        if inside {
            out.push(l);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fenced(doc: &str) -> Vec<usize> {
        scan(doc)
            .iter()
            .filter(|l| l.in_fence)
            .map(|l| l.n)
            .collect()
    }

    // markdown_test.go, TestScanFences.
    #[test]
    fn scan_fences() {
        for (name, doc, want) in [
            ("backticks", "a\n```\nx\n```\nb", vec![2, 3, 4]),
            ("tildes with info", "~~~text\nx\n~~~\nb", vec![1, 2, 3]),
            ("longer close", "```\nx\n`````\nb", vec![1, 2, 3]),
            (
                "shorter run doesn't close",
                "````\n```\nx\n````\nb",
                vec![1, 2, 3, 4],
            ),
            (
                "other char doesn't close",
                "```\n~~~\nx\n```\nb",
                vec![1, 2, 3, 4],
            ),
            (
                "close with text doesn't close",
                "```\n``` go\nx\n```\nb",
                vec![1, 2, 3, 4],
            ),
            (
                "indented, in a list item",
                "- a\n    ```\n    x\n    ```\nb",
                vec![2, 3, 4],
            ),
            ("two ticks aren't a fence", "``\nx\n``", vec![]),
            ("backtick in info isn't a fence", "``` a`b\nx", vec![]),
            ("unclosed runs to the end", "a\n```\nx\n## y", vec![2, 3, 4]),
            ("crlf", "```\r\nx\r\n```\r\nb", vec![1, 2, 3]),
        ] {
            assert_eq!(fenced(doc), want, "{name}");
        }
    }

    #[test]
    fn scan_trims_cr() {
        assert_eq!(scan("a\r\nb")[0].text, "a");
        let l = &scan("~~~text\nx\n~~~")[0];
        assert!(l.open && l.info == "text");
        assert!(scan("~~~text\nx\n~~~")[2].close);
    }

    #[test]
    fn section_lines_match_go() {
        let doc = [
            "# Title",
            "## Wanted  ",
            "a",
            "### Sub",
            "b",
            "```",
            "## not a heading",
            "```",
            "## Other",
            "c",
            "## Wanted",
            "d",
            "# Ends too",
            "e",
        ]
        .join("\n");
        let lines = scan(&doc);
        let got: Vec<usize> = section_lines(&lines, "## Wanted")
            .iter()
            .map(|l| l.n)
            .collect();
        assert_eq!(got, [3, 4, 5, 6, 7, 8, 12]);
        assert!(section_lines(&lines, "## Missing").is_empty());
    }

    #[test]
    fn ends_in_comment_matches_go() {
        for (text, inside, want) in [
            ("plain", false, false),
            ("<!-- open", false, true),
            ("<!-- closed -->", false, false),
            ("still inside", true, true),
            ("closes --> after", true, false),
            ("closes --> then <!-- opens", true, true),
            ("<!--> empty", false, false),
            ("<!---> empty", false, false),
            ("`<!--` in code", false, false),
            ("``a ` <!-- b`` in code", false, false),
            ("` unmatched <!--", false, true),
            (
                "`a`` <!-- b` a longer run doesn't close the span",
                false,
                false,
            ),
        ] {
            assert_eq!(ends_in_comment(text, inside), want, "{text:?}");
        }
    }

    #[test]
    fn scan_comments() {
        let lines = scan("a\n<!--\n```\n-->\n```\nx\n```");
        let comment: Vec<usize> = lines.iter().filter(|l| l.in_comment).map(|l| l.n).collect();
        let fence: Vec<usize> = lines.iter().filter(|l| l.in_fence).map(|l| l.n).collect();
        assert_eq!((comment, fence), (vec![3, 4], vec![5, 6, 7]));
    }
}
