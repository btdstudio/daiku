//! `--json` output for R5 read commands (ADR-0004, D8).

use std::io::Write;

use daiku_core::Error;
use daiku_core::json::{Success, failure, to_pretty};
use serde::Serialize;

/// Returns `(args_without_json, json_requested)`.
pub fn take_flag(args: &[String]) -> (Vec<String>, bool) {
    let mut out = Vec::with_capacity(args.len());
    let mut json = false;
    for a in args {
        if a == "--json" {
            json = true;
        } else {
            out.push(a.clone());
        }
    }
    (out, json)
}

/// Writes a success envelope to stdout. Exit 0 unless `exit` is overridden.
pub fn ok<T: Serialize>(stdout: &mut impl Write, data: T, exit: u8) -> u8 {
    let _ = stdout.write_all(&to_pretty(&Success::new(data)).expect("json"));
    exit
}

/// Writes an error envelope to stdout and returns 1.
pub fn err(stdout: &mut impl Write, e: &Error) -> u8 {
    let _ = stdout.write_all(&to_pretty(&failure(e)).expect("json"));
    1
}

/// Prints a failure envelope with an explicit kind/message and returns `code`.
pub fn err_raw(stdout: &mut impl Write, kind: &str, message: String, code: u8) -> u8 {
    let f = daiku_core::json::Failure {
        schema: daiku_core::json::SCHEMA,
        error: daiku_core::json::ErrorBody {
            kind: kind.to_string(),
            message,
        },
    };
    let _ = stdout.write_all(&to_pretty(&f).expect("json"));
    code
}
