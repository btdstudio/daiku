//! The `--json` envelope for a core result, with every string made safe to show
//! (ADR-0008). Every command ends in [`reply`].

use daiku_core::json::{self, Success};
use daiku_core::text::{Display, display};
use daiku_core::{Error, Outcome};
use serde::Serialize;
use serde_json::{Map, Value};

/// `Success` or `Failure` as a JSON value, with every string and key escaped.
pub fn reply<T: Serialize>(r: Result<Outcome<T>, Error>) -> Value {
    let value = match r {
        Ok(outcome) => serde_json::to_value(Success::from_outcome(outcome)),
        Err(e) => serde_json::to_value(json::failure(&e)),
    };
    match value {
        Ok(v) => escape(v),
        Err(e) => internal(&e.to_string()),
    }
}

/// A `Failure` of kind `Internal`, for what isn't a core error.
pub fn internal(message: &str) -> Value {
    escape(serde_json::json!({
        "schema": 1,
        "error": { "kind": "Internal", "message": message },
    }))
}

/// A `Failure` of kind `TooLarge`: a file over the viewer's cap. `bytes` is the
/// file's size; the message names it in MiB to one decimal.
pub fn too_large(bytes: usize) -> Value {
    let tenths = (bytes as u128 * 10 + 524_288) / 1_048_576;
    escape(serde_json::json!({
        "schema": 1,
        "error": {
            "kind": "TooLarge",
            "message": format!("file too large to show ({}.{} MiB)", tenths / 10, tenths % 10),
        },
    }))
}

/// A `Failure` of kind `NotAnArtifact`: `read_artifact` was asked for a path that
/// `artifacts` doesn't list for the change.
pub fn not_an_artifact(path: &str, change: &str) -> Value {
    escape(serde_json::json!({
        "schema": 1,
        "error": {
            "kind": "NotAnArtifact",
            "message": format!("{path} isn't a file of {change}"),
        },
    }))
}

/// A `Failure` of kind `AmbiguousArtifact`: two or more listed files escape to the
/// same path, so the path can't say which one to read.
pub fn ambiguous_artifact(path: &str, change: &str) -> Value {
    escape(serde_json::json!({
        "schema": 1,
        "error": {
            "kind": "AmbiguousArtifact",
            "message": format!("{path} names more than one file of {change}"),
        },
    }))
}

/// A `Failure` of kind `NotATicket`: `read_ticket` was asked for a path
/// `tickets_list` doesn't show.
pub fn not_a_ticket(path: &str) -> Value {
    escape(serde_json::json!({
        "schema": 1,
        "error": {
            "kind": "NotATicket",
            "message": format!("daiku-app: {path} isn't a ticket of this workspace"),
        },
    }))
}

/// A `Failure` of kind `AmbiguousTicket`: two tickets are shown with the same
/// path.
pub fn ambiguous_ticket(path: &str) -> Value {
    escape(serde_json::json!({
        "schema": 1,
        "error": {
            "kind": "AmbiguousTicket",
            "message": format!("{path} names more than one ticket"),
        },
    }))
}

/// A `Failure` of kind `NotOpen`: a command asked for a root the person hasn't
/// opened (S-0020 security finding 1).
pub fn not_open(root: &str) -> Value {
    escape(serde_json::json!({
        "schema": 1,
        "error": {
            "kind": "NotOpen",
            "message": format!("daiku-app: won't read {root}: it isn't an open workspace"),
        },
    }))
}

pub(crate) fn shown(s: &str) -> String {
    // `display` only ever writes valid UTF-8 for valid UTF-8 input; the lossy
    // conversion can't fail and never returns the raw string.
    String::from_utf8_lossy(&display(s.as_bytes(), Display::Document)).into_owned()
}

/// Every string and object key through `display(.., Document)`: control and
/// invisible characters become visible escapes; `\n` and `\t` stay.
pub fn escape(v: Value) -> Value {
    match v {
        Value::String(s) => Value::String(shown(&s)),
        Value::Array(a) => Value::Array(a.into_iter().map(escape).collect()),
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| (shown(&k), escape(v)))
                .collect::<Map<_, _>>(),
        ),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct Sample {
        esc: String,
        lines: String,
        tab: String,
        map: BTreeMap<String, String>,
    }

    #[test]
    fn success_escapes_strings_and_keys() {
        let sample = Sample {
            esc: "a\u{1b}b".into(),
            lines: "line1\nline2".into(),
            tab: "x\ty".into(),
            map: BTreeMap::from([("k\u{1b}".to_string(), "v".to_string())]),
        };
        let v = reply(Ok(Outcome::new(sample)));
        assert_eq!(v["schema"], 1);
        assert_eq!(v["warnings"], serde_json::json!([]));
        assert_eq!(v["data"]["esc"], "a\\x1bb");
        assert_eq!(v["data"]["lines"], "line1\nline2");
        assert_eq!(v["data"]["tab"], "x\ty");
        assert_eq!(v["data"]["map"]["k\\x1b"], "v");
        assert!(v["data"]["map"].get("k\u{1b}").is_none());
    }

    #[test]
    fn failure_escapes_the_message() {
        let e = Error::Frontmatter {
            message: "bad\u{1b}[31m input".into(),
        };
        let v = reply::<()>(Err(e));
        assert_eq!(v["schema"], 1);
        assert!(v.get("data").is_none());
        assert!(v["error"]["kind"].is_string());
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains("\\x1b[31m"), "{msg}");
        assert!(!msg.contains('\u{1b}'));
    }

    #[test]
    fn not_open_names_the_root_escaped() {
        let v = not_open("/tmp/a\u{1b}b");
        assert_eq!(v["error"]["kind"], "NotOpen");
        let msg = v["error"]["message"].as_str().unwrap();
        assert!(msg.contains("/tmp/a\\x1bb"), "{msg}");
        assert!(!msg.contains('\u{1b}'));
    }
}
