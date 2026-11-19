//! Go's `encoding/json` syntax-error messages, so core reports invalid JSON exactly
//! as the Go CLI did (R7, D5).
//!
//! [`check`] is a translation of `checkValid` and the scanner state machine in Go's
//! `src/encoding/json/scanner.go` (Go 1.26.5). Callers run it before parsing with
//! serde_json; when it accepts a document, serde_json does too.
//!
//! Translated from the Go source, which carries this notice:
//!
//! Copyright 2010 The Go Authors. All rights reserved.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are met:
//!
//!    * Redistributions of source code must retain the above copyright notice,
//!      this list of conditions and the following disclaimer.
//!    * Redistributions in binary form must reproduce the above copyright notice,
//!      this list of conditions and the following disclaimer in the documentation
//!      and/or other materials provided with the distribution.
//!    * Neither the name of Google LLC nor the names of its contributors may be
//!      used to endorse or promote products derived from this software without
//!      specific prior written permission.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
//! ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
//! WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
//! DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR
//! ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
//! (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
//! LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
//! ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//! (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
//! SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

pub mod decode;

/// Go's nesting limit.
const MAX_NESTING_DEPTH: usize = 10_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    BeginValueOrEmpty,
    BeginValue,
    BeginStringOrEmpty,
    BeginString,
    EndValue,
    EndTop,
    InString,
    InStringEsc,
    InStringEscU,
    InStringEscU1,
    InStringEscU12,
    InStringEscU123,
    Neg,
    One,
    Zero,
    Dot,
    Dot0,
    E,
    ESign,
    E0,
    T,
    Tr,
    Tru,
    F,
    Fa,
    Fal,
    Fals,
    N,
    Nu,
    Nul,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Parse {
    ObjectKey,
    ObjectValue,
    ArrayValue,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scan {
    Continue,
    End,
    Error,
}

struct Scanner {
    step: State,
    end_top: bool,
    parse_state: Vec<Parse>,
    err: Option<String>,
    /// The offset of the byte being scanned.
    pos: usize,
    /// Where the top-level value ended (the offset just past it), once known.
    end_at: Option<usize>,
}

impl Scanner {
    fn new() -> Scanner {
        Scanner {
            step: State::BeginValue,
            end_top: false,
            parse_state: Vec::new(),
            err: None,
            pos: 0,
            end_at: None,
        }
    }
}

/// Go's `checkValid`: `Ok` for valid JSON, or Go's `SyntaxError` message.
pub fn check(data: &[u8]) -> Result<(), String> {
    let mut s = Scanner::new();
    for (i, &c) in data.iter().enumerate() {
        s.pos = i;
        if s.run(c) == Scan::Error {
            return Err(s.err.unwrap_or_default());
        }
    }
    s.pos = data.len();
    if s.eof() == Scan::Error {
        return Err(s.err.unwrap_or_default());
    }
    Ok(())
}

/// The length of the first JSON value in `data`, leading space included, as Go's
/// `json.Decoder` reads it; `None` when the scanner fails before the value ends or
/// the data ends inside it. One pass over the bytes.
pub fn first_value_len(data: &[u8]) -> Option<usize> {
    let mut s = Scanner::new();
    for (i, &c) in data.iter().enumerate() {
        s.pos = i;
        let result = s.run(c);
        if s.end_at.is_some() {
            return s.end_at;
        }
        if result == Scan::Error {
            return None;
        }
    }
    s.pos = data.len();
    s.eof();
    s.end_at
}

fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_hex(c: u8) -> bool {
    c.is_ascii_hexdigit()
}

impl Scanner {
    fn eof(&mut self) -> Scan {
        if self.err.is_some() {
            return Scan::Error;
        }
        if self.end_top {
            return Scan::End;
        }
        self.run(b' ');
        if self.end_top {
            return Scan::End;
        }
        if self.err.is_none() {
            self.err = Some("unexpected end of JSON input".into());
        }
        Scan::Error
    }

    fn error(&mut self, c: u8, context: &str) -> Scan {
        self.step = State::Error;
        self.err = Some(format!("invalid character {} {context}", quote_char(c)));
        Scan::Error
    }

    fn push(&mut self, c: u8, p: Parse) -> Scan {
        self.parse_state.push(p);
        if self.parse_state.len() <= MAX_NESTING_DEPTH {
            return Scan::Continue;
        }
        self.error(c, "exceeded max depth")
    }

    fn pop(&mut self) {
        self.parse_state.pop();
        if self.parse_state.is_empty() {
            self.step = State::EndTop;
            self.end_top = true;
            // The closing bracket is the value's last byte.
            self.end_at.get_or_insert(self.pos + 1);
        } else {
            self.step = State::EndValue;
        }
    }

    fn goto(&mut self, s: State) -> Scan {
        self.step = s;
        Scan::Continue
    }

    /// One byte through the current state, as `scan.step(scan, c)`.
    fn run(&mut self, c: u8) -> Scan {
        match self.step {
            State::BeginValueOrEmpty => {
                if is_space(c) {
                    return Scan::Continue;
                }
                if c == b']' {
                    return self.end_value(c);
                }
                self.begin_value(c)
            }
            State::BeginValue => self.begin_value(c),
            State::BeginStringOrEmpty => {
                if is_space(c) {
                    return Scan::Continue;
                }
                if c == b'}' {
                    if let Some(last) = self.parse_state.last_mut() {
                        *last = Parse::ObjectValue;
                    }
                    return self.end_value(c);
                }
                self.begin_string(c)
            }
            State::BeginString => self.begin_string(c),
            State::EndValue => self.end_value(c),
            State::EndTop => self.end_top_step(c),
            State::InString => match c {
                b'"' => self.goto(State::EndValue),
                b'\\' => self.goto(State::InStringEsc),
                c if c < 0x20 => self.error(c, "in string literal"),
                _ => Scan::Continue,
            },
            State::InStringEsc => match c {
                b'b' | b'f' | b'n' | b'r' | b't' | b'\\' | b'/' | b'"' => {
                    self.goto(State::InString)
                }
                b'u' => self.goto(State::InStringEscU),
                _ => self.error(c, "in string escape code"),
            },
            State::InStringEscU
            | State::InStringEscU1
            | State::InStringEscU12
            | State::InStringEscU123 => {
                if !is_hex(c) {
                    return self.error(c, "in \\u hexadecimal character escape");
                }
                let next = match self.step {
                    State::InStringEscU => State::InStringEscU1,
                    State::InStringEscU1 => State::InStringEscU12,
                    State::InStringEscU12 => State::InStringEscU123,
                    _ => State::InString,
                };
                self.goto(next)
            }
            State::Neg => match c {
                b'0' => self.goto(State::Zero),
                b'1'..=b'9' => self.goto(State::One),
                _ => self.error(c, "in numeric literal"),
            },
            State::One => {
                if c.is_ascii_digit() {
                    return self.goto(State::One);
                }
                self.zero(c)
            }
            State::Zero => self.zero(c),
            State::Dot => {
                if c.is_ascii_digit() {
                    return self.goto(State::Dot0);
                }
                self.error(c, "after decimal point in numeric literal")
            }
            State::Dot0 => match c {
                b'0'..=b'9' => Scan::Continue,
                b'e' | b'E' => self.goto(State::E),
                _ => self.end_value(c),
            },
            State::E => {
                if c == b'+' || c == b'-' {
                    return self.goto(State::ESign);
                }
                self.e_sign(c)
            }
            State::ESign => self.e_sign(c),
            State::E0 => {
                if c.is_ascii_digit() {
                    return Scan::Continue;
                }
                self.end_value(c)
            }
            State::T => self.literal(c, b'r', State::Tr, "in literal true (expecting 'r')"),
            State::Tr => self.literal(c, b'u', State::Tru, "in literal true (expecting 'u')"),
            State::Tru => self.literal(c, b'e', State::EndValue, "in literal true (expecting 'e')"),
            State::F => self.literal(c, b'a', State::Fa, "in literal false (expecting 'a')"),
            State::Fa => self.literal(c, b'l', State::Fal, "in literal false (expecting 'l')"),
            State::Fal => self.literal(c, b's', State::Fals, "in literal false (expecting 's')"),
            State::Fals => {
                self.literal(c, b'e', State::EndValue, "in literal false (expecting 'e')")
            }
            State::N => self.literal(c, b'u', State::Nu, "in literal null (expecting 'u')"),
            State::Nu => self.literal(c, b'l', State::Nul, "in literal null (expecting 'l')"),
            State::Nul => self.literal(c, b'l', State::EndValue, "in literal null (expecting 'l')"),
            State::Error => Scan::Error,
        }
    }

    fn literal(&mut self, c: u8, want: u8, next: State, context: &str) -> Scan {
        if c == want {
            return self.goto(next);
        }
        self.error(c, context)
    }

    fn begin_value(&mut self, c: u8) -> Scan {
        if is_space(c) {
            return Scan::Continue;
        }
        match c {
            b'{' => {
                self.step = State::BeginStringOrEmpty;
                self.push(c, Parse::ObjectKey)
            }
            b'[' => {
                self.step = State::BeginValueOrEmpty;
                self.push(c, Parse::ArrayValue)
            }
            b'"' => self.goto(State::InString),
            b'-' => self.goto(State::Neg),
            b'0' => self.goto(State::Zero),
            b't' => self.goto(State::T),
            b'f' => self.goto(State::F),
            b'n' => self.goto(State::N),
            b'1'..=b'9' => self.goto(State::One),
            _ => self.error(c, "looking for beginning of value"),
        }
    }

    fn begin_string(&mut self, c: u8) -> Scan {
        if is_space(c) {
            return Scan::Continue;
        }
        if c == b'"' {
            return self.goto(State::InString);
        }
        self.error(c, "looking for beginning of object key string")
    }

    fn end_value(&mut self, c: u8) -> Scan {
        let Some(&ps) = self.parse_state.last() else {
            self.step = State::EndTop;
            self.end_top = true;
            // A scalar ends just before the byte that shows it has ended.
            self.end_at.get_or_insert(self.pos);
            return self.end_top_step(c);
        };
        if is_space(c) {
            self.step = State::EndValue;
            return Scan::Continue;
        }
        let n = self.parse_state.len();
        match ps {
            Parse::ObjectKey => {
                if c == b':' {
                    self.parse_state[n - 1] = Parse::ObjectValue;
                    return self.goto(State::BeginValue);
                }
                self.error(c, "after object key")
            }
            Parse::ObjectValue => {
                if c == b',' {
                    self.parse_state[n - 1] = Parse::ObjectKey;
                    return self.goto(State::BeginString);
                }
                if c == b'}' {
                    self.pop();
                    return Scan::Continue;
                }
                self.error(c, "after object key:value pair")
            }
            Parse::ArrayValue => {
                if c == b',' {
                    return self.goto(State::BeginValue);
                }
                if c == b']' {
                    self.pop();
                    return Scan::Continue;
                }
                self.error(c, "after array element")
            }
        }
    }

    /// Go's `stateEndTop`: a non-space byte records an error but still returns
    /// `scanEnd`; the next byte (or EOF) then reports it.
    fn end_top_step(&mut self, c: u8) -> Scan {
        if !is_space(c) {
            self.error(c, "after top-level value");
        }
        Scan::End
    }

    fn zero(&mut self, c: u8) -> Scan {
        match c {
            b'.' => self.goto(State::Dot),
            b'e' | b'E' => self.goto(State::E),
            _ => self.end_value(c),
        }
    }

    fn e_sign(&mut self, c: u8) -> Scan {
        if c.is_ascii_digit() {
            return self.goto(State::E0);
        }
        self.error(c, "in exponent of numeric literal")
    }
}

/// Go's `quoteChar`: the byte as a rune in single quotes, with `strconv.Quote`'s
/// escapes.
fn quote_char(c: u8) -> String {
    match c {
        b'\'' => "'\\''".into(),
        b'"' => "'\"'".into(),
        c => {
            let q = crate::gofmt::quote(&char::from(c).to_string());
            format!("'{}'", &q[1..q.len() - 1])
        }
    }
}

/// Parses JSON as Go's `json.Unmarshal` into `any` does: Go's syntax check first,
/// then Go's decode error for the first number beyond `float64`, then serde_json.
/// Go replaces invalid UTF-8 in strings with U+FFFD rather than failing, so such
/// input is parsed from a lossy copy.
pub fn parse(data: &[u8]) -> Result<serde_json::Value, String> {
    check(data)?;
    if let Some(literal) = first_overflow(data) {
        return Err(format!(
            "json: cannot unmarshal number {literal} into Go value of type float64"
        ));
    }
    match serde_json::from_slice(data) {
        Ok(v) => Ok(v),
        Err(_) => {
            serde_json::from_str(&String::from_utf8_lossy(data)).map_err(|e| format!("json: {e}"))
        }
    }
}

/// The first number literal outside strings that overflows a `float64` (as Go's
/// `strconv.ParseFloat` reports `ErrRange` for it). Underflow rounds to zero
/// without an error, as in Go. `data` has passed [`check`].
fn first_overflow(data: &[u8]) -> Option<String> {
    let mut i = 0;
    while i < data.len() {
        match data[i] {
            b'"' => {
                i += 1;
                while i < data.len() && data[i] != b'"' {
                    i += if data[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
            }
            b'-' | b'0'..=b'9' => {
                let start = i;
                while i < data.len()
                    && matches!(data[i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
                {
                    i += 1;
                }
                let literal = std::str::from_utf8(&data[start..i]).unwrap_or_default();
                if literal.parse::<f64>().is_ok_and(f64::is_infinite) {
                    return Some(literal.to_string());
                }
            }
            _ => i += 1,
        }
    }
    None
}
