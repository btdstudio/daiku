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

/// How [`display`] treats line breaks (ADR-0008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// A single-line field: line breaks are escaped too.
    Field,
    /// A whole file or a message: `\n`, `\t` and `\r\n` are kept.
    Document,
}

/// The characters a terminal could act on, which [`display`] shows as escapes:
/// C0 controls except tab, DEL, C1 controls, the line and paragraph separators,
/// and the bidi controls; and the invisible ones that can hide text from a reader:
/// zero-width space, BOM and the tag characters (ADR-0008, N-0007 security review).
/// ZWJ (U+200D) isn't one, so emoji sequences print as they are.
fn escaped(c: char) -> bool {
    matches!(c as u32,
        0x00..=0x08 | 0x0A..=0x1F | 0x7F..=0x9F | 0x2028 | 0x2029
        | 0x061C | 0x200E | 0x200F | 0x202A..=0x202E | 0x2066..=0x2069
        | 0x200B | 0xFEFF | 0xE0000..=0xE007F)
}

/// `c`'s escape, in `gofmt::quote`'s forms.
fn escape(c: char) -> String {
    match c {
        '\u{7}' => "\\a".into(),
        '\u{8}' => "\\b".into(),
        '\u{c}' => "\\f".into(),
        '\n' => "\\n".into(),
        '\r' => "\\r".into(),
        '\u{b}' => "\\v".into(),
        c if (c as u32) < 0x20 || c as u32 == 0x7f => format!("\\x{:02x}", c as u32),
        c if (c as u32) < 0x10000 => format!("\\u{:04x}", c as u32),
        c => format!("\\U{:08x}", c as u32),
    }
}

/// Text for a terminal (ADR-0008): the characters a terminal could act on become
/// visible escapes, and each byte of invalid UTF-8 becomes `\xNN`. `Document` keeps
/// `\n`, `\t` and a `\r` that's followed by `\n`; `Field` keeps only `\t`. `\` and
/// `"` are never escaped. Borrowed when nothing changes, so clean text is printed
/// byte for byte.
pub fn display(bytes: &[u8], mode: Display) -> Cow<'_, [u8]> {
    // Each edit replaces bytes[from..to] with its escape.
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let (good, bad) = match std::str::from_utf8(&bytes[at..]) {
            Ok(_) => (bytes.len() - at, 0),
            Err(e) => {
                let good = e.valid_up_to();
                (good, e.error_len().unwrap_or(bytes.len() - at - good))
            }
        };
        let valid = std::str::from_utf8(&bytes[at..at + good]).expect("checked valid");
        for (i, c) in valid.char_indices() {
            let from = at + i;
            let kept = match c {
                '\n' => mode == Display::Document,
                '\r' => mode == Display::Document && bytes.get(from + 1) == Some(&b'\n'),
                c => !escaped(c),
            };
            if !kept {
                edits.push((from, from + c.len_utf8(), escape(c)));
            }
        }
        let bad_at = at + good;
        for (i, b) in bytes[bad_at..bad_at + bad].iter().enumerate() {
            edits.push((bad_at + i, bad_at + i + 1, format!("\\x{b:02x}")));
        }
        at = bad_at + bad;
    }
    if edits.is_empty() {
        return Cow::Borrowed(bytes);
    }
    let mut out = Vec::with_capacity(bytes.len() + 4 * edits.len());
    let mut start = 0;
    for (from, to, esc) in edits {
        out.extend_from_slice(&bytes[start..from]);
        out.extend_from_slice(esc.as_bytes());
        start = to;
    }
    out.extend_from_slice(&bytes[start..]);
    Cow::Owned(out)
}

/// A parsed field (a title, a body, a reason) as the file holds it.
#[derive(Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Text(Vec<u8>);

impl Text {
    /// The bytes, for a client to print or write.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// The text for a terminal ([`display`], ADR-0008).
    pub fn display(&self, mode: Display) -> Cow<'_, [u8]> {
        display(&self.0, mode)
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

    fn shown(b: &[u8], mode: Display) -> String {
        String::from_utf8(display(b, mode).into_owned()).unwrap()
    }

    /// ADR-0008 R1, R2, R5: what each mode escapes and keeps.
    #[test]
    fn display_escapes_what_a_terminal_acts_on() {
        use Display::{Document, Field};
        for (input, field, document) in [
            // C0, with the short forms, and DEL.
            (
                "Fix \u{1b}[2J login",
                "Fix \\x1b[2J login",
                "Fix \\x1b[2J login",
            ),
            (
                "\u{7}\u{8}\u{c}\u{b}\u{0}\u{1f}\u{7f}",
                "\\a\\b\\f\\v\\x00\\x1f\\x7f",
                "\\a\\b\\f\\v\\x00\\x1f\\x7f",
            ),
            // Tab is kept everywhere; newline only in Document.
            ("a\tb", "a\tb", "a\tb"),
            ("a\nb", "a\\nb", "a\nb"),
            // CRLF is kept in Document; a lone \r, also at the end, never is.
            ("a\r\nb", "a\\r\\nb", "a\r\nb"),
            ("a\rb", "a\\rb", "a\\rb"),
            ("a\r", "a\\r", "a\\r"),
            // C1 (NEL, CSI), separators, bidi controls.
            ("x\u{85}y\u{9b}z", "x\\u0085y\\u009bz", "x\\u0085y\\u009bz"),
            ("\u{2028}\u{2029}", "\\u2028\\u2029", "\\u2028\\u2029"),
            (
                "ab\u{202e}cd\u{2066}\u{200f}\u{61c}",
                "ab\\u202ecd\\u2066\\u200f\\u061c",
                "ab\\u202ecd\\u2066\\u200f\\u061c",
            ),
            // Invisible text: zero-width space, BOM, tag characters (\U form).
            ("a\u{200b}b\u{feff}", "a\\u200bb\\ufeff", "a\\u200bb\\ufeff"),
            (
                "x\u{e0041}\u{e007f}",
                "x\\U000e0041\\U000e007f",
                "x\\U000e0041\\U000e007f",
            ),
            // Left alone: backslash, quote, emoji with ZWJ, NBSP, accents, CJK.
            ("C:\\x \"q\"", "C:\\x \"q\"", "C:\\x \"q\""),
            (
                "\u{1f468}\u{200d}\u{1f469} caf\u{e9} \u{6f22}\u{a0}",
                "\u{1f468}\u{200d}\u{1f469} caf\u{e9} \u{6f22}\u{a0}",
                "\u{1f468}\u{200d}\u{1f469} caf\u{e9} \u{6f22}\u{a0}",
            ),
            ("", "", ""),
        ] {
            assert_eq!(shown(input.as_bytes(), Field), field, "Field {input:?}");
            assert_eq!(
                shown(input.as_bytes(), Document),
                document,
                "Document {input:?}"
            );
        }
    }

    /// ADR-0008 R3: invalid bytes are `\xNN`, so a raw 0x9b differs from U+009B.
    #[test]
    fn display_escapes_invalid_utf8_bytes() {
        assert_eq!(shown(b"a\x9bb", Display::Document), "a\\x9bb");
        assert_eq!(shown("a\u{9b}b".as_bytes(), Display::Document), "a\\u009bb");
        assert_eq!(shown(b"Caf\xe9\n", Display::Document), "Caf\\xe9\n");
        // A truncated sequence at the end, and one before valid text.
        assert_eq!(shown(b"x\xe6\xbc", Display::Field), "x\\xe6\\xbc");
        assert_eq!(
            shown(b"\xe6\xbcok\x1b", Display::Field),
            "\\xe6\\xbcok\\x1b"
        );
        // A \r before an invalid byte isn't part of a CRLF.
        assert_eq!(shown(b"a\r\xff", Display::Document), "a\\r\\xff");
    }

    #[test]
    fn display_borrows_clean_text() {
        for clean in [&b"plain text\twith tab\n"[..], "caf\u{e9}".as_bytes(), b""] {
            assert!(matches!(
                display(clean, Display::Document),
                Cow::Borrowed(_)
            ));
        }
        assert!(matches!(display(b"a\nb", Display::Field), Cow::Owned(_)));
        let t = Text::from(&b"T\x1b"[..]);
        assert_eq!(&*t.display(Display::Field), b"T\\x1b");
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
