//! The `daiku` command: parse arguments, call daiku-core, print. Core errors are
//! printed as their `Display` and nothing else (R12).

mod env;
mod usage;

use std::io::Write;
use std::process::ExitCode;

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
