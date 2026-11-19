//! The handle every operation goes through (ADR-0003, D2).

use std::path::{Path, PathBuf};

use crate::settings::{self, Loaded};
use crate::{Env, Error, Outcome};

/// A daiku workspace at a root folder, with the `Env` its operations use. Opening
/// does no I/O.
pub struct Workspace {
    root: PathBuf,
    env: Env,
}

impl Workspace {
    /// The CLI opens `.`, as Go does, so paths in messages are relative.
    pub fn open(root: impl Into<PathBuf>, env: Env) -> Workspace {
        Workspace {
            root: root.into(),
            env,
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn env(&self) -> &Env {
        &self.env
    }

    /// The effective settings and the layers they came from.
    pub fn settings(&self) -> Result<Outcome<Loaded>, Error> {
        settings::load(&self.root, &self.env.dirs).map(Outcome::new)
    }
}
