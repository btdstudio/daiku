//! Fixture cases: a folder holding `case.toml`, `before/` and `expect/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Error;

/// An operating system a case can be skipped on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Os {
    Macos,
    Linux,
    Windows,
}

impl Os {
    /// The OS this binary was built for, if it's one cases can name.
    pub fn current() -> Option<Os> {
        match std::env::consts::OS {
            "macos" => Some(Os::Macos),
            "linux" => Some(Os::Linux),
            "windows" => Some(Os::Windows),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Os::Macos => "macos",
            Os::Linux => "linux",
            Os::Windows => "windows",
        }
    }
}

/// The contents of `case.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseSpec {
    /// Arguments passed to the binary. `{ROOT}`, `{HOME}` and `{CONFIG}` expand.
    pub args: Vec<String>,
    /// Extra environment variables. Values expand like `args`.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    /// Compare stdout lines as a sorted list.
    #[serde(default)]
    pub unordered_stdout: bool,
    /// Run in a git repo and record `git-log`.
    #[serde(default)]
    pub git: bool,
    /// Operating systems the case doesn't run on.
    #[serde(default)]
    pub skip_on: Vec<Os>,
    /// Recorded differences from the Go CLI, as free text.
    #[serde(default)]
    pub deviations: String,
}

/// A loaded case.
#[derive(Debug, Clone)]
pub struct Case {
    pub name: String,
    pub dir: PathBuf,
    pub spec: CaseSpec,
}

impl Case {
    /// The starting workspace. A missing folder means an empty workspace, since git
    /// can't store an empty one.
    pub fn before_dir(&self) -> PathBuf {
        self.dir.join("before")
    }

    pub fn expect_dir(&self) -> PathBuf {
        self.dir.join("expect")
    }

    /// The OS the case is skipped on here, if any.
    pub fn skipped_here(&self) -> Option<Os> {
        let os = Os::current()?;
        self.spec.skip_on.contains(&os).then_some(os)
    }

    /// Loads one case folder.
    ///
    /// The case folder, `case.toml`, `before` and `expect` may not be symlinks: a
    /// committed case must not reach outside itself.
    pub fn load(dir: &Path) -> Result<Case, Error> {
        if no_symlink(dir)?.is_none() {
            return Err(Error::at(dir, "no such case"));
        }
        let file = dir.join("case.toml");
        no_symlink(&file)?;
        let text = std::fs::read_to_string(&file).map_err(|e| Error::at(&file, e))?;
        let spec: CaseSpec = toml::from_str(&text).map_err(|e| Error::at(&file, e))?;
        for sub in ["before", "expect"] {
            let path = dir.join(sub);
            if let Some(meta) = no_symlink(&path)?
                && !meta.is_dir()
            {
                return Err(Error::at(&path, "not a folder"));
            }
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Case {
            name,
            dir: dir.to_path_buf(),
            spec,
        })
    }
}

/// Loads every case under `dir` (folders holding `case.toml`), sorted by name, or
/// only the named ones when `names` isn't empty.
pub fn load_cases(dir: &Path, names: &[String]) -> Result<Vec<Case>, Error> {
    if !names.is_empty() {
        return names
            .iter()
            .map(|n| {
                let d = dir.join(n);
                if d.join("case.toml").is_file() {
                    Case::load(&d)
                } else {
                    Err(Error::at(&d, "no such case"))
                }
            })
            .collect();
    }
    let entries = std::fs::read_dir(dir).map_err(|e| Error::at(dir, e))?;
    let mut dirs = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| Error::at(dir, e))?.path();
        if path.join("case.toml").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs.iter().map(|d| Case::load(d)).collect()
}

/// The metadata of `path` without following a symlink, `None` if it doesn't exist, or
/// an error if it is a symlink.
pub(crate) fn no_symlink(path: &Path) -> Result<Option<std::fs::Metadata>, Error> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => Err(Error::at(
            path,
            "is a symlink; a case may not link outside itself",
        )),
        Ok(m) => Ok(Some(m)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::at(path, e)),
    }
}
