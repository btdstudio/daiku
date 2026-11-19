//! The plan room: what gates and the overview read from a plan (D7). A port of the
//! parts of Go's `internal/plan` that S-0005 needs.

pub mod adr;
pub mod check;
pub mod diagrams;
pub mod model;
pub mod problems;
pub mod source;
pub mod spec;

/// The plan room's folder.
pub const ROOM: &str = "plan";
/// One folder per plan change: `plan/specs/<n>-<slug>/`.
pub const SPECS_DIR: &str = crate::state::PLAN_DIR;

#[cfg(test)]
mod tests;
