//! Go's text and path helpers, ported so core's messages and paths read exactly as
//! the Go CLI's do (D4).
//!
//! - [`quote`] is `strconv.Quote` (Go's `%q`).
//! - [`clean`] and [`join`] are Go's `path.Clean` and `path.Join`: slash-separated,
//!   which is how daiku prints paths on every OS (architecture D13).
//! - [`is_local`] is `filepath.IsLocal` for the OS the binary was built for.

/// `strconv.Quote`: a double-quoted string with Go's escapes.
pub fn quote(s: &str) -> String {
    quote_bytes(s.as_bytes())
}

/// [`quote`] for bytes that may not be UTF-8 (each invalid byte becomes `\xNN`).
pub fn quote_bytes(mut b: &[u8]) -> String {
    let mut out = String::from("\"");
    while !b.is_empty() {
        let (valid, rest) = match std::str::from_utf8(b) {
            Ok(s) => (s, &b[b.len()..]),
            Err(e) => {
                let (ok, bad) = b.split_at(e.valid_up_to());
                // SAFETY of unwrap: the prefix up to valid_up_to is valid UTF-8.
                (std::str::from_utf8(ok).unwrap_or_default(), bad)
            }
        };
        for c in valid.chars() {
            escape_char(&mut out, c);
        }
        if let Some((&bad, more)) = rest.split_first() {
            out.push_str(&format!("\\x{bad:02x}"));
            b = more;
        } else {
            b = rest;
        }
    }
    out.push('"');
    out
}

fn escape_char(out: &mut String, c: char) {
    match c {
        '"' => out.push_str("\\\""),
        '\\' => out.push_str("\\\\"),
        c if is_print(c) => out.push(c),
        '\u{7}' => out.push_str("\\a"),
        '\u{8}' => out.push_str("\\b"),
        '\u{c}' => out.push_str("\\f"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{b}' => out.push_str("\\v"),
        c if (c as u32) < 0x20 || c as u32 == 0x7f => out.push_str(&format!("\\x{:02x}", c as u32)),
        c if (c as u32) < 0x10000 => out.push_str(&format!("\\u{:04x}", c as u32)),
        c => out.push_str(&format!("\\U{:08x}", c as u32)),
    }
}

/// Go's `unicode.IsPrint`, approximated without Unicode tables: everything is
/// printable except control characters, spaces other than U+0020, line and
/// paragraph separators, common format characters, private use and
/// noncharacters. Unassigned code points (which Go also escapes) aren't detected.
fn is_print(c: char) -> bool {
    let u = c as u32;
    if c == ' ' {
        return true;
    }
    if c.is_control() {
        return false;
    }
    let not_print = matches!(u,
        0x00A0 | 0x00AD | 0x1680 | 0x180E | 0x2000..=0x200F | 0x2028..=0x202F
        | 0x205F..=0x206F | 0x3000 | 0xFEFF | 0xFFF9..=0xFFFB
        | 0x0600..=0x0605 | 0x061C | 0x06DD | 0x070F
        | 0xE000..=0xF8FF | 0xF0000..=0x10FFFF | 0xFDD0..=0xFDEF)
        || u & 0xFFFE == 0xFFFE;
    !not_print
}

/// Go's `path.Clean`: the shortest equivalent slash path.
pub fn clean(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }
    let rooted = path.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            p => parts.push(p),
        }
    }
    let joined = parts.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".into(),
        (false, false) => joined,
    }
}

/// Go's `path.Join`: the non-empty elements joined with `/`, then cleaned; empty
/// when every element is empty.
pub fn join<S: AsRef<str>>(elems: &[S]) -> String {
    let parts: Vec<&str> = elems
        .iter()
        .map(AsRef::as_ref)
        .filter(|e| !e.is_empty())
        .collect();
    if parts.is_empty() {
        return String::new();
    }
    clean(&parts.join("/"))
}

/// Backslashes to slashes on Windows, as Go's `filepath.ToSlash`; unchanged
/// elsewhere.
pub fn to_slash(path: &str) -> String {
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.to_string()
    }
}

/// Go's `filepath.IsLocal` for this OS: relative, not empty, never climbing out with
/// `..`, and on Windows no drive, root, colon or reserved device name.
pub fn is_local(path: &str) -> bool {
    if cfg!(windows) {
        windows_is_local(path)
    } else {
        unix_is_local(path)
    }
}

/// `filepath.IsLocal` on Unix.
pub fn unix_is_local(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') {
        return false;
    }
    let path = if path.split('/').any(|p| p == "." || p == "..") {
        clean(path)
    } else {
        path.to_string()
    };
    !(path == ".." || path.starts_with("../"))
}

/// `filepath.IsLocal` on Windows.
pub fn windows_is_local(path: &str) -> bool {
    if path.is_empty() || path.starts_with(['/', '\\']) || path.contains(':') {
        return false;
    }
    let mut has_dots = false;
    for part in path.split(['/', '\\']) {
        if part == "." || part == ".." {
            has_dots = true;
        }
        if is_reserved_name(part) {
            return false;
        }
    }
    let path = path.replace('\\', "/");
    let path = if has_dots { clean(&path) } else { path };
    !(path == ".." || path.starts_with("../"))
}

/// A Windows device name (`NUL`, `COM1`, `CONIN$`…), allowing trailing spaces and a
/// suffix after `.` or `:`. Go asks Windows whether a name with an extension is
/// still a device (`RtlIsDosDeviceName_U`; since Windows 11, `CON.txt` isn't); this
/// port treats the name as a device when what follows is only dots and spaces, or
/// starts with `:`.
fn is_reserved_name(name: &str) -> bool {
    let cut = name.find([':', '.']).unwrap_or(name.len());
    let base = name[..cut].trim_end_matches(' ');
    if !is_reserved_base_name(base) {
        return false;
    }
    let rest = &name[base.len()..];
    rest.is_empty()
        || rest.starts_with(':')
        || rest.chars().all(|c| c == '.' || c == ' ')
        || rest.trim_start_matches(' ').starts_with(':')
}

fn is_reserved_base_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) {
        return true;
    }
    let (head, tail) = (upper.get(..3), upper.get(3..));
    if matches!(head, Some("COM") | Some("LPT")) {
        return matches!(
            tail,
            Some(
                "1" | "2"
                    | "3"
                    | "4"
                    | "5"
                    | "6"
                    | "7"
                    | "8"
                    | "9"
                    | "\u{b9}"
                    | "\u{b2}"
                    | "\u{b3}"
            )
        );
    }
    false
}

/// A JSON value as Go's `%v` prints the `any` that `encoding/json` decodes it to:
/// `true`, `3`, `1.5`, `[a b]`, `map[k:v]` (keys sorted), `<nil>`.
pub fn value_v(v: &serde_json::Value) -> String {
    use serde_json::Value;
    match v {
        Value::Null => "<nil>".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_f64() {
            Some(f) => float_v(f),
            None => n.to_string(),
        },
        Value::String(s) => s.clone(),
        Value::Array(a) => format!("[{}]", a.iter().map(value_v).collect::<Vec<_>>().join(" ")),
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| format!("{k}:{}", value_v(&m[*k])))
                .collect();
            format!("map[{}]", parts.join(" "))
        }
    }
}

/// A `float64` as Go's `%v` prints it: `%g` with the shortest digits, which uses
/// exponent form (`1e+06`, `1.5e-05`) when the decimal exponent is below -4 or at
/// least 6 (strconv's precision 6 for the shortest form), and plain digits otherwise.
pub fn float_v(f: f64) -> String {
    if f.is_infinite() {
        return if f > 0.0 {
            "+Inf".into()
        } else {
            "-Inf".into()
        };
    }
    if f.is_nan() {
        return "NaN".into();
    }
    let e = format!("{f:e}");
    let (mantissa, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    if f != 0.0 && !(-4..6).contains(&exp) {
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mantissa}e{sign}{:02}", exp.abs());
    }
    f.to_string()
}

/// An I/O error's text as Go's `syscall.Errno` words it, for the common cases; other
/// errors keep the OS text without Rust's ` (os error N)` suffix.
pub fn io_error_text(e: &std::io::Error) -> String {
    use std::io::ErrorKind as K;
    let known = match e.kind() {
        K::NotFound => "no such file or directory",
        K::PermissionDenied => "permission denied",
        K::AlreadyExists => "file exists",
        K::IsADirectory => "is a directory",
        K::NotADirectory => "not a directory",
        K::DirectoryNotEmpty => "directory not empty",
        K::ReadOnlyFilesystem => "read-only file system",
        _ => "",
    };
    if !known.is_empty() && e.raw_os_error().is_some() {
        return known.to_string();
    }
    let text = e.to_string();
    let text = match text.find(" (os error ") {
        Some(i) => &text[..i],
        None => &text,
    };
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // strconv/quote_test.go, quotetests (Quote column).
    #[test]
    fn quote_matches_go() {
        for (input, want) in [
            ("\u{7}\u{8}\u{c}\r\n\t\u{b}", r#""\a\b\f\r\n\t\v""#),
            ("\\", r#""\\""#),
            ("\u{263a}", "\"☺\""),
            ("\u{10ffff}", r#""\U0010ffff""#),
            ("\u{4}", r#""\x04""#),
            ("!\u{a0}!\u{2000}!\u{3000}!", r#""!\u00a0!\u2000!\u3000!""#),
            ("\u{7f}", r#""\x7f""#),
            ("sometimes", r#""sometimes""#),
            ("a\"b", r#""a\"b""#),
            ("é — ✗", "\"é — ✗\""),
        ] {
            assert_eq!(quote(input), want, "{input:?}");
        }
        assert_eq!(quote_bytes(b"abc\xffdef"), r#""abc\xffdef""#);
    }

    // path/filepath/path_test.go, cleantests and nonwincleantests.
    #[test]
    fn clean_matches_go() {
        for (input, want) in [
            ("abc", "abc"),
            ("abc/def", "abc/def"),
            ("a/b/c", "a/b/c"),
            (".", "."),
            ("..", ".."),
            ("../..", "../.."),
            ("../../abc", "../../abc"),
            ("/abc", "/abc"),
            ("/", "/"),
            ("", "."),
            ("abc/", "abc"),
            ("abc/def/", "abc/def"),
            ("a/b/c/", "a/b/c"),
            ("./", "."),
            ("../", ".."),
            ("../../", "../.."),
            ("/abc/", "/abc"),
            ("abc//def//ghi", "abc/def/ghi"),
            ("abc//", "abc"),
            ("abc/./def", "abc/def"),
            ("/./abc/def", "/abc/def"),
            ("abc/.", "abc"),
            ("abc/def/ghi/../jkl", "abc/def/jkl"),
            ("abc/def/../ghi/../jkl", "abc/jkl"),
            ("abc/def/..", "abc"),
            ("abc/def/../..", "."),
            ("/abc/def/../..", "/"),
            ("abc/def/../../..", ".."),
            ("/abc/def/../../..", "/"),
            ("abc/def/../../../ghi/jkl/../../../mno", "../../mno"),
            ("/../abc", "/abc"),
            ("a/../b:/../../c", "../c"),
            ("abc/./../def", "def"),
            ("abc//./../def", "def"),
            ("abc/../../././../def", "../../def"),
            ("//abc", "/abc"),
            ("///abc", "/abc"),
            ("//abc//", "/abc"),
        ] {
            assert_eq!(clean(input), want, "{input:?}");
        }
    }

    // jointests and nonwinjointests.
    #[test]
    fn join_matches_go() {
        let cases: [(&[&str], &str); 18] = [
            (&[], ""),
            (&[""], ""),
            (&["/"], "/"),
            (&["a"], "a"),
            (&["a", "b"], "a/b"),
            (&["a", ""], "a"),
            (&["", "b"], "b"),
            (&["/", "a"], "/a"),
            (&["/", "a/b"], "/a/b"),
            (&["/", ""], "/"),
            (&["/a", "b"], "/a/b"),
            (&["a", "/b"], "a/b"),
            (&["/a", "/b"], "/a/b"),
            (&["a/", "b"], "a/b"),
            (&["a/", ""], "a"),
            (&["", ""], ""),
            (&["/", "a", "b"], "/a/b"),
            (&["//", "a"], "/a"),
        ];
        for (input, want) in cases {
            assert_eq!(join(input), want, "{input:?}");
        }
        assert_eq!(join(&[".", ".daiku/settings.json"]), ".daiku/settings.json");
        assert_eq!(
            join(&[".", "plan", "04_architecture/adr"]),
            "plan/04_architecture/adr"
        );
    }

    #[test]
    fn values_print_like_go() {
        let v = |s: &str| value_v(&serde_json::from_str(s).unwrap());
        assert_eq!(v("true"), "true");
        assert_eq!(v("3"), "3");
        assert_eq!(v("1.5"), "1.5");
        assert_eq!(v("1e21"), "1e+21");
        // Checked against Go's `daiku settings` output (code review finding 1).
        for (n, go) in [
            ("123456789", "1.23456789e+08"),
            ("1000000", "1e+06"),
            ("1e6", "1e+06"),
            ("1e20", "1e+20"),
            ("-2.5e7", "-2.5e+07"),
            ("100000", "100000"),
            ("999999", "999999"),
            ("12345.678", "12345.678"),
            ("0.1", "0.1"),
            ("1e-5", "1e-05"),
            ("0.00001234", "1.234e-05"),
            ("1.5e300", "1.5e+300"),
            ("0.0001", "0.0001"),
            ("0", "0"),
            ("-0", "-0"),
            ("18446744073709551615", "1.8446744073709552e+19"),
        ] {
            assert_eq!(v(n), go, "{n}");
        }
        assert_eq!(v("null"), "<nil>");
        assert_eq!(v(r#"[1,"a"]"#), "[1 a]");
        assert_eq!(v(r#"{"b":1,"a":true}"#), "map[a:true b:1]");
    }

    #[test]
    fn io_errors_read_like_go() {
        let e = std::fs::read("/no/such/daiku/file").unwrap_err();
        assert_eq!(io_error_text(&e), "no such file or directory");
        let e = std::io::Error::other("Something odd");
        assert_eq!(io_error_text(&e), "something odd");
    }

    // islocaltests.
    #[test]
    fn unix_is_local_matches_go() {
        for (input, want) in [
            ("", false),
            (".", true),
            ("..", false),
            ("../a", false),
            ("/", false),
            ("/a", false),
            ("/a/../..", false),
            ("a", true),
            ("a/../a", true),
            ("a/", true),
            ("a/.", true),
            ("a/./b/./c", true),
            ("a/../b:/../../c", false),
        ] {
            assert_eq!(unix_is_local(input), want, "{input:?}");
        }
    }

    // islocaltests and winislocaltests, with Windows rules.
    #[test]
    fn windows_is_local_matches_go() {
        for (input, want) in [
            ("", false),
            (".", true),
            ("..", false),
            ("../a", false),
            ("/", false),
            ("/a", false),
            ("/a/../..", false),
            ("a", true),
            ("a/../a", true),
            ("a/", true),
            ("a/.", true),
            ("a/./b/./c", true),
            ("a/../b:/../../c", false),
            ("NUL", false),
            ("nul", false),
            ("nul ", false),
            ("nul.", false),
            ("a/nul:", false),
            ("a/nul : a", false),
            ("com0", true),
            ("com1", false),
            ("com2", false),
            ("com3", false),
            ("com4", false),
            ("com5", false),
            ("com6", false),
            ("com7", false),
            ("com8", false),
            ("com9", false),
            ("com¹", false),
            ("com²", false),
            ("com³", false),
            ("com¹ : a", false),
            ("cOm1", false),
            ("lpt1", false),
            ("LPT1", false),
            ("lpt³", false),
            ("./nul", false),
            ("\\", false),
            ("\\a", false),
            ("C:", false),
            ("C:\\a", false),
            ("..\\a", false),
            ("a/../c:", false),
            ("CONIN$", false),
            ("conin$", false),
            ("CONOUT$", false),
            ("conout$", false),
            ("dollar$", true),
        ] {
            assert_eq!(windows_is_local(input), want, "{input:?}");
        }
    }
}
