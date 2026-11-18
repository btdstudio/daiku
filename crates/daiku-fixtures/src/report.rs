//! Per-case result lines and the summary.

use crate::{Difference, Error, Os};

/// How one case went.
#[derive(Debug)]
pub enum Outcome {
    Pass,
    Fail(Vec<Difference>),
    /// The case couldn't run (bad case, timeout, missing git).
    Error(Error),
    Skip(Os),
}

/// The lines printed for one case: `ok NAME`, `skip NAME (os)`, or `FAIL NAME`
/// followed by each differing part and its diff, indented.
pub fn case_report(name: &str, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Pass => format!("ok   {name}\n"),
        Outcome::Skip(os) => format!("skip {name} ({})\n", os.name()),
        Outcome::Error(e) => format!("FAIL {name}\n  error: {e}\n"),
        Outcome::Fail(diffs) => {
            let mut s = format!("FAIL {name}\n");
            for d in diffs {
                s.push_str(&format!("  {}:\n", d.part));
                for line in d.detail.lines() {
                    s.push_str(&format!("    {line}\n"));
                }
            }
            s
        }
    }
}

/// Counts of passed, failed and skipped cases.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

impl Summary {
    pub fn add(&mut self, outcome: &Outcome) {
        match outcome {
            Outcome::Pass => self.passed += 1,
            Outcome::Fail(_) | Outcome::Error(_) => self.failed += 1,
            Outcome::Skip(_) => self.skipped += 1,
        }
    }

    pub fn line(&self) -> String {
        format!(
            "{} passed, {} failed, {} skipped\n",
            self.passed, self.failed, self.skipped
        )
    }
}
