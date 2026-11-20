//! The README's markers: a narrative the author writes and a generated region
//! daiku owns. A port of Go's `internal/plan/overview/markers.go`.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::bytes::Regex;

const NARRATIVE_START: &str = "<!-- daiku:narrative";
const NARRATIVE_END: &str = "<!-- /daiku:narrative";
const GENERATED_START: &str = "<!-- daiku:generated";
const GENERATED_END: &str = "<!-- /daiku:generated";
const ORDER: [&str; 4] = [
    NARRATIVE_START,
    NARRATIVE_END,
    GENERATED_START,
    GENERATED_END,
];

/// Go's `Stub`: a new README with empty regions.
pub fn stub(title: &str) -> String {
    format!(
        "# {title}\n\n{NARRATIVE_START} -->\n\
         <!-- About 250 words: the problem, the shape of the solution, who's affected, and what's out of scope. -->\n\
         {NARRATIVE_END} -->\n\n\
         {GENERATED_START} — do not edit; run `daiku overview` -->\n\
         {GENERATED_END} -->\n"
    )
}

/// Go's `markers`: each marker's line index, or Go's message for a missing,
/// duplicated or misordered one.
fn markers(ls: &[&[u8]]) -> Result<HashMap<&'static str, usize>, String> {
    let mut at: HashMap<&'static str, usize> = HashMap::new();
    for (i, l) in ls.iter().enumerate() {
        for m in ORDER {
            if !l.starts_with(m.as_bytes()) {
                continue;
            }
            if let Some(prev) = at.get(m) {
                return Err(format!(
                    "duplicated marker {m} --> (lines {} and {})",
                    prev + 1,
                    i + 1
                ));
            }
            at.insert(m, i);
        }
    }
    for m in ORDER {
        if !at.contains_key(m) {
            return Err(format!("missing marker {m} -->"));
        }
    }
    for w in ORDER.windows(2) {
        let (a, b) = (w[0], w[1]);
        if at[b] < at[a] {
            return Err(format!(
                "markers out of order: {b} --> (line {}) comes before {a} --> (line {})",
                at[b] + 1,
                at[a] + 1
            ));
        }
    }
    Ok(at)
}

/// `b`'s lines, each with its `\n`, as Go's `bytes.SplitAfter`.
fn lines(b: &[u8]) -> Vec<&[u8]> {
    b.split_inclusive(|c| *c == b'\n').collect()
}

/// Go's `Merge`: `existing` with its generated region replaced. Every byte outside
/// the region is kept as it is, valid UTF-8 or not, as Go's `[]byte` merge keeps
/// it (N-0001).
pub fn merge(existing: &[u8], region: &str) -> Result<Vec<u8>, String> {
    let ls = lines(existing);
    let at = markers(&ls)?;
    let mut out: Vec<u8> = ls[..=at[GENERATED_START]].concat();
    out.extend_from_slice(region.as_bytes());
    out.extend_from_slice(&ls[at[GENERATED_END]..].concat());
    Ok(out)
}

/// Go's `Narrative`: the lines between the narrative markers.
pub fn narrative(existing: &[u8]) -> Result<Vec<u8>, String> {
    let ls = lines(existing);
    let at = markers(&ls)?;
    Ok(ls[at[NARRATIVE_START] + 1..at[NARRATIVE_END]].concat())
}

/// `<!--…-->` with any bytes between: without Unicode mode `.` matches any byte,
/// as Go's `regexp` matches an invalid byte as U+FFFD.
static HTML_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s-u)<!--.*?-->").expect("valid pattern"));

/// Go's `NarrativeEmpty`: nothing but comments and space.
pub fn narrative_empty(narrative: &[u8]) -> bool {
    crate::gofmt::trim_space(&HTML_COMMENT.replace_all(narrative, &b""[..])).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_the_narrative() {
        let readme = stub("T").replace("<!-- About 250 words", "Our story.\n<!-- About 250 words");
        let merged = String::from_utf8(merge(readme.as_bytes(), "\nGEN\n").unwrap()).unwrap();
        assert!(merged.contains("Our story.\n"));
        assert!(
            merged.contains(
                "do not edit; run `daiku overview` -->\n\nGEN\n<!-- /daiku:generated -->\n"
            )
        );
        assert_eq!(
            merge(merged.as_bytes(), "\nGEN\n").unwrap(),
            merged.as_bytes()
        );
        assert!(!narrative_empty(&narrative(readme.as_bytes()).unwrap()));
        assert!(narrative_empty(&narrative(stub("T").as_bytes()).unwrap()));
    }

    #[test]
    fn marker_problems_have_gos_messages() {
        assert_eq!(
            merge(b"# T\n", "").unwrap_err(),
            "missing marker <!-- daiku:narrative -->"
        );
        let dup = stub("T") + "<!-- daiku:narrative -->\n";
        assert_eq!(
            merge(dup.as_bytes(), "").unwrap_err(),
            "duplicated marker <!-- daiku:narrative --> (lines 3 and 9)"
        );
        let swapped = "<!-- /daiku:narrative -->\n<!-- daiku:narrative -->\n<!-- daiku:generated -->\n<!-- /daiku:generated -->\n";
        assert_eq!(
            merge(swapped.as_bytes(), "").unwrap_err(),
            "markers out of order: <!-- /daiku:narrative --> (line 1) comes before <!-- daiku:narrative --> (line 2)"
        );
    }

    /// N-0001: bytes that aren't valid UTF-8 in the title line, the narrative and
    /// after the region survive a merge; the region is replaced.
    #[test]
    fn merge_keeps_bytes_outside_the_region() {
        let head =
            &b"# Caf\xe9\n\n<!-- daiku:narrative -->\nOurs \xff.\n<!-- /daiku:narrative -->\n"[..];
        let readme = [
            head,
            b"<!-- daiku:generated -->\nold \xfe\n<!-- /daiku:generated -->\nAfter \xe9\n",
        ]
        .concat();
        let want = [
            head,
            b"<!-- daiku:generated -->\n\nNEW\n<!-- /daiku:generated -->\nAfter \xe9\n",
        ]
        .concat();
        assert_eq!(merge(&readme, "\nNEW\n").unwrap(), want);
        assert_eq!(narrative(&readme).unwrap(), b"Ours \xff.\n");
    }

    /// Go's emptiness: a comment hides any bytes in it; an invalid byte outside
    /// one is content, and isn't white space.
    #[test]
    fn narrative_empty_on_bytes() {
        assert!(narrative_empty(b"<!-- note \xff -->\n \t\n"));
        assert!(narrative_empty(b"\xc2\xa0<!-- a -->\n"));
        assert!(!narrative_empty(b"<!-- a -->\xff\n"));
        assert!(!narrative_empty(b"words\n"));
    }
}
