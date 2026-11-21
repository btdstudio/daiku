//! The architecture's `## Diagrams` section. A port of Go's
//! `internal/plan/diagrams.go`.

use crate::markdown;

/// One `### ` subsection of `## Diagrams`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// The text after `### `, such as `Flow: approving a gate`.
    pub heading: String,
    /// The heading's line, 1-based.
    pub line: usize,
    /// The body of the subsection's first closed mermaid fence, or "".
    pub source: String,
}

pub const DIAGRAMS_SECTION: &str = "## Diagrams";

/// Go's `Diagrams`: whether the section exists, and its diagrams.
pub fn diagrams(text: &str) -> (bool, Vec<Diagram>) {
    let lines = markdown::scan(text);
    if !lines
        .iter()
        .any(|l| markdown::is_heading(l, DIAGRAMS_SECTION))
    {
        return (false, Vec::new());
    }
    let mut ds: Vec<Diagram> = Vec::new();
    let mut cur: Option<usize> = None;
    let mut seen = false;
    let mut body: Vec<&str> = Vec::new();
    let mut in_mermaid = false;
    for l in markdown::section_lines(&lines, DIAGRAMS_SECTION) {
        if l.prose() && l.text.starts_with("### ") {
            ds.push(Diagram {
                heading: l.text[4..].trim().to_string(),
                line: l.n,
                source: String::new(),
            });
            cur = Some(ds.len() - 1);
            seen = false;
            continue;
        }
        let Some(c) = cur else { continue };
        if l.open {
            in_mermaid = !seen && l.info.split_whitespace().next() == Some("mermaid");
            seen = seen || in_mermaid;
            body.clear();
        } else if l.close {
            if in_mermaid {
                let src = body.join("\n");
                if !src.trim().is_empty() {
                    ds[c].source = src;
                }
            }
            in_mermaid = false;
        } else if in_mermaid {
            body.push(&l.text);
        }
    }
    (true, ds)
}
