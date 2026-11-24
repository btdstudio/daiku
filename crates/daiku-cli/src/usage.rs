//! The usage table: the one source of `daiku help`, every `help <command> [<verb>]`
//! page and every usage line (R3, D10). Ported from Go's `internal/cli/usage.go`;
//! the table's text is copied from it.

use std::io::Write;

/// One command or verb.
pub struct Command {
    /// `["adr", "new"]`; a shorter shared prefix is a group.
    pub path: &'static [&'static str],
    /// Every flag the command's parser accepts.
    pub synopsis: &'static str,
    pub summary: &'static str,
    /// Extra `--help` lines, such as what a flag means.
    pub notes: &'static str,
    /// Replaces the synopsis and summary in listings (`repo`).
    pub raw: &'static str,
}

/// The `repo` block, as Go's `repos.Usage("daiku")` prints it.
const REPO_USAGE: &str = "  daiku repo add <name> <remote> [--path <p>] [--branch <b>]
                                          register a repo
  daiku repo list                          list repos and their local paths
  daiku repo path <name> <path>            set a repo's local path
  daiku repo doctor [--clone] [--into <dir>]
                                          check repos; --clone fetches missing ones
  daiku repo archive <name> [--reason <r>]
                                          mark a repo archived: no clones, no new builds
  daiku repo unarchive <name>             make an archived repo live again
";

pub const COMMANDS: &[Command] = &[
    Command {
        path: &["init"],
        synopsis: "daiku init [dir] [--no-git] [--force]",
        summary: "scaffold the planning ICM structure",
        notes: "  --no-git   a local workspace (vcs: none)\n  --force    overwrite files that already exist\n",
        raw: "",
    },
    Command {
        path: &["update"],
        synopsis: "daiku update",
        summary: "refresh managed files",
        notes: "",
        raw: "",
    },
    Command {
        path: &["status"],
        synopsis: "daiku status [<plan-change>]",
        summary: "show the active change and the board, or a plan's tickets",
        notes: "",
        raw: "",
    },
    Command {
        path: &["artifacts"],
        synopsis: "daiku artifacts <change>",
        summary: "list a change's files by phase (plan or build)",
        notes: "",
        raw: "",
    },
    Command {
        path: &["next"],
        synopsis: "daiku next",
        summary: "list what is waiting on you: gates to approve, phases to start, tickets ready to build",
        notes: "",
        raw: "",
    },
    Command {
        path: &["settings"],
        synopsis: "daiku settings",
        summary: "show effective settings",
        notes: "",
        raw: "",
    },
    Command {
        path: &["switch"],
        synopsis: "daiku switch [--all] [<change>]",
        summary: "list changes, or switch the active one",
        notes: "  --all      include closed changes in the list\n",
        raw: "",
    },
    Command {
        path: &["source", "new"],
        synopsis: "daiku source new --title <t> [--repo <path>] [--rev <r>] [--body <b>]",
        summary: "add an intake source to plan/00_intake/",
        notes: "",
        raw: "",
    },
    Command {
        path: &["source", "list"],
        synopsis: "daiku source list",
        summary: "list intake sources (the default verb)",
        notes: "",
        raw: "",
    },
    Command {
        path: &["source", "show"],
        synopsis: "daiku source show <id>",
        summary: "show an intake source",
        notes: "",
        raw: "",
    },
    Command {
        path: &["adr", "new"],
        synopsis: "daiku adr new --title <t> [--deciders <d>] [--context <c>] [--decision <d>] [--consequences <c>] [--file <path>]",
        summary: "create an ADR in plan/04_architecture/adr/",
        notes: "  --file     read the ADR body from a file\n",
        raw: "",
    },
    Command {
        path: &["adr", "list"],
        synopsis: "daiku adr list",
        summary: "list ADRs (the default verb)",
        notes: "",
        raw: "",
    },
    Command {
        path: &["adr", "show"],
        synopsis: "daiku adr show <number>",
        summary: "show an ADR",
        notes: "",
        raw: "",
    },
    Command {
        path: &["build", "new"],
        synopsis: "daiku build new <ticket-id>",
        summary: "build a ticket in its repo; creates build/ the first time",
        notes: "",
        raw: "",
    },
    Command {
        path: &["change", "new"],
        synopsis: "daiku change new <title> [--repo <name|path>]... [--rev <r>] [--body <b>]",
        summary: "create a change + intake + switch",
        notes: "",
        raw: "",
    },
    Command {
        path: &["change", "close"],
        synopsis: "daiku change close <change> --reason <r>",
        summary: "close a change",
        notes: "",
        raw: "",
    },
    Command {
        path: &["gate"],
        synopsis: "daiku gate <phase> <status> [--reason <r>] [--force]",
        summary: "advance the active change's gate",
        notes: "",
        raw: "",
    },
    Command {
        path: &["reopen"],
        synopsis: "daiku reopen <phase> --reason <r> [--force]",
        summary: "move back to a phase, clearing later approvals",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "new"],
        synopsis: "daiku tickets new <type> <title> [--parent <id>] [--repo <name>] [--depends <id,id>] [--requirements <id,id>] [--inbox]",
        summary: "create a ticket in the active plan change, or with --inbox (or no open plan change) an N- task in build/tickets/",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "pull"],
        synopsis: "daiku tickets pull <file>",
        summary: "record a ticket fetched from a PM tool as X-<pm-id> in build/tickets/",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "check"],
        synopsis: "daiku tickets check",
        summary: "validate every ticket",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "status"],
        synopsis: "daiku tickets status",
        summary: "list tickets by draft, ready, published and drifted",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "list"],
        synopsis: "daiku tickets list",
        summary: "list every ticket with its fields, from every plan and the inbox",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "order"],
        synopsis: "daiku tickets order",
        summary: "list tickets in publish order",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "ready"],
        synopsis: "daiku tickets ready <id>... | --all [--force]",
        summary: "ready draft tickets once breakdown is approved",
        notes: "",
        raw: "",
    },
    Command {
        path: &["tickets", "mark-published"],
        synopsis: "daiku tickets mark-published <id> --external <tool>=<item-id> [--force]",
        summary: "record a PM tool's id and freeze the ticket",
        notes: "  --force    allow it outside the publish phase\n",
        raw: "",
    },
    Command {
        path: &["tickets", "unfreeze"],
        synopsis: "daiku tickets unfreeze <id> --reason <r>",
        summary: "clear a ticket's freeze",
        notes: "",
        raw: "",
    },
    Command {
        path: &["overview"],
        synopsis: "daiku overview [<change>] [--check] [--stdout]",
        summary: "write or check the change's README overview",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "new"],
        synopsis: "daiku idea new <title>",
        summary: "create an idea in ideas/, from the summary template",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "list"],
        synopsis: "daiku idea list",
        summary: "list ideas with their status",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "show"],
        synopsis: "daiku idea show <id>",
        summary: "print an idea's summary and status",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "park"],
        synopsis: "daiku idea park <id> --reason <r>",
        summary: "set an open idea aside, with a reason",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "drop"],
        synopsis: "daiku idea drop <id> --reason <r>",
        summary: "drop an open idea, with a reason (the folder stays)",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "reopen"],
        synopsis: "daiku idea reopen <id> --reason <r>",
        summary: "reopen a parked or dropped idea, with a reason",
        notes: "",
        raw: "",
    },
    Command {
        path: &["idea", "promote"],
        synopsis: "daiku idea promote <id> --title <t> [--repo <name|path>]... [--rev <r>]",
        summary: "start a change from an idea",
        notes: "",
        raw: "",
    },
    Command {
        path: &["references", "check"],
        synopsis: "daiku references check",
        summary: "warn about unindexed references and bad index entries",
        notes: "",
        raw: "",
    },
    Command {
        path: &["repo"],
        synopsis: "",
        summary: "",
        notes: "",
        raw: REPO_USAGE,
    },
    Command {
        path: &["env"],
        synopsis: "daiku env",
        summary: "print harness flags for the active change's repos",
        notes: "",
        raw: "",
    },
    Command {
        path: &["commit"],
        synopsis: "daiku commit [git commit args]",
        summary: "commit what's staged in the active build's repo",
        notes: "",
        raw: "",
    },
    Command {
        path: &["pr"],
        synopsis: "daiku pr [--template | --body-file <path>] [title]",
        summary: "open a pull request from the active build's repo, linking its ticket",
        notes: "  --template   print the pull request template, with the links in place, and open nothing\n  --body-file  use the text in <path> as the body; the links block is refreshed or added\n\nThe body starts from the first template found: the repo's pull_request_template.md\n(.github/, the repo root, then docs/), the workspace's\nbuild/05_ship/templates/pull-request.md, then daiku's built-in copy.\n",
        raw: "",
    },
    Command {
        path: &["check"],
        synopsis: "daiku check",
        summary: "validate the embedded scaffold",
        notes: "",
        raw: "",
    },
    Command {
        path: &["version"],
        synopsis: "daiku version",
        summary: "print the version",
        notes: "",
        raw: "",
    },
    Command {
        path: &["help"],
        synopsis: "daiku help [<command> [<verb>]]",
        summary: "this help, or one command's",
        notes: "",
        raw: "",
    },
];

/// Where summaries start in a listing.
const SUMMARY_COLUMN: usize = 43;

impl Command {
    /// The command's lines in a listing. Widths count bytes, as Go's `len` does.
    fn listing(&self) -> String {
        if !self.raw.is_empty() {
            return self.raw.to_string();
        }
        let line = format!("  {}", self.synopsis);
        if line.len() < SUMMARY_COLUMN {
            return format!(
                "{line}{}{}\n",
                " ".repeat(SUMMARY_COLUMN - line.len()),
                self.summary
            );
        }
        format!("{line}\n{}{}\n", " ".repeat(SUMMARY_COLUMN), self.summary)
    }

    fn is(&self, path: &[&str]) -> bool {
        self.path == path
    }
}

/// The full usage, as `daiku help` prints it.
pub fn usage() -> String {
    let mut out = String::from("daiku — folder-driven planning workspace\n\nUsage:\n");
    for c in COMMANDS {
        out.push_str(&c.listing());
    }
    out
}

/// The commands whose path starts with `prefix`.
pub fn group(prefix: &[&str]) -> Vec<&'static Command> {
    COMMANDS
        .iter()
        .filter(|c| c.path.len() >= prefix.len() && c.path[..prefix.len()] == *prefix)
        .collect()
}

/// `usage: <synopsis>` for one command, or `usage:` and a listing for a group.
pub fn usage_text(path: &[&str]) -> String {
    let cmds = group(path);
    if cmds.len() == 1 && cmds[0].is(path) && cmds[0].raw.is_empty() {
        return format!("usage: {}\n", cmds[0].synopsis);
    }
    let mut out = String::from("usage:\n");
    for c in cmds {
        out.push_str(&c.listing());
    }
    out
}

/// [`usage_text`] without its final newline. Argument errors print it; the parsers
/// that need it are ported from S-0004 on.
#[allow(dead_code)]
pub fn usage_line(path: &[&str]) -> String {
    usage_text(path).trim_end_matches('\n').to_string()
}

/// A command's `--help` page: its usage, then its summary and notes.
pub fn help_text(path: &[&str]) -> String {
    let mut text = usage_text(path);
    let cmds = group(path);
    if cmds.len() == 1 && cmds[0].raw.is_empty() && cmds[0].is(path) {
        text.push_str(&format!("\n{}\n{}", cmds[0].summary, cmds[0].notes));
    }
    text
}

/// Go's `cli.Help`: answers `--help`, `-h` (anywhere before `--`) or `help` as the
/// first word, printing to `stdout` (or the unknown-command error to `stderr`).
/// `None` when `args` isn't a help request, or is `daiku bogus --help`, which
/// dispatch then reports as an unknown command.
pub fn help(args: &[String], stdout: &mut impl Write, stderr: &mut impl Write) -> Option<u8> {
    let mut asked = false;
    let mut words: Vec<&str> = Vec::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--" {
            break;
        }
        if a == "--help" || a == "-h" || (i == 0 && a == "help") {
            asked = true;
            continue;
        }
        words.push(a);
    }
    if !asked {
        return None;
    }
    let mut path: Vec<&str> = Vec::new();
    for w in &words {
        let mut next = path.clone();
        next.push(w);
        if w.starts_with('-') || group(&next).is_empty() {
            break;
        }
        path = next;
    }
    if path.is_empty() || path == ["help"] {
        if path.is_empty() && !words.is_empty() && !words[0].starts_with('-') {
            if args[0] == words[0] {
                return None; // `daiku bogus --help`: dispatch reports it
            }
            let _ = write!(
                stderr,
                "daiku: unknown command {}\n{}",
                daiku_core::gofmt::quote(words[0]),
                usage()
            );
            return Some(1);
        }
        let _ = stdout.write_all(usage().as_bytes());
        return Some(0);
    }
    let _ = stdout.write_all(help_text(&path).as_bytes());
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listing_matches_usage_txt() {
        let usage_txt = include_str!("../../../fixtures/usage.txt");
        assert_eq!(usage(), usage_txt);
    }

    fn run(args: &[&str]) -> (Option<u8>, String, String) {
        let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = help(&args, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn help_pages() {
        assert_eq!(run(&["status"]).0, None);
        assert_eq!(run(&["help"]).1, usage());
        assert_eq!(
            run(&["gate", "--help"]).1,
            "usage: daiku gate <phase> <status> [--reason <r>] [--force]\n\nadvance the active change's gate\n"
        );
        assert!(
            run(&["help", "tickets"])
                .1
                .starts_with("usage:\n  daiku tickets new")
        );
        assert!(
            run(&["help", "repo", "add"])
                .1
                .starts_with("usage:\n  daiku repo add")
        );
        assert_eq!(run(&["bogus", "--help"]).0, None);
        let (code, out, err) = run(&["help", "bogus"]);
        assert_eq!((code, out.as_str()), (Some(1), ""));
        assert!(err.starts_with("daiku: unknown command \"bogus\"\ndaiku — "));
        assert_eq!(run(&["--", "--help"]).0, None);
        assert_eq!(
            run(&["tickets", "ready", "S-1", "-h"]).1,
            help_text(&["tickets", "ready"])
        );
    }

    #[test]
    fn usage_lines() {
        assert_eq!(
            usage_line(&["idea", "new"]),
            "usage: daiku idea new <title>"
        );
        assert!(usage_text(&["idea"]).starts_with("usage:\n  daiku idea new <title>"));
    }
}
