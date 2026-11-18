//! Shared starting workspaces, built from recipes by running the binary (D2, D3).
//!
//! `workspaces/<name>/recipe.toml` says how to make the workspace; `tree/` and
//! `config/` beside it are the stored result, with fixed dates and path tokens.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

use crate::case::{no_git_config, no_symlink, plain_name};
use crate::normalize::{Normalizer, unix_now};
use crate::run::{DEFAULT_TIMEOUT, KEEP_MARKER, Sandbox, copy_tree, git_base, spawn};
use crate::{Error, observe};

/// The contents of `recipe.toml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// A workspace to start from; empty when not set.
    #[serde(default)]
    pub from: Option<String>,
    /// Run in a git repo (the base commit is made before the steps).
    #[serde(default)]
    pub git: bool,
    /// Argument lists for the binary, run in order; each must exit 0.
    #[serde(default)]
    pub steps: Vec<Vec<String>>,
    /// Files written after the steps. A path under `config/` goes to the config folder.
    #[serde(default)]
    pub files: Vec<RecipeFile>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeFile {
    pub path: String,
    pub text: String,
}

fn load_recipe(dir: &Path, name: &str) -> Result<Recipe, Error> {
    plain_name(name).map_err(|e| Error::at(dir, e))?;
    let wdir = dir.join(name);
    no_symlink(&wdir)?;
    let file = wdir.join("recipe.toml");
    if no_symlink(&file)?.is_none() {
        return Err(Error::at(&file, "no such recipe"));
    }
    let text = std::fs::read_to_string(&file).map_err(|e| Error::at(&file, e))?;
    let recipe: Recipe = toml::from_str(&text).map_err(|e| Error::at(&file, e))?;
    if let Some(from) = &recipe.from {
        plain_name(from).map_err(|e| Error::at(&file, format!("from: {e}")))?;
    }
    for f in &recipe.files {
        relative_path(&f.path).map_err(|e| Error::at(&file, e))?;
        let mut parts = f.path.split('/');
        let first = parts.next().unwrap_or_default();
        let second = parts.next().unwrap_or_default();
        if (first == "config" && second.eq_ignore_ascii_case("git"))
            || f.path.split('/').any(|p| p.eq_ignore_ascii_case(".git"))
        {
            return Err(Error::at(
                &file,
                format!("files: {:?} would be git's config or repo", f.path),
            ));
        }
    }
    Ok(recipe)
}

/// A `[[files]]` path: relative, slash-separated, no `..`, `.` or empty parts.
fn relative_path(path: &str) -> Result<(), String> {
    let bad = path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..");
    if bad {
        return Err(format!(
            "files: {path:?} must be a relative path inside the workspace"
        ));
    }
    Ok(())
}

/// Builds the named workspaces under `dir` (every recipe when `names` is empty), plus
/// any parent they need whose `tree/` is missing, parents first. Returns the names
/// built, in order.
pub fn build_workspaces(bin: &Path, dir: &Path, names: &[String]) -> Result<Vec<String>, Error> {
    let bin = std::path::absolute(bin).map_err(|e| Error::at(bin, e))?;
    let mut all = BTreeMap::new();
    for entry in std::fs::read_dir(dir).map_err(|e| Error::at(dir, e))? {
        let entry = entry.map_err(|e| Error::at(dir, e))?;
        if entry.path().join("recipe.toml").exists() {
            let name = entry.file_name().to_string_lossy().into_owned();
            all.insert(name.clone(), load_recipe(dir, &name)?);
        }
    }
    for n in names {
        if !all.contains_key(n) {
            return Err(Error::at(&dir.join(n), "no such recipe"));
        }
    }
    let wanted: Vec<String> = if names.is_empty() {
        all.keys().cloned().collect()
    } else {
        names.to_vec()
    };

    let mut order = Vec::new();
    let mut done = BTreeSet::new();
    for name in &wanted {
        visit(
            name,
            &all,
            dir,
            names.is_empty(),
            true,
            &mut Vec::new(),
            &mut done,
            &mut order,
        )?;
    }
    for name in &order {
        build_one(&bin, dir, name, &all[name])?;
    }
    Ok(order)
}

/// Depth-first by `from`. A parent is added when everything is being built, when it
/// was asked for, or when its `tree/` is missing.
#[allow(clippy::too_many_arguments)]
fn visit(
    name: &str,
    all: &BTreeMap<String, Recipe>,
    dir: &Path,
    every: bool,
    asked: bool,
    path: &mut Vec<String>,
    done: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), Error> {
    if done.contains(name) {
        return Ok(());
    }
    if path.iter().any(|p| p == name) {
        path.push(name.to_string());
        return Err(Error::at(
            dir,
            format!("recipes loop: {}", path.join(" -> ")),
        ));
    }
    let recipe = all
        .get(name)
        .ok_or_else(|| Error::at(&dir.join(name), "no such recipe"))?;
    if !(every || asked || !dir.join(name).join("tree").is_dir()) {
        return Ok(());
    }
    path.push(name.to_string());
    if let Some(from) = &recipe.from {
        if !all.contains_key(from) {
            return Err(Error::at(
                &dir.join(name).join("recipe.toml"),
                format!("from: no recipe named {from:?}"),
            ));
        }
        visit(from, all, dir, every, false, path, done, order)?;
    }
    path.pop();
    done.insert(name.to_string());
    order.push(name.to_string());
    Ok(())
}

fn build_one(bin: &Path, dir: &Path, name: &str, recipe: &Recipe) -> Result<(), Error> {
    let wdir = dir.join(name);
    let start = unix_now();
    let sb = Sandbox::new()?;
    if let Some(from) = &recipe.from {
        let pdir = dir.join(from);
        let label = format!("workspaces/{from}");
        copy_tree(&pdir.join("tree"), &sb.root, &label, &|s| sb.expand(s))?;
        no_git_config(&pdir.join("config"))?;
        copy_tree(&pdir.join("config"), &sb.config, &label, &|s| sb.expand(s))?;
    }
    let env = sb.base_env(recipe.git);
    if recipe.git {
        git_base(&sb.root, &env)?;
    }
    for (i, step) in recipe.steps.iter().enumerate() {
        let mut cmd = Command::new(bin);
        cmd.args(step.iter().map(|a| sb.expand(a)))
            .current_dir(&sb.root)
            .env_clear()
            .envs(env.iter().map(|(k, v)| (k, v)));
        let raw = spawn(cmd, DEFAULT_TIMEOUT).map_err(|e| Error::at(&wdir, e))?;
        if raw.exit != 0 {
            return Err(Error::at(
                &wdir.join("recipe.toml"),
                format!(
                    "step {} ({}) exited {}: {}",
                    i + 1,
                    step.join(" "),
                    raw.exit,
                    String::from_utf8_lossy(&raw.stderr).trim()
                ),
            ));
        }
    }
    for f in &recipe.files {
        let path = match f.path.strip_prefix("config/") {
            Some(rest) => sb.config.join(rest),
            None => sb.root.join(&f.path),
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::at(parent, e))?;
        }
        std::fs::write(&path, sb.expand(&f.text)).map_err(|e| Error::at(&path, e))?;
    }
    let norm = Normalizer::fixed(&sb, &[start, unix_now()]);
    store(&sb.root, &wdir.join("tree"), &norm, true)?;
    store(&sb.config, &wdir.join("config"), &norm, false)
}

/// Replaces `to` with a normalised copy of `from` (without `.git/`). Empty folders
/// get the keep marker. With `always` false, an empty `from` leaves no `to` at all.
fn store(from: &Path, to: &Path, norm: &Normalizer, always: bool) -> Result<(), Error> {
    if no_symlink(to)?.is_some() {
        std::fs::remove_dir_all(to).map_err(|e| Error::at(to, e))?;
    }
    let mut manifest = Vec::new();
    let mut files = BTreeMap::new();
    observe::walk(from, "", &mut manifest, &mut files).map_err(Error::msg)?;
    if manifest.is_empty() && !always {
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| Error::at(to, e))?;
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    let mut parents: BTreeSet<PathBuf> = BTreeSet::new();
    for entry in &manifest {
        let rel = observe::entry_path(entry);
        let path = to.join(rel);
        if let Some(p) = Path::new(rel).parent() {
            parents.insert(p.to_path_buf());
        }
        match &entry[..2] {
            "d " => {
                std::fs::create_dir_all(&path).map_err(|e| Error::at(&path, e))?;
                dirs.insert(PathBuf::from(rel));
            }
            "f " => {
                let bytes = &files[rel];
                let bytes = match std::str::from_utf8(bytes) {
                    Ok(t) => norm.apply(t).into_bytes(),
                    Err(_) => bytes.clone(),
                };
                std::fs::write(&path, bytes).map_err(|e| Error::at(&path, e))?;
            }
            _ => {
                let target = entry.split(" -> ").nth(1).unwrap_or_default();
                let depth = Path::new(rel).components().count() - 1;
                if !crate::run::link_stays_inside(Path::new(target), depth) {
                    return Err(Error::at(
                        &from.join(rel),
                        format!("symlink to {target} leaves the workspace"),
                    ));
                }
                crate::run::symlink(Path::new(target), &path, from.join(rel).is_dir())
                    .map_err(|e| Error::at(&path, e))?;
            }
        }
    }
    for d in dirs.difference(&parents) {
        let marker = to.join(d).join(KEEP_MARKER);
        std::fs::write(&marker, "").map_err(|e| Error::at(&marker, e))?;
    }
    if manifest.is_empty() {
        let marker = to.join(KEEP_MARKER);
        std::fs::write(&marker, "").map_err(|e| Error::at(&marker, e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_paths_stay_inside() {
        for ok in ["a", "a/b.json", "config/daiku/settings.json", ".daiku/x"] {
            assert!(relative_path(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "/etc/x",
            "../x",
            "a/../../x",
            "a//b",
            "./a",
            r"a\b",
            "C:/x",
        ] {
            assert!(relative_path(bad).is_err(), "{bad}");
        }
    }
}
