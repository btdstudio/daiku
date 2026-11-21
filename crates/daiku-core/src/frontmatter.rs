//! The frontmatter subset: scalars, inline lists (`[a, b]`) and one level of nested
//! map at the top of a markdown file (D9). A port of Go's
//! `internal/core/frontmatter`: it refuses the same YAML constructs with the same
//! line-numbered messages, and fields that aren't changed keep their exact text,
//! comments included.
//!
//! As in Go, the frontmatter's lines are written back with the line ending of the
//! opening `---`; the body after the closing `---` is kept byte for byte.

use std::collections::BTreeMap;

use crate::{Error, gofmt};

const DELIMITER: &str = "---";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Scalar,
    List,
    Nested,
}

#[derive(Debug, Clone)]
struct Field {
    key: String,
    shape: Shape,
    value: String,
    items: Vec<String>,
    subs: Vec<(String, String)>,
    /// The field's source lines until it's changed.
    raw: Option<Vec<String>>,
    /// Blank and comment lines kept ahead of this field.
    before: Vec<String>,
}

/// A parsed file: its fields in order, and its body.
#[derive(Debug, Clone)]
pub struct Document {
    fields: Vec<Field>,
    trailing: Vec<String>,
    eol: &'static str,
    /// Everything after the closing delimiter, byte for byte.
    pub body: String,
}

/// The shape of a top-level field's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `key: value`
    Scalar,
    /// `key: [a, b]`
    List,
    /// One level of nested keys, indented under the key.
    Map,
}

/// A read-only view of one top-level field, with quotes, escapes and comments
/// removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldView {
    pub key: String,
    pub kind: Kind,
    pub value: String,
    pub items: Vec<String>,
    pub subs: BTreeMap<String, String>,
}

fn err(message: impl Into<String>) -> Error {
    Error::Frontmatter {
        message: message.into(),
    }
}

fn line_err(line_no: usize, what: impl std::fmt::Display) -> Error {
    err(format!("frontmatter: line {line_no}: {what}"))
}

/// Go's `keyRe`: `^[A-Za-z_][A-Za-z0-9_-]*$`.
fn key_ok(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Go's `plainRe`: `^[A-Za-z0-9_./@+-][A-Za-z0-9_./@+ -]*$`.
fn plain_ok(value: &str) -> bool {
    let first = |c: char| c.is_ascii_alphanumeric() || "_./@+-".contains(c);
    let mut chars = value.chars();
    matches!(chars.next(), Some(c) if first(c)) && chars.all(|c| first(c) || c == ' ')
}

fn is_ignorable(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

impl Document {
    /// An empty document with the given body.
    pub fn new(body: impl Into<String>) -> Document {
        Document {
            fields: Vec::new(),
            trailing: Vec::new(),
            eol: "\n",
            body: body.into(),
        }
    }

    /// Splits frontmatter from the body. A file without an opening delimiter is an
    /// error, so a missing or mangled header can't pass for an empty one.
    pub fn parse(text: &str) -> Result<Document, Error> {
        let eol = if text.starts_with("---\r\n") {
            "\r\n"
        } else if text.starts_with("---\n") {
            "\n"
        } else {
            return Err(err(format!(
                "frontmatter: file must start with {}",
                gofmt::quote(DELIMITER)
            )));
        };
        let mut rest = &text[DELIMITER.len() + eol.len()..];
        let mut lines: Vec<String> = Vec::new();
        loop {
            let Some(i) = rest.find('\n') else {
                if rest.trim_end_matches('\r') == DELIMITER {
                    return parse_lines(lines, String::new(), eol);
                }
                return Err(err(format!(
                    "frontmatter: no closing {}",
                    gofmt::quote(DELIMITER)
                )));
            };
            let line = rest[..i].strip_suffix('\r').unwrap_or(&rest[..i]);
            let after = &rest[i + 1..];
            if line == DELIMITER {
                return parse_lines(lines, after.to_string(), eol);
            }
            lines.push(line.to_string());
            rest = after;
        }
    }

    fn find(&self, key: &str) -> Option<&Field> {
        self.fields.iter().find(|f| f.key == key)
    }

    /// Whether a key is present.
    pub fn has(&self, key: &str) -> bool {
        self.find(key).is_some()
    }

    /// The top-level keys in order.
    pub fn keys(&self) -> Vec<String> {
        self.fields.iter().map(|f| f.key.clone()).collect()
    }

    /// A scalar's value; empty when absent or not a scalar.
    pub fn get(&self, key: &str) -> String {
        match self.find(key) {
            Some(f) if f.shape == Shape::Scalar => f.value.clone(),
            _ => String::new(),
        }
    }

    /// A list's items; `None` when absent or not a list.
    pub fn list(&self, key: &str) -> Option<Vec<String>> {
        match self.find(key) {
            Some(f) if f.shape == Shape::List => Some(f.items.clone()),
            _ => None,
        }
    }

    /// A nested map's values; `None` when absent or not a map.
    pub fn map(&self, key: &str) -> Option<BTreeMap<String, String>> {
        match self.find(key) {
            Some(f) if f.shape == Shape::Nested => Some(f.subs.iter().cloned().collect()),
            _ => None,
        }
    }

    /// The top-level fields in file order. Changing the result doesn't change the
    /// document.
    pub fn fields(&self) -> Vec<FieldView> {
        self.fields
            .iter()
            .map(|f| FieldView {
                key: f.key.clone(),
                kind: match f.shape {
                    Shape::Scalar => Kind::Scalar,
                    Shape::List => Kind::List,
                    Shape::Nested => Kind::Map,
                },
                value: if f.shape == Shape::Scalar {
                    f.value.clone()
                } else {
                    String::new()
                },
                items: if f.shape == Shape::List {
                    f.items.clone()
                } else {
                    Vec::new()
                },
                subs: if f.shape == Shape::Nested {
                    f.subs.iter().cloned().collect()
                } else {
                    BTreeMap::new()
                },
            })
            .collect()
    }

    /// The field for a key, cleared and ready to overwrite, added at the end if new.
    fn slot(&mut self, key: &str) -> Result<&mut Field, Error> {
        if !key_ok(key) {
            return Err(err(format!(
                "frontmatter: invalid key {}",
                gofmt::quote(key)
            )));
        }
        let i = match self.fields.iter().position(|f| f.key == key) {
            Some(i) => i,
            None => {
                self.fields.push(Field {
                    key: key.to_string(),
                    shape: Shape::Scalar,
                    value: String::new(),
                    items: Vec::new(),
                    subs: Vec::new(),
                    raw: None,
                    before: Vec::new(),
                });
                self.fields.len() - 1
            }
        };
        let f = &mut self.fields[i];
        f.raw = None;
        f.value.clear();
        f.items.clear();
        f.subs.clear();
        Ok(f)
    }

    /// Writes a scalar, replacing any existing value.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Error> {
        check_value(value)?;
        let f = self.slot(key)?;
        f.shape = Shape::Scalar;
        f.value = value.to_string();
        Ok(())
    }

    /// Writes an inline list, replacing any existing value.
    pub fn set_list(&mut self, key: &str, items: &[String]) -> Result<(), Error> {
        for item in items {
            check_value(item)?;
        }
        let f = self.slot(key)?;
        f.shape = Shape::List;
        f.items = items.to_vec();
        Ok(())
    }

    /// Writes one key of a nested map, creating the map if needed and keeping its
    /// other keys.
    pub fn set_sub(&mut self, key: &str, sub: &str, value: &str) -> Result<(), Error> {
        if !key_ok(sub) {
            return Err(err(format!(
                "frontmatter: invalid key {}",
                gofmt::quote(sub)
            )));
        }
        if value.is_empty() {
            return Err(err(format!("frontmatter: {key}.{sub} can't be empty")));
        }
        check_value(value)?;
        let subs = match self.find(key) {
            Some(f) if f.shape == Shape::Nested => f.subs.clone(),
            _ => Vec::new(),
        };
        let f = self.slot(key)?;
        f.shape = Shape::Nested;
        f.subs = subs;
        match f.subs.iter_mut().find(|(k, _)| k == sub) {
            Some(slot) => slot.1 = value.to_string(),
            None => f.subs.push((sub.to_string(), value.to_string())),
        }
        Ok(())
    }

    /// Removes a key.
    pub fn delete(&mut self, key: &str) {
        if let Some(i) = self.fields.iter().position(|f| f.key == key) {
            self.fields.remove(i);
        }
    }

    /// Renders the document. A parsed document with no changes comes back
    /// identical, apart from a file with no newline after its closing delimiter,
    /// which gains one.
    pub fn to_bytes(&self) -> Vec<u8> {
        let eol = self.eol;
        let mut out = String::new();
        let mut write = |line: &str| {
            out.push_str(line);
            out.push_str(eol);
        };
        write(DELIMITER);
        for f in &self.fields {
            for line in &f.before {
                write(line);
            }
            for line in field_lines(f) {
                write(&line);
            }
        }
        for line in &self.trailing {
            write(line);
        }
        write(DELIMITER);
        out.push_str(&self.body);
        out.into_bytes()
    }
}

fn check_value(value: &str) -> Result<(), Error> {
    if value.contains(['\r', '\n']) {
        return Err(err(format!(
            "frontmatter: values can't span lines: {}",
            gofmt::quote(value)
        )));
    }
    Ok(())
}

fn quote(value: &str) -> String {
    if plain_ok(value) && !value.ends_with(' ') {
        return value.to_string();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn field_lines(f: &Field) -> Vec<String> {
    if let Some(raw) = &f.raw {
        return raw.clone();
    }
    match f.shape {
        Shape::List => {
            let items: Vec<String> = f.items.iter().map(|i| quote(i)).collect();
            vec![format!("{}: [{}]", f.key, items.join(", "))]
        }
        Shape::Nested => {
            let mut lines = vec![format!("{}:", f.key)];
            lines.extend(f.subs.iter().map(|(k, v)| format!("  {k}: {}", quote(v))));
            lines
        }
        Shape::Scalar if f.value.is_empty() => vec![format!("{}:", f.key)],
        Shape::Scalar => vec![format!("{}: {}", f.key, quote(&f.value))],
    }
}

fn parse_lines(lines: Vec<String>, body: String, eol: &'static str) -> Result<Document, Error> {
    let mut d = Document {
        fields: Vec::new(),
        trailing: Vec::new(),
        eol,
        body,
    };
    let mut seen = std::collections::HashSet::new();
    let mut pending: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        let line_no = i + 2; // the opening delimiter is line 1
        if is_ignorable(line) {
            pending.push(line.clone());
            i += 1;
            continue;
        }
        if line.starts_with([' ', '\t']) {
            return Err(line_err(line_no, "unexpected indentation"));
        }
        if line.starts_with("- ") {
            return Err(line_err(
                line_no,
                "block lists aren't supported — use [a, b]",
            ));
        }
        let (key, raw_value) = split_key(line, line_no)?;
        if !seen.insert(key.clone()) {
            return Err(line_err(
                line_no,
                format!("duplicate key {}", gofmt::quote(&key)),
            ));
        }
        let mut f = Field {
            key: key.clone(),
            shape: Shape::Scalar,
            value: String::new(),
            items: Vec::new(),
            subs: Vec::new(),
            raw: Some(vec![line.clone()]),
            before: std::mem::take(&mut pending),
        };
        let no_value = raw_value.is_empty() || raw_value.starts_with('#');
        if no_value && i + 1 < lines.len() && lines[i + 1].starts_with("  ") {
            f.shape = Shape::Nested;
            while i + 1 < lines.len() && lines[i + 1].starts_with("  ") {
                i += 1;
                let (sub_key, sub_value) = parse_sub(&lines[i], i + 2)?;
                if f.subs.iter().any(|(k, _)| *k == sub_key) {
                    return Err(line_err(
                        i + 2,
                        format!(
                            "duplicate key {}",
                            gofmt::quote(&format!("{key}.{sub_key}"))
                        ),
                    ));
                }
                f.subs.push((sub_key, sub_value));
                if let Some(raw) = f.raw.as_mut() {
                    raw.push(lines[i].clone());
                }
            }
        } else if raw_value.starts_with('[') {
            f.shape = Shape::List;
            f.items = parse_list(&raw_value, line_no)?;
        } else {
            f.value = parse_scalar(&raw_value, line_no)?;
        }
        d.fields.push(f);
        i += 1;
    }
    d.trailing = pending;
    Ok(d)
}

fn split_key(line: &str, line_no: usize) -> Result<(String, String), Error> {
    let Some(i) = line.find(':') else {
        return Err(line_err(line_no, "expected \"key: value\""));
    };
    let key = &line[..i];
    if !key_ok(key) {
        return Err(line_err(
            line_no,
            format!("invalid key {}", gofmt::quote(key)),
        ));
    }
    let rest = &line[i + 1..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return Err(line_err(
            line_no,
            format!(
                "expected a space after {}",
                gofmt::quote(&format!("{key}:"))
            ),
        ));
    }
    Ok((key.to_string(), rest.trim().to_string()))
}

fn parse_sub(line: &str, line_no: usize) -> Result<(String, String), Error> {
    let inner = &line[2..];
    if inner.starts_with([' ', '\t']) {
        return Err(line_err(line_no, "only one level of nesting is supported"));
    }
    if inner.starts_with("- ") {
        return Err(line_err(
            line_no,
            "block lists aren't supported — use [a, b]",
        ));
    }
    let (key, raw) = split_key(inner, line_no)?;
    if raw.is_empty() {
        return Err(line_err(line_no, "only one level of nesting is supported"));
    }
    if raw.starts_with('[') {
        return Err(line_err(line_no, "a nested value must be a scalar"));
    }
    Ok((key, parse_scalar(&raw, line_no)?))
}

/// One value: double-quoted with `\"` and `\\` escapes, single-quoted with `''`
/// for a quote, or plain up to a ` #` comment.
fn parse_scalar(raw: &str, line_no: usize) -> Result<String, Error> {
    let Some(first) = raw.bytes().next() else {
        return Ok(String::new());
    };
    match first {
        b'"' | b'\'' => {
            let (value, rest) = read_quoted(raw).map_err(|e| line_err(line_no, e))?;
            let rest = rest.trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                return Err(line_err(line_no, "unexpected text after a quoted value"));
            }
            Ok(value)
        }
        b'&' | b'*' => Err(line_err(line_no, "anchors and aliases aren't supported")),
        b'|' | b'>' => Err(line_err(line_no, "multiline values aren't supported")),
        b'{' => Err(line_err(line_no, "inline maps aren't supported")),
        b'#' => Ok(String::new()),
        _ => {
            let raw = match raw.find(" #") {
                Some(i) => &raw[..i],
                None => raw,
            };
            Ok(raw.trim().to_string())
        }
    }
}

fn read_quoted(raw: &str) -> Result<(String, &str), &'static str> {
    let b = raw.as_bytes();
    let q = b[0];
    let mut out: Vec<u8> = Vec::new();
    let mut i = 1;
    while i < b.len() {
        let c = b[i];
        if q == b'"' && c == b'\\' {
            i += 1;
            if i >= b.len() || (b[i] != b'"' && b[i] != b'\\') {
                return Err(r#"only \" and \\ escapes are supported"#);
            }
            out.push(b[i]);
        } else if c == q {
            if q == b'\'' && i + 1 < b.len() && b[i + 1] == b'\'' {
                i += 2;
                out.push(b'\'');
                continue;
            }
            // Only ASCII quote bytes were removed, so the rest is still UTF-8.
            return Ok((String::from_utf8_lossy(&out).into_owned(), &raw[i + 1..]));
        } else {
            out.push(c);
        }
        i += 1;
    }
    Err("unterminated quote")
}

fn parse_list(raw: &str, line_no: usize) -> Result<Vec<String>, Error> {
    let mut items: Vec<String> = Vec::new();
    let mut s = &raw[1..];
    loop {
        s = s.trim_start_matches(' ');
        if s.is_empty() {
            return Err(line_err(line_no, "unterminated list"));
        }
        if let Some(after) = s.strip_prefix(']') {
            if !items.is_empty() {
                return Err(line_err(line_no, "trailing comma in list"));
            }
            check_after_list(after, line_no)?;
            return Ok(items);
        }
        let item;
        if s.starts_with(['"', '\'']) {
            let (value, rest) = read_quoted(s).map_err(|e| line_err(line_no, e))?;
            item = value;
            s = rest.trim_start_matches(' ');
        } else {
            let Some(end) = s.find([',', ']']) else {
                return Err(line_err(line_no, "unterminated list"));
            };
            let it = s[..end].trim();
            if it.is_empty() {
                return Err(line_err(line_no, "empty list item"));
            }
            if it.starts_with(['[', '{', '&', '*', '|', '>']) {
                return Err(line_err(line_no, "list items must be scalars"));
            }
            item = it.to_string();
            s = &s[end..];
        }
        items.push(item);
        if s.is_empty() {
            return Err(line_err(line_no, "unterminated list"));
        }
        match s.as_bytes()[0] {
            b',' => {
                s = &s[1..];
                if s.trim_start_matches(' ').starts_with(']') {
                    return Err(line_err(line_no, "trailing comma in list"));
                }
            }
            b']' => {
                check_after_list(&s[1..], line_no)?;
                return Ok(items);
            }
            _ => return Err(line_err(line_no, "expected ',' or ']' in list")),
        }
    }
}

fn check_after_list(rest: &str, line_no: usize) -> Result<(), Error> {
    let rest = rest.trim();
    if !rest.is_empty() && !rest.starts_with('#') {
        return Err(line_err(line_no, "unexpected text after a list"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
