//! `daiku repo`, `env`, `build`, `commit` and `pr`: Go's `repos.Run` and `RunEnv`,
//! `runBuild`, `runCommit` and `runPR`. Each ports its handler's argument parsing,
//! usage errors and output; the work is a `Workspace` method in daiku-core
//! (`build.rs`) (S-0008).

use std::io::Write;

use daiku_core::build::{CloneStep, PrOptions};

use crate::display::field;
use crate::{VERSION, finish, report, usage, workspace};

fn fail(stderr: &mut impl Write, message: &str) -> u8 {
    let _ = writeln!(stderr, "{message}");
    1
}

/// Go's `repos.Run`: the usage block, to stderr, exit 1.
fn repo_usage(stderr: &mut impl Write) -> u8 {
    let _ = write!(stderr, "{}", usage::usage_text(&["repo"]));
    1
}

/// Go's `repos.Run` for `daiku repo`. The registry is loaded before the verb is
/// read, so a corrupt `REPOS.json` or `paths.json` is reported first.
pub fn repo(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some(verb) = args.first() else {
        return repo_usage(stderr);
    };
    let ws = workspace();
    if let Err(e) = ws.registry() {
        return fail(stderr, &report(&e));
    }
    let rest = &args[1..];
    match verb.as_str() {
        "add" => repo_add(rest, stdout, stderr),
        "list" => finish(ws.repo_list(), stderr, |rows| {
            for r in rows {
                // Free text from REPOS.json and paths.json (ADR-0008, Field).
                let path = r.path.as_deref().map_or("(no local path)".into(), field);
                let archived = match (r.archived, r.archived_reason.as_str()) {
                    (false, _) => String::new(),
                    (true, "") => "  (archived)".into(),
                    (true, why) => format!("  (archived: {})", field(why)),
                };
                let _ = writeln!(
                    stdout,
                    "{}  {}  {}  {path}{archived}",
                    r.name,
                    field(&r.remote),
                    field(&r.branch)
                );
            }
        }),
        "path" => {
            if rest.len() != 2 {
                return repo_usage(stderr);
            }
            finish(ws.repo_path(&rest[0], &rest[1]), stderr, |stored| {
                let _ = writeln!(stdout, "{}: {stored}", rest[0]);
            })
        }
        "doctor" => repo_doctor(rest, stdout, stderr),
        "archive" => repo_archive(rest, stdout, stderr),
        "unarchive" => {
            if rest.len() != 1 || rest[0].starts_with("--") {
                return repo_usage(stderr);
            }
            finish(ws.repo_unarchive(&rest[0]), stderr, |name| {
                let _ = writeln!(stdout, "{name}: unarchived");
            })
        }
        _ => repo_usage(stderr),
    }
}

fn repo_add(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (mut pos, mut path, mut branch) = (Vec::new(), None, String::new());
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--path" && i + 1 < args.len() {
            i += 1;
            path = Some(args[i].as_str());
        } else if a == "--branch" && i + 1 < args.len() {
            i += 1;
            branch = args[i].clone();
        } else if a.starts_with("--") {
            return repo_usage(stderr);
        } else {
            pos.push(a);
        }
        i += 1;
    }
    if pos.len() != 2 {
        return repo_usage(stderr);
    }
    finish(
        workspace().repo_add(pos[0], pos[1], path, &branch),
        stderr,
        |name| {
            let _ = writeln!(stdout, "added {name} — commit REPOS.json to share it");
        },
    )
}

/// `daiku repo archive <name> [--reason <r>]` (N-0008).
fn repo_archive(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (mut pos, mut reason) = (Vec::new(), String::new());
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--reason" && i + 1 < args.len() {
            i += 1;
            reason = args[i].clone();
        } else if a.starts_with("--") {
            return repo_usage(stderr);
        } else {
            pos.push(a);
        }
        i += 1;
    }
    if pos.len() != 1 {
        return repo_usage(stderr);
    }
    finish(workspace().repo_archive(pos[0], &reason), stderr, |name| {
        let _ = writeln!(stdout, "{name}: archived");
    })
}

/// Go's `runDoctor`.
fn repo_doctor(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (mut clone, mut into) = (false, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--clone" => clone = true,
            "--into" if i + 1 < args.len() => {
                i += 1;
                into = Some(args[i].as_str());
            }
            _ => return repo_usage(stderr),
        }
        i += 1;
    }
    let outcome = match workspace().repo_doctor(clone, into) {
        Ok(o) => o,
        Err(e) => return fail(stderr, &report(&e)),
    };
    let r = &outcome.value;
    for step in &r.clones {
        match step {
            CloneStep::NoPath { repo } => {
                let _ = writeln!(
                    stderr,
                    "{repo}: no local path — pass --into <dir> or run `daiku repo path {repo} <path>`"
                );
            }
            CloneStep::Failed { error, .. } | CloneStep::SaveFailed { error } => {
                let _ = writeln!(stderr, "{error}");
            }
            CloneStep::Cloned { repo, path } => {
                let _ = writeln!(stdout, "cloned {repo} to {}", field(path));
            }
        }
    }
    if r.aborted {
        return 1;
    }
    for p in &r.problems {
        let _ = writeln!(stdout, "✗ {}: {}", p.repo, p.detail);
    }
    if !r.problems.is_empty() {
        return 1;
    }
    let _ = writeln!(stdout, "{} repos ok", r.count);
    0
}

/// Go's `runEnv`. Extra arguments are ignored, as in Go.
pub fn env(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    finish(workspace().env_flags(), stderr, |flags| {
        if !flags.is_empty() {
            let _ = writeln!(stdout, "{flags}");
        }
    })
}

/// Go's `runBuild` and `buildNew`.
pub fn build(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args.first().map(String::as_str) != Some("new") {
        let _ = write!(stderr, "{}", usage::usage_text(&["build"]));
        return 1;
    }
    let rest = &args[1..];
    if rest.len() != 1 || rest[0].starts_with('-') {
        return fail(stderr, &usage::usage_line(&["build", "new"]));
    }
    let r = workspace().build_new(&rest[0], VERSION);
    if r.room_created {
        let _ = writeln!(stdout, "created room build/");
    }
    finish(r.result, stderr, |b| {
        let _ = writeln!(stdout, "created build {} for {} ({})", b.id, b.repo, b.rel);
    })
}

/// Go's `runCommit`: the arguments go to `git commit` as they are.
pub fn commit(args: &[String], stderr: &mut impl Write) -> u8 {
    // git takes the terminal next: nothing of ours may still be held back.
    let _ = stderr.flush();
    finish(workspace().commit(args), stderr, |()| {})
}

/// What `daiku pr`'s arguments ask for.
enum PrArgs {
    /// `--template`: print the chosen template.
    Template,
    /// Open a pull request, with a title and a finished body file, either optional.
    Open {
        title: Option<String>,
        body_file: Option<String>,
    },
}

/// Parses `pr`'s arguments: `--template`, `--body-file <path>` (or
/// `--body-file=<path>`) and at most one title. `Err` is a usage error.
fn parse_pr(args: &[String]) -> Result<PrArgs, ()> {
    let (mut template, mut title, mut body_file) = (false, None, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--template" {
            template = true;
        } else if a == "--body-file" {
            match it.next() {
                Some(p) if body_file.is_none() => body_file = Some(p.clone()),
                _ => return Err(()),
            }
        } else if let Some(p) = a.strip_prefix("--body-file=") {
            if body_file.is_some() {
                return Err(());
            }
            body_file = Some(p.to_string());
        } else if a.starts_with('-') || title.is_some() {
            return Err(());
        } else {
            title = Some(a.clone());
        }
    }
    if body_file.as_deref() == Some("") {
        return Err(());
    }
    if template {
        return if title.is_none() && body_file.is_none() {
            Ok(PrArgs::Template)
        } else {
            Err(())
        };
    }
    Ok(PrArgs::Open { title, body_file })
}

/// Go's `runPR`, with N-0019's `--template` and `--body-file`: the warning about
/// the branch reaches stderr before gh takes the terminal. `--template` prints the
/// body to stdout and the template's origin to stderr, and doesn't run gh.
pub fn pr(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Ok(parsed) = parse_pr(args) else {
        return fail(stderr, &usage::usage_line(&["pr"]));
    };
    let ws = workspace();
    let (title, body_file) = match parsed {
        PrArgs::Template => {
            return match ws.pr_template() {
                Ok(o) => {
                    for w in &o.warnings {
                        let _ = writeln!(stderr, "warning: {w}");
                    }
                    let t = o.value;
                    let _ = write!(stdout, "{}", t.body);
                    let _ = writeln!(stderr, "template: {} {}", t.origin.as_str(), field(&t.path));
                    0
                }
                Err(e) => fail(stderr, &report(&e)),
            };
        }
        PrArgs::Open { title, body_file } => (title, body_file),
    };
    let body = match body_file {
        None => None,
        Some(path) => match std::fs::read_to_string(&path) {
            Ok(b) => Some(b),
            Err(e) => return fail(stderr, &format!("daiku: cannot read {}: {e}", field(&path))),
        },
    };
    let outcome = match ws.pr_command(&PrOptions { title, body }) {
        Ok(o) => o,
        Err(e) => return fail(stderr, &report(&e)),
    };
    for w in &outcome.warnings {
        let _ = writeln!(stderr, "warning: {w}");
    }
    let _ = stderr.flush();
    match ws.run_ship(&outcome.value) {
        Ok(()) => 0,
        Err(e) => fail(stderr, &report(&e)),
    }
}
