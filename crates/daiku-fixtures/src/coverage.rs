//! Which commands and verbs in the Go usage text no case exercises (R9, D14).

use crate::Case;

/// The command paths in `daiku help` output, in order and without repeats: for each
/// line starting with `  daiku `, the lowercase words after `daiku`, up to the first
/// flag, placeholder or bracket (`tickets mark-published`, `init`). The synopsis ends
/// at the first run of two spaces, where the summary starts.
pub fn usage_entries(usage: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in usage.lines() {
        let Some(rest) = line.strip_prefix("  daiku ") else {
            continue;
        };
        let synopsis = rest.split("  ").next().unwrap_or(rest);
        let words: Vec<&str> = synopsis
            .split_whitespace()
            .take_while(|w| {
                w.starts_with(|c: char| c.is_ascii_lowercase())
                    && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')
            })
            .collect();
        if words.is_empty() {
            continue;
        }
        let path = words.join(" ");
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

/// The usage entries no case's `args` start with.
pub fn uncovered(usage: &str, cases: &[Case]) -> Vec<String> {
    usage_entries(usage)
        .into_iter()
        .filter(|entry| {
            let words: Vec<&str> = entry.split(' ').collect();
            !cases.iter().any(|c| {
                c.spec.args.len() >= words.len()
                    && c.spec.args.iter().zip(&words).all(|(a, w)| a == w)
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const USAGE: &str = "daiku — folder-driven planning workspace

Usage:
  daiku init [dir] [--no-git] [--force]    scaffold
  daiku source new --title <t> [--repo <path>]
                                           add an intake source to plan/00_intake/
  daiku gate <phase> <status> [--reason <r>]
  daiku tickets ready <id>... | --all [--force]
  daiku tickets mark-published <id> --external <tool>=<item-id>
  daiku tickets check                      validate every ticket
  daiku commit [git commit args]           commit what's staged
  daiku init                               again
";

    #[test]
    fn parses_command_paths() {
        assert_eq!(
            usage_entries(USAGE),
            [
                "init",
                "source new",
                "gate",
                "tickets ready",
                "tickets mark-published",
                "tickets check",
                "commit",
            ]
        );
    }

    #[test]
    fn checked_in_usage_is_all_uncovered_with_no_cases() {
        let usage = include_str!("../../../fixtures/usage.txt");
        let entries = usage_entries(usage);
        assert!(entries.len() > 40, "{entries:?}");
        for want in ["init", "tickets mark-published", "repo doctor", "help"] {
            assert!(entries.iter().any(|e| e == want), "{want}");
        }
        assert_eq!(uncovered(usage, &[]), entries);
    }
}
