//! A scriptable fake `daiku` for the fixture runner's self-tests.
//!
//! Usage: `daiku-fixtures-stub STEP [ARG]... [; STEP [ARG]...]...`
//!
//! Steps run in order: `print TEXT`, `eprint TEXT`, `write PATH TEXT`, `mkdir PATH`,
//! `cat PATH` (prints the file), `stamp PATH` (writes today's date and the time to
//! the file), `now`, `today`, `pwd`, `env`, `commit SUBJECT` (prints the new commit
//! id), `staged` (prints the staged paths), `sleep MS`, `exit CODE`. With no `exit`, the stub exits 0.

use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

use daiku_fixtures::civil;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    for step in args.split(|a| a == ";") {
        if step.is_empty() {
            continue;
        }
        match run(step) {
            Ok(None) => {}
            Ok(Some(code)) => return ExitCode::from(code),
            Err(msg) => {
                eprintln!("stub: {msg}");
                return ExitCode::from(2);
            }
        }
    }
    ExitCode::SUCCESS
}

fn run(step: &[String]) -> Result<Option<u8>, String> {
    let name = step[0].as_str();
    let rest = &step[1..];
    let want = |n: usize| {
        if rest.len() == n {
            Ok(())
        } else {
            Err(format!("{name}: want {n} argument(s), got {}", rest.len()))
        }
    };
    match name {
        "print" => {
            want(1)?;
            println!("{}", rest[0]);
        }
        "eprint" => {
            want(1)?;
            eprintln!("{}", rest[0]);
        }
        "write" => {
            want(2)?;
            let path = std::path::Path::new(&rest[0]);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("write: {e}"))?;
            }
            std::fs::write(path, &rest[1]).map_err(|e| format!("write: {e}"))?;
        }
        "mkdir" => {
            want(1)?;
            std::fs::create_dir_all(&rest[0]).map_err(|e| format!("mkdir: {e}"))?;
        }
        "cat" => {
            want(1)?;
            let text = std::fs::read_to_string(&rest[0]).map_err(|e| format!("cat: {e}"))?;
            print!("{text}");
        }
        "stamp" => {
            want(1)?;
            let (secs, millis) = now();
            let (date, time) = civil(secs);
            let text = format!("date {date}\nat {date}T{time}.{millis:03}Z\n");
            std::fs::write(&rest[0], text).map_err(|e| format!("stamp: {e}"))?;
        }
        "now" => {
            want(0)?;
            let (secs, millis) = now();
            let (date, time) = civil(secs);
            println!("{date}T{time}.{millis:03}Z");
        }
        "today" => {
            want(0)?;
            println!("{}", civil(now().0).0);
        }
        "pwd" => {
            want(0)?;
            let dir = std::env::current_dir().map_err(|e| format!("pwd: {e}"))?;
            println!("{}", dir.display());
        }
        "env" => {
            want(0)?;
            let mut vars: Vec<(String, String)> = std::env::vars().collect();
            vars.sort();
            for (k, v) in vars {
                println!("{k}={v}");
            }
        }
        "staged" => {
            want(0)?;
            print!("{}", git(&["diff", "--cached", "--name-only"])?);
        }
        "commit" => {
            want(1)?;
            git(&["add", "-A"])?;
            git(&["commit", "-q", "-m", &rest[0]])?;
            print!("{}", git(&["rev-parse", "HEAD"])?);
        }
        "sleep" => {
            want(1)?;
            let ms = rest[0]
                .parse()
                .map_err(|_| format!("sleep: bad ms {:?}", rest[0]))?;
            std::thread::sleep(std::time::Duration::from_millis(ms));
        }
        "exit" => {
            want(1)?;
            let code = rest[0]
                .parse()
                .map_err(|_| format!("exit: bad code {:?}", rest[0]))?;
            return Ok(Some(code));
        }
        other => return Err(format!("unknown step {other:?}")),
    }
    Ok(None)
}

fn git(args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .stderr(std::process::Stdio::inherit())
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!("git {} failed: {}", args.join(" "), out.status))
    }
}

fn now() -> (i64, u32) {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (d.as_secs() as i64, d.subsec_millis())
}
