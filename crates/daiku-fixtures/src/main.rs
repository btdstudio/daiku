//! `daiku-fixtures`: check, capture or measure coverage of fixture cases.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use daiku_fixtures::{
    Case, Outcome, Summary, build_workspaces, case_report, compare, date_warnings, load_cases,
    run_case, uncovered, usage_entries, write_expect,
};

const USAGE: &str = "usage:
  daiku-fixtures check    --bin PATH [--cases DIR] [--list FILE] [CASE...]
  daiku-fixtures capture  --bin PATH [--cases DIR] [CASE...]
  daiku-fixtures coverage [--cases DIR] [--usage FILE]
  daiku-fixtures workspaces --bin PATH [--workspaces DIR] [NAME...]
";

struct Args {
    command: String,
    bin: Option<PathBuf>,
    cases: PathBuf,
    usage: PathBuf,
    workspaces: PathBuf,
    names: Vec<String>,
}

fn parse(argv: &[String]) -> Result<Args, String> {
    let (command, rest) = argv.split_first().ok_or("missing command")?;
    if !matches!(
        command.as_str(),
        "check" | "capture" | "coverage" | "workspaces"
    ) {
        return Err(format!("unknown command {command:?}"));
    }
    let mut args = Args {
        command: command.clone(),
        bin: None,
        cases: PathBuf::from("fixtures/cases"),
        usage: PathBuf::from("fixtures/usage.txt"),
        workspaces: PathBuf::from("fixtures/workspaces"),
        names: Vec::new(),
    };
    let mut it = rest.iter();
    while let Some(a) = it.next() {
        let mut value = || {
            it.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{a} needs a value"))
        };
        match a.as_str() {
            "--bin" if args.command != "coverage" => args.bin = Some(value()?),
            "--cases" if args.command != "workspaces" => args.cases = value()?,
            "--list" if args.command == "check" => {
                let file = value()?;
                let text = std::fs::read_to_string(&file)
                    .map_err(|e| format!("{}: {e}", file.display()))?;
                let names = list_names(&text);
                if names.is_empty() {
                    return Err(format!("{} lists no cases", file.display()));
                }
                args.names.extend(names);
            }
            "--workspaces" if args.command == "workspaces" => args.workspaces = value()?,
            "--usage" if args.command == "coverage" => args.usage = value()?,
            f if f.starts_with('-') => {
                return Err(format!("unknown flag {f:?} for {}", args.command));
            }
            name if args.command != "coverage" => args.names.push(name.to_string()),
            name => return Err(format!("coverage takes no case names, got {name:?}")),
        }
    }
    if args.command != "coverage" && args.bin.is_none() {
        return Err(format!("{} needs --bin PATH", args.command));
    }
    Ok(args)
}

/// Case names from a list file: one per line, blank lines and `#` comments skipped.
fn list_names(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse(&argv) {
        Ok(a) => a,
        Err(e) => {
            eprint!("daiku-fixtures: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    if args.command == "workspaces" {
        let bin = args.bin.as_deref().unwrap_or(Path::new(""));
        return match build_workspaces(bin, &args.workspaces, &args.names) {
            Ok(built) => {
                for name in &built {
                    out(&format!("built {name}\n"));
                }
                out(&format!("{} built\n", built.len()));
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("daiku-fixtures: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let cases = match load_cases(&args.cases, &args.names) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("daiku-fixtures: {e}");
            return ExitCode::FAILURE;
        }
    };
    let ok = match args.command.as_str() {
        "check" => check(args.bin.as_deref().unwrap_or(Path::new("")), &cases),
        "capture" => capture(args.bin.as_deref().unwrap_or(Path::new("")), &cases),
        _ => coverage(&args.usage, &cases),
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn out(s: &str) {
    let mut stdout = std::io::stdout().lock();
    let _ = stdout.write_all(s.as_bytes());
    let _ = stdout.flush();
}

fn check(bin: &Path, cases: &[Case]) -> bool {
    let mut summary = Summary::default();
    for case in cases {
        let outcome = if let Some(os) = case.skipped_here() {
            Outcome::Skip(os)
        } else {
            match run_case(bin, case) {
                Ok(obs) => {
                    let diffs = compare(case, &obs);
                    if diffs.is_empty() {
                        Outcome::Pass
                    } else {
                        Outcome::Fail(diffs)
                    }
                }
                Err(e) => Outcome::Error(e),
            }
        };
        out(&case_report(&case.name, &outcome));
        summary.add(&outcome);
    }
    out(&summary.line());
    summary.failed == 0
}

fn capture(bin: &Path, cases: &[Case]) -> bool {
    let (mut captured, mut skipped, mut failed) = (0, 0, 0);
    for case in cases {
        if let Some(os) = case.skipped_here() {
            out(&case_report(&case.name, &Outcome::Skip(os)));
            skipped += 1;
            continue;
        }
        for w in date_warnings(case) {
            eprintln!("warning: {w}");
        }
        match run_case(bin, case).and_then(|obs| write_expect(case, &obs)) {
            Ok(()) => {
                out(&format!("captured {}\n", case.name));
                captured += 1;
            }
            Err(e) => {
                out(&case_report(&case.name, &Outcome::Error(e)));
                failed += 1;
            }
        }
    }
    out(&format!(
        "{captured} captured, {failed} failed, {skipped} skipped\n"
    ));
    failed == 0
}

fn coverage(usage_file: &Path, cases: &[Case]) -> bool {
    let usage = match std::fs::read_to_string(usage_file) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("daiku-fixtures: {}: {e}", usage_file.display());
            return false;
        }
    };
    let total = usage_entries(&usage).len();
    let missing = uncovered(&usage, cases);
    for m in &missing {
        out(&format!("uncovered: {m}\n"));
    }
    out(&format!("{} of {total} covered\n", total - missing.len()));
    missing.is_empty()
}
