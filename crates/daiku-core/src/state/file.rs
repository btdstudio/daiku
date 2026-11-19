//! Reading and writing `STATE.json` as Go does.
//!
//! Go decodes the file with `json.Unmarshal` into `stateFile`, which has rules of
//! its own: keys match fields without regard to case, unknown keys are ignored,
//! `null` leaves a string as it was, a repeated object key merges into a map, and a
//! repeated array reuses the slice's storage. [`decode`] follows them, and its type
//! errors carry Go's wording. `scripts/state-messages.go` captures Go's behaviour
//! for the tests.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::gojson;
use crate::gojson::decode::{Kind, Reader, fold};

/// The file's schema version this daiku writes and reads.
pub const SCHEMA: i64 = 1;

/// A change's record as decoded, with Go's nil distinctions kept.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawChange {
    pub loop_: String,
    pub phase: String,
    pub status: String,
    pub gates: Option<BTreeMap<String, String>>,
    pub reasons: Option<BTreeMap<String, String>>,
    pub sources: GoSlice,
    pub repos: GoSlice,
    pub updated_at: String,
}

/// A Go `[]string` as `json.Unmarshal` fills it: decoding an array into a slice
/// reuses its backing array, so elements past the length can come back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoSlice {
    backing: Vec<String>,
    len: usize,
    nil: bool,
}

impl GoSlice {
    fn nil() -> GoSlice {
        GoSlice {
            backing: Vec::new(),
            len: 0,
            nil: true,
        }
    }

    pub fn is_nil(&self) -> bool {
        self.nil
    }

    pub fn items(&self) -> &[String] {
        &self.backing[..self.len]
    }
}

/// The decoded file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RawFile {
    pub schema: i64,
    pub changes: Option<BTreeMap<String, Option<RawChange>>>,
}

/// Decodes `STATE.json` as Go's `json.Unmarshal` into `stateFile` does, returning
/// Go's error text (syntax or type) on failure.
pub fn decode(data: &[u8]) -> Result<RawFile, String> {
    gojson::check(data)?;
    let mut r = Reader::new(data);
    let mut file = RawFile::default();
    match r.kind() {
        Kind::Null => {
            r.literal();
        }
        Kind::Object => {
            r.open();
            while let Some(key) = r.next_key() {
                match fold(&key).as_str() {
                    "SCHEMA" => int(&mut r, &mut file.schema, "stateFile", "schema")?,
                    "CHANGES" => changes(&mut r, &mut file.changes)?,
                    _ => r.skip(),
                }
            }
        }
        k => {
            return Err(format!(
                "json: cannot unmarshal {} into Go value of type state.stateFile",
                k.go_name()
            ));
        }
    }
    Ok(file)
}

fn type_error(value: &str, strukt: &str, field: &str, ty: &str) -> String {
    format!("json: cannot unmarshal {value} into Go struct field {strukt}.{field} of type {ty}")
}

/// Skips a value of the wrong kind and returns Go's type error for it.
fn mismatch(r: &mut Reader, k: Kind, strukt: &str, field: &str, ty: &str) -> String {
    r.skip();
    type_error(k.go_name(), strukt, field, ty)
}

fn int(r: &mut Reader, into: &mut i64, strukt: &str, field: &str) -> Result<(), String> {
    match r.kind() {
        Kind::Null => {
            r.literal();
            Ok(())
        }
        Kind::Number => {
            let lit = r.literal();
            match lit.parse::<i64>() {
                Ok(n) => {
                    *into = n;
                    Ok(())
                }
                Err(_) => Err(type_error(&format!("number {lit}"), strukt, field, "int")),
            }
        }
        k => Err(mismatch(r, k, strukt, field, "int")),
    }
}

fn string(r: &mut Reader, into: &mut String, field: &str) -> Result<(), String> {
    match r.kind() {
        Kind::Null => {
            r.literal();
            Ok(())
        }
        Kind::String => {
            *into = r.string();
            Ok(())
        }
        k => Err(mismatch(r, k, "Change", field, "string")),
    }
}

fn string_map(
    r: &mut Reader,
    into: &mut Option<BTreeMap<String, String>>,
    field: &str,
) -> Result<(), String> {
    match r.kind() {
        Kind::Null => {
            r.literal();
            *into = None;
            Ok(())
        }
        Kind::Object => {
            r.open();
            let map = into.get_or_insert_with(BTreeMap::new);
            while let Some(key) = r.next_key() {
                let mut v = String::new();
                string(r, &mut v, field)?;
                map.insert(key, v);
            }
            Ok(())
        }
        k => Err(mismatch(r, k, "Change", field, "map[string]string")),
    }
}

fn string_slice(r: &mut Reader, into: &mut GoSlice, field: &str) -> Result<(), String> {
    match r.kind() {
        Kind::Null => {
            r.literal();
            *into = GoSlice::nil();
            Ok(())
        }
        Kind::Array => {
            r.open();
            into.nil = false;
            let mut i = 0;
            while r.next_item() {
                if i >= into.backing.len() {
                    into.backing.push(String::new());
                }
                if i >= into.len {
                    into.len = i + 1;
                }
                string(r, &mut into.backing[i], field)?;
                i += 1;
            }
            if i < into.len {
                into.len = i;
            }
            if i == 0 {
                *into = GoSlice::default();
            }
            Ok(())
        }
        k => Err(mismatch(r, k, "Change", field, "[]string")),
    }
}

fn changes(
    r: &mut Reader,
    into: &mut Option<BTreeMap<String, Option<RawChange>>>,
) -> Result<(), String> {
    match r.kind() {
        Kind::Null => {
            r.literal();
            *into = None;
            Ok(())
        }
        Kind::Object => {
            r.open();
            let map = into.get_or_insert_with(BTreeMap::new);
            while let Some(name) = r.next_key() {
                let c = change(r)?;
                map.insert(name, c);
            }
            Ok(())
        }
        k => Err(mismatch(
            r,
            k,
            "stateFile",
            "changes",
            "map[string]*state.Change",
        )),
    }
}

fn change(r: &mut Reader) -> Result<Option<RawChange>, String> {
    match r.kind() {
        Kind::Null => {
            r.literal();
            Ok(None)
        }
        Kind::Object => {
            r.open();
            let mut c = RawChange {
                sources: GoSlice::nil(),
                repos: GoSlice::nil(),
                ..RawChange::default()
            };
            while let Some(key) = r.next_key() {
                match fold(&key).as_str() {
                    "LOOP" => string(r, &mut c.loop_, "changes.loop")?,
                    "PHASE" => string(r, &mut c.phase, "changes.phase")?,
                    "STATUS" => string(r, &mut c.status, "changes.status")?,
                    "GATES" => string_map(r, &mut c.gates, "changes.gates")?,
                    "REASONS" => string_map(r, &mut c.reasons, "changes.reasons")?,
                    "SOURCES" => string_slice(r, &mut c.sources, "changes.sources")?,
                    "REPOS" => string_slice(r, &mut c.repos, "changes.repos")?,
                    "UPDATEDAT" => string(r, &mut c.updated_at, "changes.updatedAt")?,
                    _ => r.skip(),
                }
            }
            Ok(Some(c))
        }
        k => Err(mismatch(r, k, "stateFile", "changes", "state.Change")),
    }
}

/// A change as written: Go's field order and `omitempty` rules.
#[derive(Serialize)]
pub struct Written<'a> {
    #[serde(rename = "loop")]
    pub loop_: &'a str,
    pub phase: &'a str,
    pub status: &'a str,
    pub gates: &'a BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub reasons: &'a BTreeMap<String, String>,
    pub sources: &'a [String],
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    pub repos: &'a [String],
    #[serde(rename = "updatedAt")]
    pub updated_at: &'a str,
}

/// The whole file as written.
#[derive(Serialize)]
pub struct WrittenFile<'a> {
    pub schema: i64,
    pub changes: BTreeMap<&'a str, Written<'a>>,
}
