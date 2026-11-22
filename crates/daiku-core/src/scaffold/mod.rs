//! The workspace scaffold: `init`, `update`, `check`, rooms and `AGENTS.md` (S-0004).
//! A port of Go's `internal/icm`, configured for daiku over the generic engine.

pub mod agents;
pub(crate) mod engine;
pub(crate) mod lint;

use std::path::Path;

pub use engine::Manifest;
pub(crate) use engine::template;

use crate::env::{Dirs, Env};
use crate::{Error, settings};
use agents::AgentsResult;

/// What `init` did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InitReport {
    /// Files written, in order: the root tree, the seeds, the vcs record, AGENTS.md.
    pub written: Vec<String>,
    /// Files left alone because they couldn't be merged, with the reason.
    pub not_merged: Vec<(String, String)>,
    pub git_init: bool,
}

/// What `update` did; each list is sorted by path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UpdateReport {
    pub updated: Vec<String>,
    pub merged: Vec<String>,
    pub skipped: Vec<String>,
    pub unchanged: Vec<String>,
    pub not_merged: Vec<(String, String)>,
}

/// What creating a room did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoomReport {
    pub created: bool,
    pub written: Vec<String>,
}

/// `init`'s options. `vcs` is `git` or `none` (see [`init_vcs`]); `version` goes into
/// the manifest.
#[derive(Debug, Clone)]
pub struct InitOptions {
    pub no_git: bool,
    pub force: bool,
    pub vcs: String,
    pub version: String,
}

/// Go's `InitVCS`: `none` with `--no-git`, else the global settings' `vcs`
/// (default `git`).
pub fn init_vcs(no_git: bool, dirs: &Dirs) -> Result<String, Error> {
    if no_git {
        return Ok("none".into());
    }
    let global = settings::read_if_exists(&settings::global_path(dirs))?;
    settings::vcs_of(&[&global])
}

/// Go's `CheckLegacy`: a zumen or tomo workspace isn't a daiku one.
pub fn check_legacy(root: &Path) -> Result<(), Error> {
    for tool in ["zumen", "tomo"] {
        if std::fs::metadata(root.join(format!(".{tool}"))).is_ok_and(|m| m.is_dir()) {
            return Err(Error::LegacyWorkspace { tool });
        }
    }
    Ok(())
}

fn present_rooms(e: &engine::Engine) -> Vec<&'static str> {
    agents::ROOMS
        .iter()
        .copied()
        .filter(|r| e.has_room(r))
        .collect()
}

/// Go's `icm.Scaffold`.
pub(crate) fn init(root: &Path, opts: &InitOptions, env: &Env) -> Result<InitReport, Error> {
    check_legacy(root)?;
    settings_vcs_ok(&opts.vcs)?;
    let e = engine::Engine {
        root,
        vcs: opts.vcs.clone(),
        version: opts.version.clone(),
    };
    let (mut written, git_init) = e.scaffold(opts.force, opts.no_git, env)?;
    let mut not_merged = Vec::new();
    match agents::write_agents(root, &present_rooms(&e), opts.force)? {
        AgentsResult::Written => written.push("AGENTS.md".into()),
        AgentsResult::Unmergeable(reason) => not_merged.push(("AGENTS.md".into(), reason)),
        AgentsResult::Merged | AgentsResult::Unchanged => {}
    }
    Ok(InitReport {
        written,
        not_merged,
        git_init,
    })
}

fn settings_vcs_ok(vcs: &str) -> Result<(), Error> {
    if vcs.is_empty() || vcs == "git" || vcs == "none" {
        return Ok(());
    }
    Err(Error::InvalidVcs {
        shown: crate::gofmt::quote(vcs),
    })
}

/// Go's `icm.Update`.
pub(crate) fn update(root: &Path, dirs: &Dirs, version: &str) -> Result<UpdateReport, Error> {
    check_legacy(root)?;
    let loaded = settings::load(root, dirs)?;
    let e = engine::Engine {
        root,
        vcs: loaded.vcs,
        version: version.to_string(),
    };
    let (updated, skipped, unchanged) = e.update(&shown_root(root))?;
    let mut rep = UpdateReport {
        updated,
        skipped,
        unchanged,
        ..UpdateReport::default()
    };
    match agents::write_agents(root, &present_rooms(&e), false)? {
        AgentsResult::Written => rep.updated.push("AGENTS.md".into()),
        AgentsResult::Merged => rep.merged.push("AGENTS.md".into()),
        AgentsResult::Unchanged => rep.unchanged.push("AGENTS.md".into()),
        AgentsResult::Unmergeable(reason) => {
            rep.skipped.push("AGENTS.md".into());
            rep.not_merged.push(("AGENTS.md".into(), reason));
        }
    }
    for list in [&mut rep.updated, &mut rep.skipped, &mut rep.unchanged] {
        list.sort();
    }
    Ok(rep)
}

/// Go's `EnsurePlan` / `EnsureBuild`.
pub(crate) fn ensure_room(
    root: &Path,
    dirs: &Dirs,
    room: &str,
    version: &str,
) -> Result<RoomReport, Error> {
    let loaded = settings::load(root, dirs)?;
    let e = engine::Engine {
        root,
        vcs: loaded.vcs,
        version: version.to_string(),
    };
    let Some(written) = e.ensure_room(room, &shown_root(root))? else {
        return Ok(RoomReport::default());
    };
    agents::write_agents(root, &present_rooms(&e), false)?;
    Ok(RoomReport {
        created: true,
        written,
    })
}

fn shown_root(root: &Path) -> String {
    crate::gofmt::to_slash(&root.to_string_lossy())
}

/// Whether `text` holds `{{`, other than the `{{links}}` placeholder that `daiku pr`
/// fills in a PR template.
fn has_template_syntax(text: &str) -> bool {
    text.replace("{{links}}", "").contains("{{")
}

/// Go's `icm.Check`: template syntax in any scaffold file, then the term lint.
pub fn check() -> Vec<String> {
    let mut problems = Vec::new();
    for (t, prefix) in [("root", ""), ("build", "build"), ("plan", "plan")] {
        for (rel, data) in engine::walk_tree(t, prefix, false) {
            if has_template_syntax(&String::from_utf8_lossy(data)) {
                problems.push(format!("{rel}: contains template syntax"));
            }
        }
    }
    problems.extend(lint::lint_scaffold());
    problems
}

#[cfg(test)]
mod tests;
