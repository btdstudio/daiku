//! The handle every operation goes through (ADR-0003, D2).

use std::path::{Path, PathBuf};

use crate::scaffold::{self, InitOptions, InitReport, RoomReport, UpdateReport};
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

    /// `daiku init` here: the scaffold, seeds, manifest and `AGENTS.md`.
    pub fn init(&self, opts: &InitOptions) -> Result<Outcome<InitReport>, Error> {
        scaffold::init(&self.root, opts, &self.env).map(Outcome::new)
    }

    /// `daiku update` here: refresh managed files and merge `AGENTS.md`.
    pub fn update(&self, version: &str) -> Result<Outcome<UpdateReport>, Error> {
        scaffold::update(&self.root, &self.env.dirs, version).map(Outcome::new)
    }

    /// Creates the `plan` or `build` room if it's missing.
    pub fn ensure_room(&self, room: &str, version: &str) -> Result<Outcome<RoomReport>, Error> {
        scaffold::ensure_room(&self.root, &self.env.dirs, room, version).map(Outcome::new)
    }
}
