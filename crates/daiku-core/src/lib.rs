//! daiku's core. It holds no UI: every operation returns data and never prints.
//!
//! Callers open a [`Workspace`] with an [`Env`], which carries everything that
//! varies between runs and clients: the clock, external processes, progress events
//! and the home and config folders (ADR-0003). Operations return
//! `Result<Outcome<T>, Error>`; an [`Error`]'s `Display` is the Go CLI's message.

pub mod changes;
pub mod date;
pub mod env;
pub mod error;
pub mod fake;
pub mod frontmatter;
pub mod gofmt;
pub mod gojson;
pub mod inbox;
pub mod json;
pub mod lock;
pub mod markdown;
pub mod outcome;
pub mod placeholder;
pub mod plan;
pub mod repos;
pub mod room;
pub mod scaffold;
pub mod settings;
pub mod state;
#[cfg(test)]
pub(crate) mod testutil;
pub mod text;
pub mod ticket;
pub mod tickets;
pub mod workspace;
pub mod write;

pub use env::{Clock, Cmd, Dirs, Env, Event, Events, Output, Process};
pub use error::Error;
pub use outcome::{Outcome, Warning};
pub use workspace::Workspace;

/// The crate's version, as Cargo built it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_set() {
        assert!(!super::VERSION.is_empty());
    }
}
