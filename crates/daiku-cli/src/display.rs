//! The terminal side of ADR-0008: stdout and stderr pass through
//! `daiku_core::text::display` (Document mode) on their way out, so every text
//! command's output shows control characters and invalid UTF-8 as visible escapes.
//! `--json` is written before this wrap exists, so it's untouched.

use std::io::{self, Write};

use daiku_core::text::{Display, display};

/// A writer that applies ADR-0008's Document rule to everything written through it.
/// It holds back at most a trailing `\r` (until it knows whether `\n` follows) or an
/// incomplete UTF-8 prefix (until the rest arrives), so output is never delayed by
/// more than one character.
pub struct DisplayWriter<W: Write> {
    inner: W,
    tail: Vec<u8>,
}

impl<W: Write> DisplayWriter<W> {
    pub fn new(inner: W) -> Self {
        DisplayWriter {
            inner,
            tail: Vec::new(),
        }
    }

    /// Writes `data`, judged as complete, to the inner writer.
    fn emit(&mut self, data: &[u8]) -> io::Result<()> {
        self.inner.write_all(&display(data, Display::Document))
    }
}

/// A free-text single-line field (a remote, a branch, a local path) for printing:
/// ADR-0008's Field rule, so a line break in it shows as `\n` instead of ending the
/// line.
pub fn field(s: &str) -> String {
    String::from_utf8(display(s.as_bytes(), Display::Field).into_owned())
        .expect("escaping valid UTF-8 gives valid UTF-8")
}

/// How many trailing bytes of `data` to hold back: a final `\r`, or the start of a
/// UTF-8 sequence that needs more bytes than are there.
fn held(data: &[u8]) -> usize {
    if data.last() == Some(&b'\r') {
        return 1;
    }
    for k in 1..=3.min(data.len()) {
        let lead = data[data.len() - k];
        let need = match lead {
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            0x80..=0xBF => continue,
            _ => return 0,
        };
        let rest_ok = data[data.len() - k + 1..]
            .iter()
            .all(|b| (0x80..=0xBF).contains(b));
        return if need > k && rest_ok { k } else { 0 };
    }
    0
}

impl<W: Write> Write for DisplayWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut data = std::mem::take(&mut self.tail);
        data.extend_from_slice(buf);
        let keep = held(&data);
        let split = data.len() - keep;
        self.emit(&data[..split])?;
        self.tail = data[split..].to_vec();
        Ok(buf.len())
    }

    /// Emits any held tail as final (a lone `\r`, or an incomplete sequence), then
    /// flushes.
    fn flush(&mut self) -> io::Result<()> {
        let tail = std::mem::take(&mut self.tail);
        if !tail.is_empty() {
            self.emit(&tail)?;
        }
        self.inner.flush()
    }
}

impl<W: Write> Drop for DisplayWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn through(writes: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut w = DisplayWriter::new(&mut out);
            for chunk in writes {
                assert_eq!(w.write(chunk).unwrap(), chunk.len());
            }
        }
        out
    }

    #[test]
    fn field_escapes_line_breaks_too() {
        assert_eq!(field("/srv/a\nb"), "/srv/a\\nb");
        assert_eq!(field("https://h/x\u{202e}"), "https://h/x\\u202e");
        assert_eq!(field("C:\\src\tapi"), "C:\\src\tapi");
    }

    #[test]
    fn clean_text_passes_through() {
        assert_eq!(
            through(&[b"S-0001  Fix login\n", b"\tok\n"]),
            b"S-0001  Fix login\n\tok\n"
        );
    }

    #[test]
    fn escapes_what_a_terminal_acts_on() {
        assert_eq!(through(&[b"Fix \x1b[2J login\n"]), b"Fix \\x1b[2J login\n");
        assert_eq!(through(&[b"a\x9bb\n"]), b"a\\x9bb\n");
    }

    #[test]
    fn a_char_split_across_writes_is_judged_whole() {
        // U+202E (e2 80 ae) in three writes, and U+00E9 (c3 a9) in two.
        assert_eq!(through(&[b"x\xe2", b"\x80", b"\xae y"]), b"x\\u202e y");
        assert_eq!(through(&[b"caf\xc3", b"\xa9\n"]), "café\n".as_bytes());
    }

    #[test]
    fn crlf_split_across_writes_is_kept() {
        assert_eq!(through(&[b"line\r", b"\nnext"]), b"line\r\nnext");
        assert_eq!(through(&[b"a\r", b"b"]), b"a\\rb");
    }

    #[test]
    fn the_tail_is_emitted_as_final() {
        assert_eq!(through(&[b"end\r"]), b"end\\r");
        assert_eq!(through(&[b"cut\xe6\xbc"]), b"cut\\xe6\\xbc");
        let mut out = Vec::new();
        let mut w = DisplayWriter::new(&mut out);
        w.write_all(b"x\r").unwrap();
        w.flush().unwrap();
        drop(w);
        assert_eq!(out, b"x\\r");
    }
}
