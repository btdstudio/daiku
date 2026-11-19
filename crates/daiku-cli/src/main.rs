//! The `daiku` command: parse arguments, call daiku-core, print. Core errors are
//! printed as their `Display` and nothing else (R12).

mod env;
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
    match command.as_str() {
        "version" | "--version" | "-v" => {
            let _ = writeln!(stdout, "daiku {}", env!("DAIKU_VERSION"));
            0
        }
        "settings" => settings(&mut stdout, &mut stderr),
        "init" => init(&args[1..], &mut stdout, &mut stderr),
        "update" => update(&mut stdout, &mut stderr),
        "check" => check(&mut stdout, &mut stderr),
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
