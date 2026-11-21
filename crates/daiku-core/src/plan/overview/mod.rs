//! A plan's README overview: a narrative the author keeps and a region daiku
//! regenerates from the plan (D7). A port of Go's `internal/plan/overview`.

pub mod markers;
pub mod render;

pub use render::{BuildRow, Input, render};

#[cfg(test)]
mod tests;
