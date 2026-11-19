//! Changes and their gates: the phase machines, `STATE.json`, the active
//! change, `LOG.md`, and transitions with their commits.
//!
//! A port of Go's `internal/core/state` (the generic machine and store) together
//! with `internal/state` (daiku's two loops), without the generic layer (D1).
//!
//! Layout:
//!
//! ```text
//! STATE.json      every change's record: { schema, changes }, committed
//! LOG.md          one timestamped line per state change (committed)
//! .daiku/active   the active change's name (gitignored)
//! plan/specs/<n>-<slug>/          a plan change's folder
//! build/<repo>/changes/<ticket>/  a build change's folder
//! ```

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;

pub mod changelog;
pub mod file;
pub mod machine;
pub mod overview;
pub mod store;

pub use machine::{Kind, Machine, Phase};
pub use store::{Known, Store};

/// The loop names a change records.
pub const PLAN_LOOP: &str = "plan";
pub const BUILD_LOOP: &str = "build";

/// Where plan changes live, as `plan/specs/<n>-<slug>/`.
pub const PLAN_DIR: &str = "plan/specs";

/// Folders that hold one subfolder per plan change, committed with its gates.
pub const PLAN_CHANGE_DIRS: [&str; 2] = ["plan/01_research", "plan/02_explore"];

/// Reasons for the skips `change new` records.
pub const NO_REPO_REASON: &str = "no repo in the intake";
pub const SINGLE_REPO_REASON: &str = "fewer than two repos in scope";

/// A plan change's name: `<n>-<slug>`.
pub static PLAN_NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+)-[a-z0-9]+(?:-[a-z0-9]+)*$").expect("valid pattern"));

/// A change folder's leading number.
pub(crate) static CHANGE_ID_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([0-9]+)-").expect("valid pattern"));

/// One change's progress through its loop. `gates` holds only started phases.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Change {
    pub name: String,
    /// `plan` or `build`; empty for a change that isn't recorded or found.
    pub loop_: String,
    pub phase: String,
    pub status: String,
    pub gates: BTreeMap<String, String>,
    /// Why each skipped phase was skipped.
    pub reasons: BTreeMap<String, String>,
    pub sources: Vec<String>,
    /// Logical repo names from `REPOS.json`.
    pub repos: Vec<String>,
    pub updated_at: String,
}

impl Change {
    /// Go's `NewChange`.
    pub fn new(name: &str) -> Change {
        Change {
            name: name.to_string(),
            ..Change::default()
        }
    }

    pub fn is_closed(&self) -> bool {
        self.status == machine::CLOSED
    }

    pub fn is_build(&self) -> bool {
        self.loop_ == BUILD_LOOP
    }

    /// A phase's status, "" when it hasn't started.
    pub fn gate(&self, phase: &str) -> &str {
        self.gates.get(phase).map(String::as_str).unwrap_or("")
    }

    /// The machine of this change's loop (Go's `MachineFor`: plan unless build).
    pub fn machine(&self) -> &'static Machine {
        machine_for(&self.loop_)
    }
}

/// Go's `MachineFor`.
pub fn machine_for(loop_: &str) -> &'static Machine {
    if loop_ == BUILD_LOOP {
        &machine::BUILD
    } else {
        &machine::PLAN
    }
}

/// Go's `Slugify`: lower case, runs of anything but `a-z0-9` as `-`, at most 40
/// characters, no leading or trailing `-`.
pub fn slugify(text: &str) -> String {
    let lower = go_to_lower(text);
    let mut slug = String::new();
    let mut in_run = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
            in_run = false;
        } else if !in_run {
            slug.push('-');
            in_run = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    let cut: String = if trimmed.chars().count() > 40 {
        trimmed.chars().take(40).collect()
    } else {
        slug
    };
    cut.trim_matches('-').to_string()
}

/// Go's `strings.ToLower` for what `slugify` needs: anything that lowers to ASCII
/// `a-z` matters, and only ASCII letters, `İ` (U+0130 → `i`) and the Kelvin sign
/// (U+212A → `k`) lower to one. Other runes join a `-` run either way.
fn go_to_lower(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{130}' => 'i',
            '\u{212A}' => 'k',
            c => c.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
