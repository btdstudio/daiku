//! Observing a run: outputs, the workspace tree and the git log.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use crate::normalize::Normalizer;
use crate::run::{RawRun, git};

/// What a run produced, normalised by `run_case`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// As the binary wrote them: bytes, valid UTF-8 or not.
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit: i32,
    /// Every path in the workspace, sorted: `d path`, `f path` or `l path -> target`,
    /// with forward slashes. `.git/` is left out.
    pub manifest: Vec<String>,
    /// Each file's content, by its manifest path.
    pub files: BTreeMap<String, Vec<u8>>,
    /// What the run changed in the workspace, compared with its starting files:
    /// `+ <entry>` added, `- <entry>` removed, `~ <entry>` a file whose content or a
    /// link whose target changed. Sorted by path; a removal comes before an addition
    /// at the same path. This, not the whole tree, is what `[expect]` records.
    pub changes: Vec<String>,
    /// For git cases: `git log --reverse --name-status --format=%s` after the base.
    pub git_log: Option<String>,
    /// Every commit id in the workspace repo, used to normalise (D6).
    pub commits: Vec<String>,
}

/// The workspace as it was just before the binary ran.
pub(crate) struct Snapshot {
    manifest: Vec<String>,
    files: BTreeMap<String, Vec<u8>>,
}

/// Records the workspace's entries and file contents (`.git/` left out).
pub(crate) fn snapshot(root: &Path) -> Result<Snapshot, String> {
    let mut manifest = Vec::new();
    let mut files = BTreeMap::new();
    walk(root, "", &mut manifest, &mut files)?;
    Ok(Snapshot { manifest, files })
}

/// The changes from `start` to the given tree, as `Observation::changes` describes.
/// Contents are compared as raw bytes, before any normalising.
pub(crate) fn changes(
    start: &Snapshot,
    manifest: &[String],
    files: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let by_path = |m: &[String]| -> BTreeMap<String, String> {
        m.iter()
            .map(|e| (entry_path(e).to_string(), e.clone()))
            .collect()
    };
    let (before, after) = (by_path(&start.manifest), by_path(manifest));
    let mut out = Vec::new();
    let paths: std::collections::BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    for path in paths {
        match (before.get(path), after.get(path)) {
            (None, Some(a)) => out.push(format!("+ {a}")),
            (Some(b), None) => out.push(format!("- {b}")),
            (Some(b), Some(a)) if b[..2] != a[..2] => {
                out.push(format!("- {b}"));
                out.push(format!("+ {a}"));
            }
            (Some(b), Some(a)) if a.starts_with("l ") && a != b => out.push(format!("~ {a}")),
            (Some(_), Some(a))
                if a.starts_with("f ") && start.files.get(path) != files.get(path) =>
            {
                out.push(format!("~ {a}"))
            }
            _ => {}
        }
    }
    out
}

pub(crate) fn observe(
    root: &Path,
    raw: RawRun,
    start: &Snapshot,
    base: Option<&str>,
    git_env: Option<&[(String, OsString)]>,
) -> Result<Observation, String> {
    let mut manifest = Vec::new();
    let mut files = BTreeMap::new();
    walk(root, "", &mut manifest, &mut files)?;
    manifest.sort_by(|a, b| entry_path(a).cmp(entry_path(b)));
    let changes = changes(start, &manifest, &files);

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
        stdout: raw.stdout,
        stderr: raw.stderr,
        exit: raw.exit,
        manifest,
        files,
        changes,
        git_log,
        commits,
    })
}

/// The path part of a manifest entry.
pub(crate) fn entry_path(entry: &str) -> &str {
    let rest = &entry[2..];
    rest.split(" -> ").next().unwrap_or(rest)
}

pub(crate) fn walk(
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
        self.stdout = n.apply_bytes(&self.stdout);
        self.stderr = n.apply_bytes(&self.stderr);
        self.git_log = self.git_log.map(|l| n.apply(&l));
        for bytes in self.files.values_mut() {
            *bytes = n.apply_bytes(bytes);
        }
        self
    }

    /// stdout as text, lossily, for messages and tests.
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// stderr as text, lossily, for messages and tests.
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(entries: &[(&str, &str)]) -> (Vec<String>, BTreeMap<String, Vec<u8>>) {
        let mut manifest = Vec::new();
        let mut files = BTreeMap::new();
        for (entry, content) in entries {
            manifest.push(entry.to_string());
            if let Some(path) = entry.strip_prefix("f ") {
                files.insert(path.to_string(), content.as_bytes().to_vec());
            }
        }
        (manifest, files)
    }

    #[test]
    fn changes_cover_add_remove_edit_and_kind() {
        let (m, f) = tree(&[
            ("f same.txt", "s"),
            ("f edit.txt", "old"),
            ("f gone.txt", "g"),
            ("f retype", "was a file"),
            ("l link -> same.txt", ""),
            ("d keep", ""),
        ]);
        let start = Snapshot {
            manifest: m,
            files: f,
        };
        let (m, f) = tree(&[
            ("f same.txt", "s"),
            ("f edit.txt", "new"),
            ("d retype", ""),
            ("f retype/x", "x"),
            ("l link -> edit.txt", ""),
            ("d keep", ""),
            ("f added.txt", "a"),
        ]);
        assert_eq!(
            changes(&start, &m, &f),
            [
                "+ f added.txt",
                "~ f edit.txt",
                "- f gone.txt",
                "~ l link -> edit.txt",
                "- f retype",
                "+ d retype",
                "+ f retype/x",
            ]
        );
        assert!(changes(&start, &start.manifest, &start.files).is_empty());
    }
}
