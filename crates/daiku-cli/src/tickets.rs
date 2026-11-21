//! `daiku tickets`: Go's `runTickets` and its verbs' argument parsing, usage
//! errors and output. The work is in daiku-core's `tickets` module (S-0007).

use std::io::Write;

use daiku_core::Outcome;
use daiku_core::gofmt::quote;
use daiku_core::tickets::{PublishResult, ReadyResult, ReadyTargets, TicketNewOptions};

use crate::{finish, parse_transition, report, usage, workspace};

/// Go's `runTickets`.
pub fn tickets(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let rest = args.get(1..).unwrap_or_default();
    match args.first().map(String::as_str) {
        Some("new") => new(rest, stdout, stderr),
        Some("check") => check(stdout, stderr),
        Some("status") => status(stdout, stderr),
        Some("order") => order(stdout, stderr),
        Some("ready") => ready(rest, stdout, stderr),
        Some("mark-published") => mark_published(rest, stdout, stderr),
        Some("unfreeze") => unfreeze(rest, stdout, stderr),
        Some("pull") => pull(rest, stdout, stderr),
        _ => {
            let _ = write!(stderr, "{}", usage::usage_text(&["tickets"]));
            1
        }
    }
}

fn fail(stderr: &mut impl Write, message: &str) -> u8 {
    let _ = writeln!(stderr, "{message}");
    1
}

/// Prints the warnings an outcome carries, after its output.
fn warn<T>(outcome: &Outcome<T>, stderr: &mut impl Write) {
    for w in &outcome.warnings {
        let _ = writeln!(stderr, "warning: {w}");
    }
}

/// Go's `splitIDs`: trimmed, without empty entries or repeats.
fn split_ids(list: &str) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for id in list.split(',').map(str::trim) {
        if !id.is_empty() && !ids.iter().any(|i| i == id) {
            ids.push(id.to_string());
        }
    }
    ids
}

/// Go's `ticketsNew`.
fn new(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let mut o = TicketNewOptions::default();
    let mut pos: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        let has_value = i + 1 < args.len();
        match a {
            "--inbox" => o.inbox = true,
            "--parent" if has_value => {
                i += 1;
                o.parent = args[i].clone();
            }
            "--repo" if has_value => {
                i += 1;
                o.repo = args[i].clone();
            }
            "--depends" if has_value => {
                i += 1;
                o.depends_on = args[i].split(',').map(str::to_string).collect();
            }
            "--requirements" if has_value => {
                i += 1;
                o.requirements = split_ids(&args[i]);
            }
            _ if a.starts_with('-') => {
                return fail(
                    stderr,
                    &format!("daiku tickets new: unknown flag {}", quote(a)),
                );
            }
            _ => pos.push(a),
        }
        i += 1;
    }
    if pos.len() != 2 {
        return fail(
            stderr,
            &format!(
                "{} (quote a multi-word title)",
                usage::usage_line(&["tickets", "new"])
            ),
        );
    }
    o.typ = pos[0].to_string();
    o.title = pos[1].to_string();
    finish(workspace().tickets_new(&o), stderr, |c| {
        let _ = writeln!(stdout, "created {} {}", c.id, c.path);
    })
}

/// Go's `ticketsCheck`.
fn check(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let report_ = match workspace().tickets_check() {
        Ok(o) => o.value,
        Err(e) => return fail(stderr, &report(&e)),
    };
    for p in &report_.errors {
        let _ = writeln!(stdout, "✗ {p}");
    }
    for w in &report_.warnings {
        let _ = writeln!(stdout, "! {w}");
    }
    if !report_.errors.is_empty() {
        return 1;
    }
    if report_.warnings.is_empty() {
        let _ = writeln!(stdout, "{} tickets ok", report_.count);
    } else {
        let _ = writeln!(
            stdout,
            "{} tickets ok, {} warning(s)",
            report_.count,
            report_.warnings.len()
        );
    }
    0
}

/// Go's `ticketsStatus`.
fn status(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    match workspace().tickets_status() {
        Err(e) => fail(stderr, &report(&e)),
        Ok(o) => {
            for b in &o.value.buckets {
                let _ = writeln!(
                    stdout,
                    "{:<10} {}  {}",
                    b.name,
                    b.ids.len(),
                    b.ids.join(" ")
                );
            }
            for p in &o.value.unreadable {
                let _ = writeln!(stderr, "✗ {p}");
            }
            warn(&o, stderr);
            0
        }
    }
}

/// Go's `ticketsOrder`.
fn order(stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    finish(workspace().tickets_order(), stderr, |list| {
        for t in list {
            let _ = writeln!(stdout, "{}  {}", t.id, t.title);
        }
    })
}

fn refused(stdout: &mut impl Write, problems: &[daiku_core::ticket::Problem]) {
    for p in problems {
        let _ = writeln!(stdout, "✗ {p}");
    }
}

/// Go's `ticketsReady`.
fn ready(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (mut ids, mut all, mut force) = (Vec::new(), false, false);
    for a in args {
        match a.as_str() {
            "--all" => all = true,
            "--force" => force = true,
            a if a.starts_with('-') => {
                return fail(
                    stderr,
                    &format!("daiku tickets ready: unknown flag {}", quote(a)),
                );
            }
            a => ids.push(a.to_string()),
        }
    }
    if all == !ids.is_empty() {
        return fail(stderr, &usage::usage_line(&["tickets", "ready"]));
    }
    let targets = if all {
        ReadyTargets::All
    } else {
        ReadyTargets::Ids(ids)
    };
    match workspace().tickets_ready(&targets, force) {
        Err(e) => fail(stderr, &report(&e)),
        Ok(o) => {
            // Go prints a policy warning as it checks, before the result.
            warn(&o, stderr);
            match &o.value {
                ReadyResult::Readied(ids) => {
                    for id in ids {
                        let _ = writeln!(stdout, "{id}: ready");
                    }
                    0
                }
                ReadyResult::NoDrafts(change) => {
                    let _ = writeln!(stdout, "no draft tickets in {change}");
                    0
                }
                ReadyResult::Refused(problems) => {
                    refused(stdout, problems);
                    1
                }
            }
        }
    }
}

/// Go's `ticketsMarkPublished`.
fn mark_published(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let usage_line = usage::usage_line(&["tickets", "mark-published"]);
    let (mut id, mut external, mut force) = (String::new(), String::new(), false);
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if a == "--external" && i + 1 < args.len() {
            i += 1;
            external = args[i].clone();
        } else if a == "--force" {
            force = true;
        } else if a.starts_with('-') || !id.is_empty() {
            return fail(stderr, &usage_line);
        } else {
            id = a.to_string();
        }
        i += 1;
    }
    let Some((tool, item)) = external.split_once('=').filter(|_| !id.is_empty()) else {
        return fail(stderr, &usage_line);
    };
    match workspace().tickets_mark_published(&id, tool, item, force) {
        Err(e) => fail(stderr, &report(&e)),
        Ok(o) => {
            warn(&o, stderr);
            match &o.value {
                PublishResult::Published(p) => {
                    let _ = writeln!(stdout, "{}: published to {} ({})", p.id, p.tool, p.item);
                    0
                }
                PublishResult::Refused(problems) => {
                    refused(stdout, problems);
                    1
                }
            }
        }
    }
}

/// Go's `ticketsUnfreeze`.
fn unfreeze(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some((pos, reason, _)) =
        parse_transition(args).filter(|(p, r, _)| p.len() == 1 && !r.is_empty())
    else {
        return fail(stderr, &usage::usage_line(&["tickets", "unfreeze"]));
    };
    finish(
        workspace().tickets_unfreeze(pos[0], &reason),
        stderr,
        |id| {
            let _ = writeln!(stdout, "{id}: unfrozen");
        },
    )
}

/// Go's `ticketsPull`.
fn pull(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args.len() != 1 || args[0].starts_with('-') {
        return fail(stderr, &usage::usage_line(&["tickets", "pull"]));
    }
    finish(workspace().tickets_pull(&args[0]), stderr, |p| {
        let _ = writeln!(stdout, "pulled {} {}", p.id, p.rel);
    })
}
