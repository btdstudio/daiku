//! Text read from a user's file: bytes that may not be valid UTF-8.
//!
//! Go keeps such bytes in its strings, prints them as they are, and writes U+FFFD
//! for them only in JSON. [`Text`] does the same: clients print
//! [`Text::as_bytes`], while `Display` and `Serialize` decode lossily, for messages
//! and for JSON (N-0001 D1).

use std::borrow::Cow;
use std::fmt;
use std::sync::LazyLock;

use regex::Regex;

/// A run of whitespace that holds a line break: `\n`, `\r\n`, or a lone `\r`.
static LINE_BREAK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\t\n\x0C\r ]*[\r\n]+[\t\n\x0C\r ]*").expect("valid pattern"));

/// Go's `oneLine`, widened to a lone `\r` (N-0006): line breaks and the whitespace
/// around them become one space, and the ends are trimmed. Whitespace away from a
/// break is kept.
pub fn one_line(s: &str) -> String {
    LINE_BREAK_RE.replace_all(s, " ").trim().to_string()
}

/// A `--reason` as core stores and logs it: unchanged when it has no line break,
/// otherwise [`one_line`] of it, so it can't add a line to `LOG.md` (N-0006).
pub fn fold_reason(reason: &str) -> Cow<'_, str> {
    if reason.contains(['\r', '\n']) {
        Cow::Owned(one_line(reason))
    } else {
        Cow::Borrowed(reason)
    }
}

/// A parsed field (a title, a body, a reason) as the file holds it.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Text(Vec<u8>);

impl Text {
    /// The bytes, for a client to print or write.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The text with U+FFFD for each invalid sequence, for messages and matching.
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) => fmt::Debug::fmt(s, f),
            Err(_) => write!(f, "b{:?}", self.0.escape_ascii().to_string()),
        }
    }
}

/// As Go's `encoding/json` writes a string holding invalid UTF-8.
impl serde::Serialize for Text {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string_lossy())
    }
}

impl schemars::JsonSchema for Text {
    fn schema_name() -> String {
        "Text".into()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        <String>::json_schema(generator)
    }
}

impl PartialEq<&str> for Text {
    fn eq(&self, other: &&str) -> bool {
        self.0 == other.as_bytes()
    }
}

impl PartialEq<str> for Text {
    fn eq(&self, other: &str) -> bool {
        self.0 == other.as_bytes()
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Text {
        Text(s.as_bytes().to_vec())
    }
}

impl From<String> for Text {
    fn from(s: String) -> Text {
        Text(s.into_bytes())
    }
}

impl From<Vec<u8>> for Text {
    fn from(b: Vec<u8>) -> Text {
        Text(b)
    }
}

impl From<&[u8]> for Text {
    fn from(b: &[u8]) -> Text {
        Text(b.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AC6: JSON is valid, with U+FFFD for the invalid byte, as Go writes it.
    #[test]
    fn serialises_lossily_as_go() {
        let t = Text::from(&b"Caf\xe9 \"q\""[..]);
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"Caf\u{fffd} \\\"q\\\"\"");
        let back: String = serde_json::from_str(&json).unwrap();
        assert_eq!(back, "Caf\u{fffd} \"q\"");
    }

    #[test]
    fn one_line_folds_line_breaks() {
        for (s, want) in [
            ("  a \r\n\n  b\nc  ", "a b c"),
            ("a\rb", "a b"),
            ("a\r\nb", "a b"),
            ("a \t\n\t b", "a b"),
            ("a\n\r\n\rb", "a b"),
            ("\na\n", "a"),
            ("a  b\tc", "a  b\tc"),
            ("\n", ""),
        ] {
            assert_eq!(one_line(s), want, "{s:?}");
        }
    }

    #[test]
    fn fold_reason_only_touches_reasons_with_a_break() {
        assert!(matches!(fold_reason(" a  b "), Cow::Borrowed(" a  b ")));
        assert_eq!(
            fold_reason("x\n- 2026-01-01 S-0001: ship → complete"),
            "x - 2026-01-01 S-0001: ship → complete"
        );
        assert_eq!(fold_reason("a\rb"), "a b");
        assert_eq!(fold_reason("\n"), "");
        assert_eq!(fold_reason(" \r\n "), "");
    }

    #[test]
    fn keeps_bytes_and_compares_exactly() {
        let t = Text::from(&b"a\xff"[..]);
        assert_eq!(t.as_bytes(), b"a\xff");
        assert_eq!(t.to_string(), "a\u{fffd}");
        assert!(t != "a\u{fffd}", "comparison is on bytes");
        assert_eq!(Text::from("x"), "x");
        assert_eq!(format!("{:?}", Text::from("x")), "\"x\"");
        assert_eq!(format!("{t:?}"), "b\"a\\\\xff\"");
    }
}
