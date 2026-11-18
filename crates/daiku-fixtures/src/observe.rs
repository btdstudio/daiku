//! Observing a run: outputs, the workspace tree and the git log.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use crate::normalize::Normalizer;
use crate::run::{RawRun, git};

/// What a run produced, normalised by `run_case`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub stdout: String,
    pub stderr: String,
    pub exit: i32,
    /// Every path in the workspace, sorted: `d path`, `f path` or `l path -> target`,
    /// with forward slashes. `.git/` is left out.
    pub manifest: Vec<String>,
    /// Each file's content, by its manifest path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// For git cases: `git log --reverse --name-status --format=%s` after the base.
    pub git_log: Option<String>,
    /// Every commit id in the workspace repo, used to normalise (D6).
    pub commits: Vec<String>,
}

pub(crate) fn observe(
    root: &Path,
    raw: RawRun,
    base: Option<&str>,
    git_env: Option<&[(String, OsString)]>,
) -> Result<Observation, String> {
    let mut manifest = Vec::new();
    let mut files = BTreeMap::new();
    walk(root, "", &mut manifest, &mut files)?;
    manifest.sort_by(|a, b| entry_path(a).cmp(entry_path(b)));

    let (git_log, commits) = match (base, git_env) {
        (Some(base), Some(env)) => {
            let range = format!("{base}..HEAD");
            let log = git(
                root,
                env,
                &["log", "--reverse", "--name-status", "--format=%s", &range],
            )?;
            let ids = git(root, env, &["rev-list", "--all"])?;
            (
                Some(log),
                ids.split_whitespace().map(str::to_string).collect(),
            )
        }
        _ => (None, Vec::new()),
    };

    Ok(Observation {
        stdout: String::from_utf8_lossy(&raw.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&raw.stderr).into_owned(),
        exit: raw.exit,
        manifest,
        files,
        git_log,
        commits,
    })
}

/// The path part of a manifest entry.
pub(crate) fn entry_path(entry: &str) -> &str {
    let rest = &entry[2..];
    rest.split(" -> ").next().unwrap_or(rest)
}

fn walk(
    dir: &Path,
    prefix: &str,
    manifest: &mut Vec<String>,
    files: &mut BTreeMap<String, Vec<u8>>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if prefix.is_empty() && name == ".git" {
            continue;
        }
        let rel = format!("{prefix}{name}");
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if ft.is_symlink() {
            let target =
                std::fs::read_link(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let target = target.to_string_lossy().replace('\\', "/");
            manifest.push(format!("l {rel} -> {target}"));
        } else if ft.is_dir() {
            manifest.push(format!("d {rel}"));
            walk(&path, &format!("{rel}/"), manifest, files)?;
        } else {
            manifest.push(format!("f {rel}"));
            let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            files.insert(rel, bytes);
        }
    }
    Ok(())
}

impl Observation {
    /// Applies the normaliser to every text output (R6).
    pub(crate) fn normalized(mut self, n: &Normalizer) -> Observation {
        self.stdout = n.apply(&self.stdout);
        self.stderr = n.apply(&self.stderr);
        self.git_log = self.git_log.map(|l| n.apply(&l));
        for bytes in self.files.values_mut() {
            if let Ok(text) = std::str::from_utf8(bytes) {
                *bytes = n.apply(text).into_bytes();
            }
        }
        self
    }
}
