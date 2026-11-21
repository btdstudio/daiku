//! The `daiku` command: parse arguments, call daiku-core, print. Core errors are
//! printed as their `Display` and nothing else (R12).

mod build;
mod env;
mod json;
mod room;
mod tickets;
mod usage;

use std::io::Write;
use std::process::ExitCode;

use daiku_core::scaffold::InitOptions;
use daiku_core::{Error, Outcome, Workspace};

/// Every command Go dispatches. Those without a handler here aren't ported yet.
const KNOWN: &[&str] = &[
    "init",
    "update",
    "check",
    "status",
    "settings",
    "switch",
    "source",
    "adr",
    "build",
    "change",
    "gate",
    "reopen",
    "tickets",
    "overview",
    "idea",
    "references",
    "repo",
    "env",
    "commit",
    "pr",
];

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ExitCode::from(run(&args))
}

fn run(args: &[String]) -> u8 {
    let (mut stdout, mut stderr) = (std::io::stdout().lock(), std::io::stderr().lock());
    if let Some(code) = usage::help(args, &mut stdout, &mut stderr) {
        return code;
    }
    let Some(command) = args.first() else {
        let _ = stderr.write_all(usage::usage().as_bytes());
        return 1;
    };
    if let Some(code) = json::run(command, &args[1..], &mut stdout) {
        return code;
    }
    match command.as_str() {
        "version" | "--version" | "-v" => {
            let _ = writeln!(stdout, "daiku {}", env!("DAIKU_VERSION"));
            0
        }
        "settings" => settings(&mut stdout, &mut stderr),
        "init" => init(&args[1..], &mut stdout, &mut stderr),
        "update" => update(&mut stdout, &mut stderr),
        "check" => check(&mut stdout, &mut stderr),
        "source" => room::source(&args[1..], &mut stdout, &mut stderr),
        "adr" => room::adr(&args[1..], &mut stdout, &mut stderr),
        "idea" => room::idea(&args[1..], &mut stdout, &mut stderr),
        "overview" => room::overview(&args[1..], &mut stdout, &mut stderr),
        "references" => room::references(&args[1..], &mut stdout, &mut stderr),
        "change" => change(&args[1..], &mut stdout, &mut stderr),
        "gate" => gate(&args[1..], &mut stdout, &mut stderr),
        "reopen" => reopen(&args[1..], &mut stdout, &mut stderr),
        "switch" => switch(&args[1..], &mut stdout, &mut stderr),
        "status" => status(&args[1..], &mut stdout, &mut stderr),
        "tickets" => tickets::tickets(&args[1..], &mut stdout, &mut stderr),
        "repo" => build::repo(&args[1..], &mut stdout, &mut stderr),
        "env" => build::env(&mut stdout, &mut stderr),
        "build" => build::build(&args[1..], &mut stdout, &mut stderr),
        "commit" => build::commit(&args[1..], &mut stderr),
        "pr" => build::pr(&args[1..], &mut stderr),
        c if KNOWN.contains(&c) => {
            let _ = writeln!(stderr, "daiku: {c} isn't implemented yet");
            1
        }
        c => {
            let _ = write!(
                stderr,
                "daiku: unknown command {}\n{}",
                daiku_core::gofmt::quote(c),
                usage::usage()
            );
            1
        }
    }
}

fn workspace() -> Workspace {
    Workspace::open(".", env::real())
}

/// Prints an operation's warnings after its output, or its error, and returns the
/// exit code.
fn finish<T>(
    result: Result<Outcome<T>, Error>,
    stderr: &mut impl Write,
    print: impl FnOnce(&T),
) -> u8 {
    match result {
        Ok(outcome) => {
            print(&outcome.value);
            for w in &outcome.warnings {
                let _ = writeln!(stderr, "warning: {w}");
            }
            0
        }
        Err(err) => {
            let _ = writeln!(stderr, "{}", report(&err));
            1
        }
    }
}

/// What the CLI prints for a core error: its `Display`, with nothing added (AC7).
fn report(err: &Error) -> String {
    err.to_string()
}

fn settings(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    finish(workspace().settings(), stderr, |loaded| {
        let e = &loaded.effective;
        let _ = write!(
            stdout,
            "baseSource:  {}\ngates:       {}\nsourceDir:   {}\nadrDir:      {}\nsyncFile:    {}\noverview:    {}\n",
            e.base_source, e.gates, e.source_dir, e.adr_dir, e.sync_file, e.overview
        );
    })
}

/// The version written into manifests, as `daiku version` prints it.
const VERSION: &str = env!("DAIKU_VERSION");

/// Go's `runInit`.
fn init(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (mut dir, mut no_git, mut force) = (".".to_string(), false, false);
    for a in args {
        match a.as_str() {
            "--no-git" => no_git = true,
            "--force" => force = true,
            f if f.starts_with('-') => {
                let _ = writeln!(stderr, "daiku init: unknown flag {f}");
                return 1;
            }
            d => dir = d.to_string(),
        }
    }
    let env = env::real();
    let vcs = match daiku_core::scaffold::init_vcs(no_git, &env.dirs) {
        Ok(v) => v,
        Err(err) => {
            let _ = writeln!(stderr, "{}", report(&err));
            return 1;
        }
    };
    let opts = InitOptions {
        no_git,
        force,
        vcs,
        version: VERSION.to_string(),
    };
    finish(Workspace::open(&dir, env).init(&opts), stderr, |rep| {
        for rel in &rep.written {
            let _ = writeln!(stdout, "wrote {rel}");
        }
        for (rel, reason) in &rep.not_merged {
            let _ = writeln!(stdout, "skipped (edited) {rel}: can't merge: {reason}");
        }
        if rep.git_init {
            let _ = writeln!(stdout, "git init");
        }
        let _ = writeln!(stdout, "scaffolded daiku workspace in {dir}");
    })
}

/// Go's `runUpdate`. Extra arguments are ignored, as in Go.
fn update(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    finish(workspace().update(VERSION), stderr, |rep| {
        for rel in &rep.updated {
            let _ = writeln!(stdout, "updated {rel}");
        }
        for rel in &rep.merged {
            let _ = writeln!(
                stdout,
                "merged {rel} (daiku's ## sections updated; your sections and the text under the title kept)"
            );
        }
        for rel in &rep.skipped {
            match rep.not_merged.iter().find(|(r, _)| r == rel) {
                Some((_, reason)) => {
                    let _ = writeln!(stdout, "skipped (edited) {rel}: can't merge: {reason}");
                }
                None => {
                    let _ = writeln!(stdout, "skipped (edited) {rel}");
                }
            }
        }
        let _ = writeln!(
            stdout,
            "update complete ({} updated, {} skipped, {} unchanged, {} merged)",
            rep.updated.len(),
            rep.skipped.len(),
            rep.unchanged.len(),
            rep.merged.len()
        );
    })
}

/// Go's `runCheck`. Arguments are ignored, as in Go.
fn check(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let problems = daiku_core::scaffold::check();
    for p in &problems {
        let _ = writeln!(stderr, "{p}");
    }
    if !problems.is_empty() {
        return 1;
    }
    let _ = writeln!(stdout, "scaffold is clean");
    0
}

/// Go's `parseTransition`: positional arguments, `--reason <r>` and `--force`.
/// An unknown flag is an error.
fn parse_transition(args: &[String]) -> Option<(Vec<&str>, String, bool)> {
    let (mut pos, mut reason, mut force) = (Vec::new(), String::new(), false);
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--force" {
            force = true;
        } else if a == "--reason" && i + 1 < args.len() {
            i += 1;
            reason = args[i].clone();
        } else if a.starts_with('-') {
            return None;
        } else {
            pos.push(a);
        }
        i += 1;
    }
    Some((pos, reason, force))
}

/// Prints a transition's result: `<change>: <phase> = <status>`.
fn transition(c: &daiku_core::state::Change, stdout: &mut impl Write) {
    let _ = writeln!(stdout, "{}: {} = {}", c.name, c.phase, c.status);
}

/// Go's `runGate`.
fn gate(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some((pos, reason, force)) = parse_transition(args).filter(|(p, _, _)| p.len() == 2) else {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["gate"]));
        return 1;
    };
    finish(
        workspace().gate(pos[0], pos[1], &reason, force),
        stderr,
        |c| transition(c, stdout),
    )
}

/// Go's `runReopen`.
fn reopen(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some((pos, reason, force)) = parse_transition(args).filter(|(p, _, _)| p.len() == 1) else {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["reopen"]));
        return 1;
    };
    finish(workspace().reopen(pos[0], &reason, force), stderr, |c| {
        transition(c, stdout)
    })
}

/// Go's `runChange`: `new` or `close`.
fn change(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    match args.first().map(String::as_str) {
        Some("close") => return change_close(&args[1..], stdout, stderr),
        Some("new") => {}
        _ => {
            let _ = write!(stderr, "{}", usage::usage_text(&["change"]));
            return 1;
        }
    }
    let mut opts = daiku_core::changes::ChangeNewOptions {
        version: VERSION.to_string(),
        ..Default::default()
    };
    let args = &args[1..];
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let value = args.get(i + 1);
        match (a, value) {
            ("--repo", Some(v)) => {
                opts.repos.push(v.clone());
                i += 1;
            }
            ("--rev", Some(v)) => {
                opts.rev = v.clone();
                i += 1;
            }
            ("--body", Some(v)) => {
                opts.body = v.as_str().into();
                i += 1;
            }
            _ if a.starts_with('-') => {
                let _ = writeln!(
                    stderr,
                    "daiku change new: unknown flag {}",
                    daiku_core::gofmt::quote(a)
                );
                return 1;
            }
            _ if !opts.title.is_empty() => {
                let _ = writeln!(
                    stderr,
                    "daiku change new: unexpected argument {} (quote a multi-word title)",
                    daiku_core::gofmt::quote(a)
                );
                return 1;
            }
            _ => opts.title = a.to_string(),
        }
        i += 1;
    }
    if opts.title.is_empty() {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["change", "new"]));
        return 1;
    }
    let res = workspace().change_new(&opts);
    if res.room_created {
        let _ = writeln!(stdout, "created room plan/");
    }
    finish(res.result, stderr, |n| print_created_change(stdout, n))
}

/// Go's `printCreatedChange`: the change and its intake, then the phases it
/// skipped. `change new` and `idea promote` print it.
fn print_created_change(stdout: &mut impl Write, n: &daiku_core::changes::NewChange) {
    let _ = writeln!(
        stdout,
        "created change {} (source #{})",
        n.change.name, n.source_id
    );
    for phase in ["explore", "synthesis"] {
        if n.change.gate(phase) == "skipped" {
            let _ = writeln!(stdout, "{phase} skipped");
        }
    }
}

/// Go's `runChangeClose`.
fn change_close(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some((pos, reason, _)) = parse_transition(args).filter(|(p, _, _)| p.len() == 1) else {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["change", "close"]));
        return 1;
    };
    finish(workspace().change_close(pos[0], &reason), stderr, |c| {
        let _ = writeln!(stdout, "{}: closed", c.name);
    })
}

/// Go's `runSwitch`.
fn switch(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let all = args.first().is_some_and(|a| a == "--all");
    let args = if all { &args[1..] } else { args };
    let Some(name) = args.first() else {
        return finish(workspace().known_changes(), stderr, |changes| {
            if changes.is_empty() {
                let _ = writeln!(
                    stdout,
                    "no changes yet — run `daiku change new <title>` first"
                );
                return;
            }
            let show_loop = changes.iter().any(|c| c.loop_ == "build");
            for c in changes {
                if c.status == "closed" && !all {
                    continue;
                }
                if show_loop {
                    let _ = writeln!(stdout, "{}  {}  {} {}", c.name, c.loop_, c.phase, c.status);
                } else {
                    let _ = writeln!(stdout, "{}  {} {}", c.name, c.phase, c.status);
                }
            }
        });
    };
    finish(workspace().switch(name), stderr, |c| {
        let _ = writeln!(stdout, "switched to {}", c.name);
    })
}

/// Go's `runStatus` and, with a plan's name, `runInitiative`.
fn status(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    match args {
        [plan] if !plan.starts_with('-') => {
            return finish(workspace().status_plan(plan), stderr, |p| {
                let _ = writeln!(
                    stdout,
                    "{}  {} {}",
                    p.plan.name, p.plan.phase, p.plan.status
                );
                if p.rows.is_empty() {
                    let _ = writeln!(stdout, "  no tickets yet");
                }
                for r in &p.rows {
                    let repo = if r.repo.is_empty() { "-" } else { &r.repo };
                    let _ = writeln!(stdout, "  {}  {repo}  {}", r.id, r.build());
                }
            });
        }
        [] => {}
        _ => {
            let _ = writeln!(stderr, "{}", usage::usage_line(&["status"]));
            return 1;
        }
    }
    finish(workspace().status(), stderr, |s| {
        let mut out = String::new();
        let active_name = s
            .active
            .as_ref()
            .map(|a| a.change.name.clone())
            .unwrap_or_default();
        match &s.active {
            None => out.push_str("no active change\n"),
            Some(a) => {
                let c = &a.change;
                out.push_str(&format!("change:   {}\nroom:     {}\n", c.name, c.loop_));
                if !c.repos.is_empty() {
                    out.push_str(&format!("repo:     {}\n", c.repos.join(", ")));
                }
                out.push_str(&format!("phase:    {}\nstatus:   {}\n", c.phase, c.status));
                let gates: Vec<String> = a
                    .phases
                    .iter()
                    .map(|p| format!("{p}={}", c.gate(p)))
                    .collect();
                out.push_str(&format!("gates:    {}\n", gates.join(" ")));
                out.push_str(&format!("initiative: {}\n", a.initiative));
                if !a.code.is_empty() {
                    let code: Vec<String> = a
                        .code
                        .iter()
                        .map(|(n, p)| format!("{n}={}", p.as_deref().unwrap_or("(not set)")))
                        .collect();
                    out.push_str(&format!("code:     {}\n", code.join(" ")));
                }
                if let Some((id, title, branch)) = &a.ticket {
                    out.push_str(&format!("ticket:   {id} {title}\nbranch:   {branch}\n"));
                }
            }
        }
        out.push_str(&format!("vcs:      {}\n\n", s.vcs));
        let mark = |name: &str| if name == active_name { "* " } else { "  " };
        if s.no_changes {
            out.push_str("no changes yet\n");
        } else {
            if !s.plans.is_empty() {
                out.push_str("plans:\n");
                for c in &s.plans {
                    out.push_str(&format!(
                        "{}{}  {} {}\n",
                        mark(&c.name),
                        c.name,
                        c.phase,
                        c.status
                    ));
                }
            }
            if !s.builds.is_empty() {
                if !s.plans.is_empty() {
                    out.push('\n');
                }
                out.push_str("builds:\n");
                for r in &s.builds {
                    match &r.holder {
                        Some(h) => out.push_str(&format!("  {}  held by {h}\n", r.repo)),
                        None => out.push_str(&format!("  {}  free\n", r.repo)),
                    }
                    for c in &r.builds {
                        let done = if c.phase == "ship" && c.status == "complete" {
                            " (done)"
                        } else {
                            ""
                        };
                        out.push_str(&format!(
                            "  {}{}  {} {}{done}\n",
                            mark(&c.name),
                            c.name,
                            c.phase,
                            c.status
                        ));
                    }
                }
            }
            if s.plans.is_empty() && s.builds.is_empty() {
                out.push_str("no open changes\n");
            }
        }
        let _ = stdout.write_all(out.as_bytes());
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC7: the CLI's error text is exactly the core error's `Display`.
    #[test]
    fn errors_print_their_display() {
        let err = Error::InvalidGates {
            value: "sometimes".into(),
        };
        assert_eq!(
            report(&err),
            "daiku: invalid gates \"sometimes\" (expected block|prompt|auto)"
        );
        let mut out = Vec::new();
        let code = finish::<()>(Err(err), &mut out, |_| {});
        assert_eq!(code, 1);
        assert_eq!(
            out,
            b"daiku: invalid gates \"sometimes\" (expected block|prompt|auto)\n"
        );
    }

    #[test]
    fn warnings_follow_the_output() {
        let mut err = Vec::new();
        let mut printed = false;
        let outcome = Outcome {
            value: (),
            warnings: vec![daiku_core::Warning {
                message: "careful".into(),
            }],
        };
        assert_eq!(finish(Ok(outcome), &mut err, |_| printed = true), 0);
        assert!(printed);
        assert_eq!(err, b"warning: careful\n");
    }
}
