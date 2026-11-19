//! How core writes JSON files (R11, ADR-0006): two-space indentation, struct field
//! order, sorted map keys (maps are `BTreeMap`, or serde_json's sorted `Map`), and
//! a trailing newline, with serde_json's normal escaping.

use serde::Serialize;

/// `value` as pretty JSON plus a newline.
pub fn to_pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut out = serde_json::to_vec_pretty(value)?;
    out.push(b'\n');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct State<'a> {
        schema: u32,
        changes: BTreeMap<&'a str, Vec<&'a str>>,
        note: &'a str,
    }

    #[test]
    fn two_spaces_field_order_sorted_keys_newline() {
        let mut changes = BTreeMap::new();
        changes.insert("b", vec!["x"]);
        changes.insert("a", vec![]);
        let s = State {
            schema: 1,
            changes,
            note: "<a> & 'b'",
        };
        let text = String::from_utf8(to_pretty(&s).unwrap()).unwrap();
        assert_eq!(
            text,
            "{\n  \"schema\": 1,\n  \"changes\": {\n    \"a\": [],\n    \"b\": [\n      \"x\"\n    ]\n  },\n  \"note\": \"<a> & 'b'\"\n}\n"
        );
    }

    #[test]
    fn serde_json_maps_are_sorted() {
        let v: serde_json::Value = serde_json::from_str(r#"{"z":1,"a":2}"#).unwrap();
        assert_eq!(to_pretty(&v).unwrap(), b"{\n  \"a\": 2,\n  \"z\": 1\n}\n");
    }
}
