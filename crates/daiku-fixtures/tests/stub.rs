//! The stub binary's script steps.

use std::process::{Command, Output};

fn stub(args: &[&str], cwd: &std::path::Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_daiku-fixtures-stub"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("run stub")
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).replace("\r\n", "\n")
}

#[test]
fn steps_run_in_order() {
    let dir = tempfile::tempdir().unwrap();
    let out = stub(
        &[
            "print", "one", ";", "eprint", "two", ";", "write", "a/b.txt", "hi", ";", "mkdir",
            "empty", ";", "print", "three", ";", "exit", "3", ";", "print", "never",
        ],
        dir.path(),
    );
    assert_eq!(text(&out.stdout), "one\nthree\n");
    assert_eq!(text(&out.stderr), "two\n");
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("a/b.txt")).unwrap(),
        "hi"
    );
    assert!(dir.path().join("empty").is_dir());
}

#[test]
fn time_steps_have_iso_shapes() {
    let dir = tempfile::tempdir().unwrap();
    let out = stub(&["now", ";", "today"], dir.path());
    assert!(out.status.success());
    let s = text(&out.stdout);
    let lines: Vec<&str> = s.lines().collect();
    let ts = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$").unwrap();
    let day = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    assert!(ts.is_match(lines[0]), "{}", lines[0]);
    assert!(day.is_match(lines[1]), "{}", lines[1]);
    assert!(lines[0].starts_with(lines[1]) || lines[0] < lines[1]);
}

#[test]
fn unknown_step_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let out = stub(&["bogus"], dir.path());
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("unknown step"));
}
