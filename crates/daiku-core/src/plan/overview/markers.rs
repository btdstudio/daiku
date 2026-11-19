//! The README's markers: a narrative the author writes and a generated region
//! daiku owns. A port of Go's `internal/plan/overview/markers.go`.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

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
fn markers(ls: &[&str]) -> Result<HashMap<&'static str, usize>, String> {
    let mut at: HashMap<&'static str, usize> = HashMap::new();
    for (i, l) in ls.iter().enumerate() {
        for m in ORDER {
            if !l.starts_with(m) {
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

/// Go's `Merge`: `existing` with its generated region replaced.
pub fn merge(existing: &str, region: &str) -> Result<String, String> {
    let ls: Vec<&str> = existing.split_inclusive('\n').collect();
    let at = markers(&ls)?;
    let mut out: String = ls[..=at[GENERATED_START]].concat();
    out.push_str(region);
    out.push_str(&ls[at[GENERATED_END]..].concat());
    Ok(out)
}

/// Go's `Narrative`: the lines between the narrative markers.
pub fn narrative(existing: &str) -> Result<String, String> {
    let ls: Vec<&str> = existing.split_inclusive('\n').collect();
    let at = markers(&ls)?;
    Ok(ls[at[NARRATIVE_START] + 1..at[NARRATIVE_END]].concat())
}

static HTML_COMMENT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<!--.*?-->").expect("valid pattern"));

/// Go's `NarrativeEmpty`: nothing but comments and space.
pub fn narrative_empty(narrative: &str) -> bool {
    HTML_COMMENT.replace_all(narrative, "").trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_keeps_the_narrative() {
        let readme = stub("T").replace("<!-- About 250 words", "Our story.\n<!-- About 250 words");
        let merged = merge(&readme, "\nGEN\n").unwrap();
        assert!(merged.contains("Our story.\n"));
        assert!(
            merged.contains(
                "do not edit; run `daiku overview` -->\n\nGEN\n<!-- /daiku:generated -->\n"
            )
        );
        assert_eq!(merge(&merged, "\nGEN\n").unwrap(), merged);
        assert!(!narrative_empty(&narrative(&readme).unwrap()));
        assert!(narrative_empty(&narrative(&stub("T")).unwrap()));
    }

    #[test]
    fn marker_problems_have_gos_messages() {
        assert_eq!(
            merge("# T\n", "").unwrap_err(),
            "missing marker <!-- daiku:narrative -->"
        );
        let dup = stub("T") + "<!-- daiku:narrative -->\n";
        assert_eq!(
            merge(&dup, "").unwrap_err(),
            "duplicated marker <!-- daiku:narrative --> (lines 3 and 9)"
        );
        let swapped = "<!-- /daiku:narrative -->\n<!-- daiku:narrative -->\n<!-- daiku:generated -->\n<!-- /daiku:generated -->\n";
        assert_eq!(
            merge(swapped, "").unwrap_err(),
            "markers out of order: <!-- /daiku:narrative --> (line 1) comes before <!-- daiku:narrative --> (line 2)"
        );
    }
}
