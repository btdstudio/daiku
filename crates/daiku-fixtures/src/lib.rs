//! Golden fixture runner for daiku binaries.
//!
//! A case is a folder holding `case.toml`, `before/`, `config/` and `expect/`. The
//! runner copies its starting files (a shared workspace, then `before/`) into a sealed
//! temp workspace, runs the binary, normalises what it saw, and compares that with
//! `expect/` or writes it. Shared workspaces are built from recipes.

mod case;
mod compare;
mod coverage;
mod normalize;
mod observe;
mod recipe;
mod report;
mod run;
mod stored;

pub use case::{Case, CaseSpec, Os, load_cases};
pub use compare::{Difference, compare, write_expect};
pub use coverage::{uncovered, usage_entries};
#[doc(hidden)]
pub use normalize::civil;
pub use normalize::{FIXED_DATE, FIXED_TIMESTAMP};
pub use observe::Observation;
pub use recipe::{Recipe, RecipeFile, build_workspaces};
pub use report::{Outcome, Summary, case_report};
pub use run::{DEFAULT_TIMEOUT, KEEP_MARKER, date_warnings, run_case, run_case_with_timeout};
pub use stored::STORED_SUFFIX;

use std::fmt;
use std::path::{Path, PathBuf};

/// A failure to load or run a case: where, and what went wrong.
#[derive(Debug)]
pub struct Error {
    pub path: Option<PathBuf>,
    pub message: String,
}

impl Error {
    pub fn at(path: &Path, message: impl fmt::Display) -> Error {
        Error {
            path: Some(path.to_path_buf()),
            message: message.to_string(),
        }
    }

    pub fn msg(message: impl fmt::Display) -> Error {
        Error {
            path: None,
            message: message.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.path {
            Some(p) => write!(f, "{}: {}", p.display(), self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl std::error::Error for Error {}
