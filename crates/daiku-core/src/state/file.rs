//! Reading and writing `STATE.json` as Go does.
//!
//! Go decodes the file with `json.Unmarshal` into `stateFile`, which has rules of
//! its own: keys match fields without regard to case, unknown keys are ignored,
//! `null` leaves a string as it was, a repeated object key merges into a map, and a
//! repeated array reuses the slice's storage. [`decode`] follows them, and its type
//! errors carry Go's wording. `tests/state-go.txt` holds Go's behaviour, captured
//! from `v0.2.0`, as the tests' expectations.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::gojson;
use crate::gojson::decode::{
    At, GoSlice, Kind, Reader, fold, int, mismatch, string, string_map, string_slice,
};

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
                    "SCHEMA" => int(&mut r, &mut file.schema, field("stateFile", "schema"))?,
                    "CHANGES" => changes(&mut r, &mut file.changes)?,
                    _ => r.skip(),
                }
            }
        }
        k => return Err(mismatch(&mut r, k, At::Value, "state.stateFile")),
    }
    Ok(file)
}

fn field<'a>(strukt: &'a str, field: &'a str) -> At<'a> {
    At::Field { strukt, field }
}

fn changes(
    r: &mut Reader,
    into: &mut Option<BTreeMap<String, Option<RawChange>>>,
) -> Result<(), String> {
    let at = field("stateFile", "changes");
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
        k => Err(mismatch(r, k, at, "map[string]*state.Change")),
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
            let mut c = RawChange::default();
            let f = |name| field("Change", name);
            while let Some(key) = r.next_key() {
                match fold(&key).as_str() {
                    "LOOP" => string(r, &mut c.loop_, f("changes.loop"))?,
                    "PHASE" => string(r, &mut c.phase, f("changes.phase"))?,
                    "STATUS" => string(r, &mut c.status, f("changes.status"))?,
                    "GATES" => string_map(r, &mut c.gates, f("changes.gates"))?,
                    "REASONS" => string_map(r, &mut c.reasons, f("changes.reasons"))?,
                    "SOURCES" => string_slice(r, &mut c.sources, f("changes.sources"))?,
                    "REPOS" => string_slice(r, &mut c.repos, f("changes.repos"))?,
                    "UPDATEDAT" => string(r, &mut c.updated_at, f("changes.updatedAt"))?,
                    _ => r.skip(),
                }
            }
            Ok(Some(c))
        }
        k => Err(mismatch(
            r,
            k,
            field("stateFile", "changes"),
            "state.Change",
        )),
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
