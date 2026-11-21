//! The plan room's commands: `source`, `adr`, `idea`, `references` and
//! `overview`. Each ports its Go handler's argument parsing, usage errors and
//! output; the work is a `Workspace` method in daiku-core (`room.rs`).

use std::io::Write;

use daiku_core::Workspace;
use daiku_core::gofmt::quote;
use daiku_core::plan::{adr, source};
use daiku_core::room::{IdeaPromoteOptions, IdeaVerb, SourceNewOptions};

use crate::{VERSION, finish, print_created_change, report, usage, workspace};

/// The workspace, once Go's handler has loaded its settings: `source` and `adr`
/// load them before looking at the verb, so a settings error comes first.
fn with_settings(stderr: &mut impl Write) -> Option<Workspace> {
    let ws = workspace();
    match ws.settings() {
        Ok(_) => Some(ws),
        Err(e) => {
            let _ = writeln!(stderr, "{}", report(&e));
            None
        }
    }
}

/// Writes one output line from its parts: text daiku formats, and `Text` read
/// from a user's file, whose bytes are printed as they are, as Go prints them
/// (N-0001). One write per line.
fn line(stdout: &mut impl Write, parts: &[&[u8]]) {
    let mut out = parts.concat();
    out.push(b'\n');
    let _ = stdout.write_all(&out);
}

/// Go's `runSource`.
pub fn source(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some(ws) = with_settings(stderr) else {
        return 1;
    };
    let verb = args.first().map_or("list", String::as_str);
    match verb {
        "list" => {
            let (rest, json_mode) = crate::json::take_flag(&args[1..]);
            if !rest.is_empty() {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["source", "list"]));
                return 1;
            }
            if json_mode {
                return match ws.source_list() {
                    Ok(o) => crate::json::ok(stdout, &o.value, 0),
                    Err(e) => crate::json::err(stdout, &e),
                };
            }
            finish(ws.source_list(), stderr, |sources| {
                for s in sources {
                    let label = if s.source == source::REPO {
                        [b"repo:", s.repo.as_bytes()].concat()
                    } else {
                        s.source.as_bytes().to_vec()
                    };
                    let id = format!("#{} [", s.id);
                    line(stdout, &[id.as_bytes(), &label, b"] ", s.title.as_bytes()]);
                }
            })
        }
        "show" => {
            let (rest, json_mode) = crate::json::take_flag(&args[1..]);
            if rest.len() != 1 {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["source", "show"]));
                return 1;
            }
            let Ok(id) = rest[0].parse::<i64>() else {
                let _ = writeln!(stderr, "invalid id: {}", rest[0]);
                return 1;
            };
            if json_mode {
                return match ws.source_show(id) {
                    Ok(o) => crate::json::ok(stdout, &o.value, 0),
                    Err(e) => crate::json::err(stdout, &e),
                };
            }
            finish(ws.source_show(id), stderr, |s| {
                let head = format!("#{} [{}] ", s.id, s.source);
                let mut parts: Vec<&[u8]> = vec![head.as_bytes(), s.title.as_bytes()];
                if s.source == source::REPO {
                    parts.extend([&b"\nrepo:     "[..], s.repo.as_bytes()]);
                    parts.extend([&b"\nrev:      "[..], s.rev.as_bytes()]);
                }
                parts.extend([&b"\n"[..], s.body.as_bytes()]);
                line(stdout, &parts);
            })
        }
        "new" => {
            let mut opts = SourceNewOptions::default();
            let mut i = 1;
            while i < args.len() {
                let a = args[i].as_str();
                let field = match a {
                    "--title" => Some(&mut opts.title),
                    "--repo" => Some(&mut opts.repo),
                    "--rev" => Some(&mut opts.rev),
                    "--body" => Some(&mut opts.body),
                    _ => None,
                };
                match (field, args.get(i + 1)) {
                    (Some(field), Some(v)) => {
                        *field = v.clone();
                        i += 1;
                    }
                    _ if a.starts_with('-') => {
                        let _ = writeln!(
                            stderr,
                            "daiku source new: unknown flag {}, or it needs a value (see daiku source new --help)",
                            quote(a)
                        );
                        return 1;
                    }
                    _ => {
                        let _ = writeln!(
                            stderr,
                            "daiku source new: unexpected argument {} (use --title <t>; see daiku source new --help)",
                            quote(a)
                        );
                        return 1;
                    }
                }
                i += 1;
            }
            if opts.title.is_empty() {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["source", "new"]));
                return 1;
            }
            finish(ws.source_new(&opts), stderr, |s| {
                let head = format!("created #{} [{}] ", s.id, s.source);
                line(stdout, &[head.as_bytes(), s.title.as_bytes()]);
            })
        }
        v => {
            let _ = write!(
                stderr,
                "daiku source: unknown verb {}\n{}",
                quote(v),
                usage::usage_text(&["source"])
            );
            1
        }
    }
}

/// Go's `runAdr`.
pub fn adr(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some(ws) = with_settings(stderr) else {
        return 1;
    };
    let verb = args.first().map_or("list", String::as_str);
    match verb {
        "list" => {
            let (rest, json_mode) = crate::json::take_flag(&args[1..]);
            if !rest.is_empty() {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["adr", "list"]));
                return 1;
            }
            if json_mode {
                return match ws.adr_list() {
                    Ok(o) => crate::json::ok(stdout, &o.value, 0),
                    Err(e) => crate::json::err(stdout, &e),
                };
            }
            finish(ws.adr_list(), stderr, |records| {
                for r in records {
                    let head = format!("{:04} [{}] ", r.number, r.status);
                    line(stdout, &[head.as_bytes(), r.title.as_bytes()]);
                }
            })
        }
        "show" => {
            let (rest, json_mode) = crate::json::take_flag(&args[1..]);
            if rest.len() != 1 {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["adr", "show"]));
                return 1;
            }
            let Ok(n) = rest[0].parse::<i64>() else {
                let _ = writeln!(stderr, "invalid number: {}", rest[0]);
                return 1;
            };
            if json_mode {
                return match ws.adr_show(n) {
                    Ok(o) => crate::json::ok(stdout, &o.value, 0),
                    Err(e) => crate::json::err(stdout, &e),
                };
            }
            finish(ws.adr_show(n), stderr, |r| {
                let _ = stdout.write_all(&r.raw);
            })
        }
        "new" => {
            let mut opts = adr::Options::default();
            let mut file = String::new();
            let mut i = 1;
            while i < args.len() {
                let a = args[i].as_str();
                let field = match a {
                    "--title" => Some(&mut opts.title),
                    "--deciders" => Some(&mut opts.deciders),
                    "--context" => Some(&mut opts.context),
                    "--decision" => Some(&mut opts.decision),
                    "--consequences" => Some(&mut opts.consequences),
                    "--file" => Some(&mut file),
                    _ => None,
                };
                match (field, args.get(i + 1)) {
                    (Some(field), Some(v)) => {
                        *field = v.clone();
                        i += 1;
                    }
                    _ if a.starts_with('-') => {
                        let _ = writeln!(
                            stderr,
                            "daiku adr new: unknown flag {}, or it needs a value (see daiku adr new --help)",
                            quote(a)
                        );
                        return 1;
                    }
                    _ => {
                        let _ = writeln!(
                            stderr,
                            "daiku adr new: unexpected argument {} (use --title <t>; see daiku adr new --help)",
                            quote(a)
                        );
                        return 1;
                    }
                }
                i += 1;
            }
            if opts.title.is_empty() {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["adr", "new"]));
                return 1;
            }
            if !file.is_empty() {
                // Go reads --file relative to the current folder, as the user typed it.
                match daiku_core::state::store::read_all(std::path::Path::new(&file)) {
                    Ok(data) => opts.body = data,
                    Err(e) => {
                        let _ = writeln!(stderr, "{}", report(&e));
                        return 1;
                    }
                }
            }
            finish(ws.adr_new(&opts), stderr, |r| {
                let head = format!("created {:04} ", r.number);
                line(stdout, &[head.as_bytes(), r.title.as_bytes()]);
            })
        }
        v => {
            let _ = write!(
                stderr,
                "daiku adr: unknown verb {}\n{}",
                quote(v),
                usage::usage_text(&["adr"])
            );
            1
        }
    }
}

/// Go's `runReferences`.
pub fn references(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    if args.first().map(String::as_str) != Some("check") {
        let _ = write!(stderr, "{}", usage::usage_text(&["references"]));
        return 1;
    }
    let (rest, json_mode) = crate::json::take_flag(&args[1..]);
    if !rest.is_empty() {
        let _ = write!(stderr, "{}", usage::usage_text(&["references"]));
        return 1;
    }
    if json_mode {
        return match workspace().references_check() {
            Ok(o) => crate::json::ok(stdout, &o.value, 0),
            Err(e) => crate::json::err(stdout, &e),
        };
    }
    finish(workspace().references_check(), stderr, |res| {
        if !res.present {
            let _ = writeln!(
                stdout,
                "no references/ folder: run `daiku update` to add it"
            );
            return;
        }
        for f in &res.findings {
            let _ = writeln!(stdout, "! {}", f.message);
        }
        let _ = writeln!(
            stdout,
            "{} of {} file(s) indexed, {} warning(s)",
            res.indexed,
            res.files,
            res.findings.len()
        );
    })
}

/// Go's `runIdea`.
pub fn idea(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let Some(verb) = args.first() else {
        let _ = write!(stderr, "{}", usage::usage_text(&["idea"]));
        return 1;
    };
    let rest = &args[1..];
    match verb.as_str() {
        "new" => idea_new(rest, stdout, stderr),
        "list" => {
            let (r, json_mode) = crate::json::take_flag(rest);
            if !r.is_empty() {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", "list"]));
                return 1;
            }
            if json_mode {
                return match workspace().idea_list() {
                    Ok(o) => crate::json::ok(stdout, &o.value, 0),
                    Err(e) => crate::json::err(stdout, &e),
                };
            }
            finish(workspace().idea_list(), stderr, |ideas| {
                for i in ideas {
                    let head = format!("{} [{}] ", i.id, i.status);
                    let mut parts: Vec<&[u8]> = vec![head.as_bytes(), i.title.as_bytes()];
                    let set_aside = i.status == "parked" || i.status == "dropped";
                    if set_aside && !i.reason.is_empty() {
                        parts.extend([" — reason: ".as_bytes(), i.reason.as_bytes()]);
                    }
                    line(stdout, &parts);
                }
            })
        }
        "show" => {
            let (r, json_mode) = crate::json::take_flag(rest);
            if r.len() != 1 {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", "show"]));
                return 1;
            }
            let Some(id) = parse_idea_id(&r[0], stderr) else {
                return 1;
            };
            if json_mode {
                return match workspace().idea_summary(id) {
                    Ok(o) => crate::json::ok(stdout, &o.value, 0),
                    Err(e) => crate::json::err(stdout, &e),
                };
            }
            finish(workspace().idea_show(id), stderr, |summary| {
                let _ = stdout.write_all(summary);
            })
        }
        "park" => idea_status(IdeaVerb::Park, "park", rest, stdout, stderr),
        "drop" => idea_status(IdeaVerb::Drop, "drop", rest, stdout, stderr),
        "reopen" => idea_status(IdeaVerb::Reopen, "reopen", rest, stdout, stderr),
        "promote" => idea_promote(rest, stdout, stderr),
        v => {
            let _ = write!(
                stderr,
                "daiku idea: unknown verb {}\n{}",
                quote(v),
                usage::usage_text(&["idea"])
            );
            1
        }
    }
}

/// Go's `parseIdeaID`: a whole number of at least 1.
fn parse_idea_id(s: &str, stderr: &mut impl Write) -> Option<i64> {
    match s.parse::<i64>() {
        Ok(id) if id >= 1 => Some(id),
        _ => {
            let _ = writeln!(stderr, "invalid id: {s}");
            None
        }
    }
}

/// Go's `ideaNew`.
fn idea_new(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let mut title = "";
    for a in args {
        if a.starts_with('-') {
            let _ = writeln!(stderr, "daiku idea new: unknown flag {}", quote(a));
            return 1;
        }
        if !title.is_empty() {
            let _ = writeln!(
                stderr,
                "daiku idea new: unexpected argument {} (quote a multi-word title)",
                quote(a)
            );
            return 1;
        }
        title = a;
    }
    if title.trim().is_empty() {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", "new"]));
        return 1;
    }
    finish(workspace().idea_new(title), stderr, |i| {
        let head = format!("created idea {} ", i.id);
        let tail = format!(" ({}/)", i.dir);
        line(
            stdout,
            &[head.as_bytes(), i.title.as_bytes(), tail.as_bytes()],
        );
    })
}

/// Go's `ideaStatus`: park, drop and reopen, which take an id and a reason.
fn idea_status(
    verb: IdeaVerb,
    name: &str,
    args: &[String],
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let (mut id_arg, mut reason) = ("", "");
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match args.get(i + 1) {
            Some(v) if a == "--reason" => {
                reason = v;
                i += 1;
            }
            _ if a.starts_with('-') => {
                let _ = writeln!(
                    stderr,
                    "daiku idea {name}: unknown flag {}, or it needs a value",
                    quote(a)
                );
                return 1;
            }
            _ if !id_arg.is_empty() => {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", name]));
                return 1;
            }
            _ => id_arg = a,
        }
        i += 1;
    }
    if id_arg.is_empty() || reason.trim().is_empty() {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", name]));
        return 1;
    }
    let Some(id) = parse_idea_id(id_arg, stderr) else {
        return 1;
    };
    finish(workspace().idea_set_status(verb, id, reason), stderr, |i| {
        let _ = writeln!(stdout, "idea {}: {}", i.id, i.status);
    })
}

/// Go's `ideaPromote`: the room line, then the change's lines, then the idea's.
fn idea_promote(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let mut opts = IdeaPromoteOptions {
        version: VERSION.to_string(),
        ..IdeaPromoteOptions::default()
    };
    let mut id_arg = "";
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match (a, args.get(i + 1)) {
            ("--title", Some(v)) => {
                opts.title = v.clone();
                i += 1;
            }
            ("--repo", Some(v)) => {
                opts.repos.push(v.clone());
                i += 1;
            }
            ("--rev", Some(v)) => {
                opts.rev = v.clone();
                i += 1;
            }
            _ if a.starts_with('-') => {
                let _ = writeln!(
                    stderr,
                    "daiku idea promote: unknown flag {}, or it needs a value",
                    quote(a)
                );
                return 1;
            }
            _ if !id_arg.is_empty() => {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", "promote"]));
                return 1;
            }
            _ => id_arg = a,
        }
        i += 1;
    }
    if id_arg.is_empty() || opts.title.trim().is_empty() {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["idea", "promote"]));
        return 1;
    }
    let Some(id) = parse_idea_id(id_arg, stderr) else {
        return 1;
    };
    opts.id = id;
    let res = workspace().idea_promote(&opts);
    if res.room_created {
        let _ = writeln!(stdout, "created room plan/");
    }
    let code = finish(res.change, stderr, |n| print_created_change(stdout, n));
    match res.marked {
        Some(Ok(_)) if code == 0 => {
            let _ = writeln!(stdout, "idea {id}: promoted");
            0
        }
        Some(Err(e)) => {
            let _ = writeln!(stderr, "{}", report(&e));
            1
        }
        _ => code,
    }
}

/// Go's `runOverview`. Exit codes: 0 written or current; 1 an error, or `--check`
/// found it stale; 2 `--check` couldn't check.
pub fn overview(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> u8 {
    let (mut change, mut check, mut to_stdout) = (None, false, false);
    for a in args {
        match a.as_str() {
            "--check" => check = true,
            "--stdout" => to_stdout = true,
            a if a.starts_with('-') || change.is_some() => {
                let _ = writeln!(stderr, "{}", usage::usage_line(&["overview"]));
                return 1;
            }
            a => change = Some(a),
        }
    }
    if check && to_stdout {
        let _ = writeln!(stderr, "{}", usage::usage_line(&["overview"]));
        return 1;
    }
    let doc = match workspace().overview(change, !check && !to_stdout) {
        Ok(o) => o.value,
        Err(e) => {
            let _ = writeln!(stderr, "{}", report(&e));
            return if check { 2 } else { 1 };
        }
    };
    if to_stdout {
        let _ = stdout.write_all(doc.merged.as_bytes());
    } else if doc.written {
        let _ = writeln!(stdout, "wrote {}", doc.rel);
    } else if doc.up_to_date {
        let _ = writeln!(stdout, "{} is up to date", doc.rel);
    } else {
        let _ = writeln!(stdout, "{} is out of date — run daiku overview", doc.rel);
        return 1;
    }
    0
}
