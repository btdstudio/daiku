//! The stored form of output that isn't valid UTF-8 (`stdout_bytes`,
//! `stderr_bytes`): the text as it is, with `\` written `\\` and each byte that
//! isn't part of valid UTF-8 written `\xNN`. Valid UTF-8 output is stored as plain
//! `stdout`/`stderr` instead, so every case that never prints such bytes is
//! unchanged.

/// `bytes` in the escaped form.
pub fn escape(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        out.push_str(&chunk.valid().replace('\\', "\\\\"));
        for b in chunk.invalid() {
            out.push_str(&format!("\\x{b:02x}"));
        }
    }
    out
}

/// The bytes an escaped string stands for; `Err` names the first bad escape.
pub fn unescape(text: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find('\\') {
        out.extend_from_slice(&rest.as_bytes()[..i]);
        let after = &rest[i + 1..];
        if let Some(r) = after.strip_prefix('\\') {
            out.push(b'\\');
            rest = r;
        } else if let Some(hex) = after.strip_prefix('x').and_then(|h| h.get(..2))
            && let Ok(b) = u8::from_str_radix(hex, 16)
            && hex.bytes().all(|c| c.is_ascii_hexdigit())
        {
            out.push(b);
            rest = &after[3..];
        } else {
            let shown: String = rest[i..].chars().take(4).collect();
            return Err(format!("bad escape {shown:?}: use \\\\ or \\xNN"));
        }
    }
    out.extend_from_slice(rest.as_bytes());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        for bytes in [
            &b""[..],
            b"plain\n",
            b"Caf\xe9 \xff\xfe\n",
            b"back\\slash \\x41 stays",
            b"\xc3\xa9 valid, \xc3 cut",
            b"\x00\x7f\x80",
        ] {
            let e = escape(bytes);
            assert_eq!(unescape(&e).unwrap(), bytes, "{e}");
        }
        assert_eq!(escape(b"Caf\xe9\\"), "Caf\\xe9\\\\");
        assert_eq!(escape("é".as_bytes()), "é");
    }

    #[test]
    fn bad_escapes_are_refused() {
        for text in ["\\n", "\\x4", "\\xzz", "end\\", "\\u0041"] {
            assert!(unescape(text).is_err(), "{text:?}");
        }
    }
}
