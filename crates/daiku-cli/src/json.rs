//! `--json` on the read commands in spec R5 (ADR-0004).
//!
//! [`run`] is called before the text dispatch. It handles an invocation only when
//! it holds `--json` and, with `--json` removed, is one of the read forms below;
//! every other invocation returns `None` and goes through the text path with its
//! arguments untouched, so a stray `--json` on any other command behaves as it
//! does in Go.
//!
//! A read group named with no verb (`tickets`, `idea`, `repo`, `references`) is a read
//! form too: it writes a `Usage` envelope with the group's usage block. An unknown
//! verb isn't, because it can't be told apart from a command that writes.
//!
//! A read form writes one envelope to stdout and nothing to stderr. The warnings
//! the text form prints to stderr go in the envelope's `warnings`. Its arguments
//! are checked as the text form checks them: what the text form ignores, `--json`
//! ignores, and what the text form rejects comes back as a `Usage` error whose
//! message is the text form's stderr.

use std::io::Write;

use daiku_core::json::{Serialize, Success, failure, to_pretty, usage_failure};
use daiku_core::{Error, Outcome};

use crate::{usage, workspace};

/// Runs `daiku <command> <args>` when it's a `--json` read form, and returns its
/// exit code; `None` when it isn't one.
pub fn run(command: &str, args: &[String], stdout: &mut impl Write) -> Option<u8> {
    let (rest, json) = take_flag(args);
    if !json {
        return None;
    }
    let rest: Vec<&str> = rest.iter().map(String::as_str).collect();
    let out = stdout;
    Some(match command {
        "version" | "--version" | "-v" => ok(
            out,
            Outcome::new(daiku_core::json::Version {
                version: env!("DAIKU_VERSION").to_string(),
            }),
            0,
        ),
        "settings" => read(out, workspace().settings()),
        "artifacts" => match rest.as_slice() {
            [change] if !change.starts_with('-') => read(out, workspace().artifacts(change)),
            _ => usage_err(out, &usage::usage_line(&["artifacts"])),
        },
        "status" => match rest.as_slice() {
            [] => read(out, workspace().status()),
            [plan] if !plan.starts_with('-') => read(out, workspace().status_plan(plan)),
            _ => usage_err(out, &usage::usage_line(&["status"])),
        },
        // Only the listing; `switch <change>` changes state, so it keeps Go's path.
        "switch" => {
            let all = rest.first() == Some(&"--all");
            if rest.len() != usize::from(all) {
                return None;
            }
            read(out, workspace().switch_list(all))
        }
        "env" => read(out, workspace().env_view()),
        // Only the template; opening a pull request is interactive, so it has no
        // JSON form.
        "pr" => match rest.as_slice() {
            ["--template"] => read(out, workspace().pr_template()),
            _ => usage_err(out, &usage::usage_line(&["pr"])),
        },
        // Go prints `repo`'s usage before it loads the registry when there's no verb.
        "repo" if rest.is_empty() => usage_err(out, &usage::usage_text(&["repo"])),
        "repo" if rest.first() == Some(&"list") => {
            // Go loads the registry before it reads the verb.
            let ws = workspace();
            match ws.registry() {
                Ok(_) => read(out, ws.repo_list()),
                Err(e) => err(out, &e),
            }
        }
        "tickets" => match rest.first().copied() {
            Some("check") => match workspace().tickets_check() {
                Ok(o) => {
                    let code = u8::from(!o.value.errors.is_empty());
                    ok(out, o, code)
                }
                Err(e) => err(out, &e),
            },
            Some("status") => read(out, workspace().tickets_status()),
            Some("list") => match rest[1..] {
                [] => read(out, workspace().tickets_list()),
                _ => usage_err(out, &usage::usage_line(&["tickets", "list"])),
            },
            Some("order") => read(out, workspace().tickets_order()),
            None => usage_err(out, &usage::usage_text(&["tickets"])),
            _ => return None,
        },
        "references" => match rest.as_slice() {
            ["check"] => read(out, workspace().references_check()),
            _ => usage_err(out, &usage::usage_text(&["references"])),
        },
        "source" | "adr" => return plan_record(command, &rest, out),
        "idea" => match rest.as_slice() {
            ["list", ..] => read(out, workspace().idea_list()),
            ["show", id] => match id.parse::<i64>() {
                Ok(n) if n >= 1 => read(out, workspace().idea_summary(n)),
                _ => usage_err(out, &format!("invalid id: {id}")),
            },
            ["show", ..] => usage_err(out, &usage::usage_line(&["idea", "show"])),
            [] => usage_err(out, &usage::usage_text(&["idea"])),
            _ => return None,
        },
        _ => return None,
    })
}

/// `source` and `adr`, `list` (the default) and `show`. Go loads the settings
/// before it reads the verb, so a settings error comes first.
fn plan_record(command: &str, rest: &[&str], out: &mut impl Write) -> Option<u8> {
    let verb = rest.first().copied().unwrap_or("list");
    if verb != "list" && verb != "show" {
        return None;
    }
    let ws = workspace();
    if let Err(e) = ws.settings() {
        return Some(err(out, &e));
    }
    let source = command == "source";
    Some(match (verb, rest.get(1)) {
        ("list", _) if source => read(out, ws.source_list()),
        ("list", _) => read(out, ws.adr_list()),
        ("show", None) => usage_err(out, &usage::usage_line(&[command, "show"])),
        ("show", Some(arg)) => match arg.parse::<i64>() {
            Ok(n) if source => read(out, ws.source_show(n)),
            Ok(n) => read(out, ws.adr_show(n)),
            Err(_) if source => usage_err(out, &format!("invalid id: {arg}")),
            Err(_) => usage_err(out, &format!("invalid number: {arg}")),
        },
        _ => unreachable!("verb is list or show"),
    })
}

/// `args` without any `--json`, and whether there was one.
fn take_flag(args: &[String]) -> (Vec<String>, bool) {
    let rest: Vec<String> = args.iter().filter(|a| *a != "--json").cloned().collect();
    let json = rest.len() != args.len();
    (rest, json)
}

/// The envelope for a core read: its value as `data` and its warnings, or its
/// error.
fn read<T: Serialize>(out: &mut impl Write, result: Result<Outcome<T>, Error>) -> u8 {
    match result {
        Ok(o) => ok(out, o, 0),
        Err(e) => err(out, &e),
    }
}

fn ok<T: Serialize>(out: &mut impl Write, outcome: Outcome<T>, exit: u8) -> u8 {
    write(out, &Success::from_outcome(outcome));
    exit
}

fn err(out: &mut impl Write, e: &Error) -> u8 {
    write(out, &failure(e));
    1
}

fn usage_err(out: &mut impl Write, message: &str) -> u8 {
    write(out, &usage_failure(message));
    1
}

fn write(out: &mut impl Write, value: &impl Serialize) {
    // The envelope holds only strings, numbers, bools and string-keyed maps, which
    // always serialize.
    let _ = out.write_all(&to_pretty(value).expect("the envelope always serializes"));
}
