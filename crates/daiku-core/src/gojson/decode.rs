//! A pull reader for decoding JSON into Go-shaped types the way Go's
//! `json.Unmarshal` does: string unquoting with Go's replacement rules, raw number
//! literals (Go parses them per target type), and the kind names Go's type errors
//! use. Input must have passed [`super::check`] first; the reader then assumes
//! valid JSON and never fails on syntax.

/// What the next value is, by its first byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Object,
    Array,
    String,
    Number,
    Bool,
    Null,
}

impl Kind {
    /// The name Go's `UnmarshalTypeError` gives this kind of value.
    pub fn go_name(self) -> &'static str {
        match self {
            Kind::Object => "object",
            Kind::Array => "array",
            Kind::String => "string",
            Kind::Number => "number",
            Kind::Bool => "bool",
            Kind::Null => "null",
        }
    }
}

/// A cursor over valid JSON.
pub struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Reader<'a> {
        Reader { data, pos: 0 }
    }

    fn ws(&mut self) {
        while self
            .data
            .get(self.pos)
            .is_some_and(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n'))
        {
            self.pos += 1;
        }
    }

    /// The kind of the next value.
    pub fn kind(&mut self) -> Kind {
        self.ws();
        match self.data.get(self.pos) {
            Some(b'{') => Kind::Object,
            Some(b'[') => Kind::Array,
            Some(b'"') => Kind::String,
            Some(b't' | b'f') => Kind::Bool,
            Some(b'n') => Kind::Null,
            _ => Kind::Number,
        }
    }

    /// Consumes a number, `true`, `false` or `null` and returns its text.
    pub fn literal(&mut self) -> &'a str {
        self.ws();
        let start = self.pos;
        while self
            .data
            .get(self.pos)
            .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'+' | b'.'))
        {
            self.pos += 1;
        }
        std::str::from_utf8(&self.data[start..self.pos]).unwrap_or_default()
    }

    /// Consumes a string and returns it unquoted as Go's `unquote` does: a byte that
    /// isn't valid UTF-8 becomes U+FFFD, and so does a `\u` surrogate that isn't
    /// half of a valid pair.
    pub fn string(&mut self) -> String {
        self.ws();
        self.pos += 1; // the opening quote
        let mut out = String::new();
        loop {
            let c = self.data[self.pos];
            match c {
                b'"' => {
                    self.pos += 1;
                    return out;
                }
                b'\\' => {
                    let e = self.data[self.pos + 1];
                    self.pos += 2;
                    match e {
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => out.push(self.unicode_escape()),
                        other => out.push(char::from(other)),
                    }
                }
                c if c < 0x80 => {
                    out.push(char::from(c));
                    self.pos += 1;
                }
                _ => {
                    let (ch, n) = decode_rune(&self.data[self.pos..]);
                    out.push(ch);
                    self.pos += n;
                }
            }
        }
    }

    /// The rune of a `\u` escape whose `\u` has been consumed.
    fn unicode_escape(&mut self) -> char {
        let first = self.hex4(self.pos);
        self.pos += 4;
        if (0xD800..0xE000).contains(&first) {
            // Go's utf16.DecodeRune on this and a following \u escape.
            if first < 0xDC00
                && self.data.get(self.pos) == Some(&b'\\')
                && self.data.get(self.pos + 1) == Some(&b'u')
            {
                let second = self.hex4(self.pos + 2);
                if (0xDC00..0xE000).contains(&second) {
                    self.pos += 6;
                    let c = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                    return char::from_u32(c).unwrap_or('\u{FFFD}');
                }
            }
            return '\u{FFFD}';
        }
        char::from_u32(first).unwrap_or('\u{FFFD}')
    }

    fn hex4(&self, at: usize) -> u32 {
        let s = std::str::from_utf8(&self.data[at..at + 4]).unwrap_or("0000");
        u32::from_str_radix(s, 16).unwrap_or(0)
    }

    /// Skips the next value, however deeply nested, without recursion.
    pub fn skip(&mut self) {
        let mut depth = 0usize;
        loop {
            self.ws();
            match self.data.get(self.pos) {
                None => return,
                Some(b'{' | b'[') => {
                    depth += 1;
                    self.pos += 1;
                }
                Some(b'}' | b']') => {
                    depth = depth.saturating_sub(1);
                    self.pos += 1;
                    if depth == 0 {
                        return;
                    }
                }
                Some(b',' | b':') => self.pos += 1,
                Some(b'"') => {
                    self.pos += 1;
                    while self.data[self.pos] != b'"' {
                        self.pos += if self.data[self.pos] == b'\\' { 2 } else { 1 };
                    }
                    self.pos += 1;
                    if depth == 0 {
                        return;
                    }
                }
                Some(_) => {
                    self.literal();
                    if depth == 0 {
                        return;
                    }
                }
            }
        }
    }

    /// Consumes `{` or `[`.
    pub fn open(&mut self) {
        self.ws();
        self.pos += 1;
    }

    /// In an object: the next key (its `:` consumed), or `None` at the closing
    /// brace, which is consumed.
    pub fn next_key(&mut self) -> Option<String> {
        self.ws();
        if self.data.get(self.pos) == Some(&b',') {
            self.pos += 1;
            self.ws();
        }
        if self.data.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return None;
        }
        let key = self.string();
        self.ws();
        self.pos += 1; // ':'
        Some(key)
    }

    /// In an array: whether another element follows; `false` consumes the
    /// closing bracket.
    pub fn next_item(&mut self) -> bool {
        self.ws();
        if self.data.get(self.pos) == Some(&b',') {
            self.pos += 1;
            self.ws();
        }
        if self.data.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return false;
        }
        true
    }
}

/// Go's `utf8.DecodeRune`: the rune at the start of `b` and its length, or U+FFFD
/// and 1 when the bytes aren't valid UTF-8 there.
fn decode_rune(b: &[u8]) -> (char, usize) {
    let n = match b[0] {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return ('\u{FFFD}', 1),
    };
    match b.get(..n).and_then(|s| std::str::from_utf8(s).ok()) {
        Some(s) => (s.chars().next().unwrap_or('\u{FFFD}'), n),
        None => ('\u{FFFD}', 1),
    }
}

/// Go's `foldName` for matching a JSON key to an ASCII field name: ASCII letters
/// fold to upper case, and the two non-ASCII runes whose fold set holds an ASCII
/// letter (ſ U+017F with S, K U+212A with K) fold to that letter. Any other rune
/// can't match an ASCII name, so it's kept.
pub fn fold(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\u{17F}' => 'S',
            '\u{212A}' => 'K',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquotes_as_go() {
        let mut r = Reader::new(b"\"a\\u00e9\\ud83d\\ude00\\udc00\\ud800x\xff\xe2\x82b\"");
        assert_eq!(
            r.string(),
            "a\u{e9}\u{1F600}\u{FFFD}\u{FFFD}x\u{FFFD}\u{FFFD}\u{FFFD}b"
        );
    }

    #[test]
    fn skips_nested_values() {
        let mut r = Reader::new(br#"{"a": [1, {"b": "]}"}], "c": 2}"#);
        r.open();
        assert_eq!(r.next_key().as_deref(), Some("a"));
        r.skip();
        assert_eq!(r.next_key().as_deref(), Some("c"));
        assert_eq!(r.literal(), "2");
        assert_eq!(r.next_key(), None);
    }

    #[test]
    fn folds_like_go() {
        assert_eq!(fold("ſtatus"), fold("STATUS"));
        assert_eq!(fold("\u{212A}"), fold("k"));
        assert_ne!(fold("é"), fold("e"));
    }
}
