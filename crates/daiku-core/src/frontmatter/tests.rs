//! frontmatter_test.go, translated.

use super::*;

const SAMPLE: &str = "---
schema: 1
id: S-0014                 # type prefix + sequence

# a comment on its own line
title: \"Add: release workflow\"
depends_on: [S-0011, S-0012]   # predecessors
labels: []
external:                  # written by mark-published only
  ado: 48213
  github: \"org/repo#7\"
quoted: 'it''s fine'
empty:
---

## Context
Body with --- inside.
---
";

fn parse(text: &str) -> Document {
    Document::parse(text).unwrap()
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn m(v: &[(&str, &str)]) -> BTreeMap<String, String> {
    v.iter()
        .map(|(k, x)| (k.to_string(), x.to_string()))
        .collect()
}

#[test]
fn reads_each_kind() {
    let d = parse(SAMPLE);
    for (key, want) in [
        ("schema", "1"),
        ("id", "S-0014"),
        ("title", "Add: release workflow"),
        ("quoted", "it's fine"),
        ("empty", ""),
    ] {
        assert_eq!(d.get(key), want, "{key}");
    }
    assert_eq!(d.list("depends_on"), Some(s(&["S-0011", "S-0012"])));
    assert_eq!(d.list("labels"), Some(vec![]));
    assert_eq!(
        d.map("external"),
        Some(m(&[("ado", "48213"), ("github", "org/repo#7")]))
    );
    assert!(
        d.body.starts_with("\n## Context") && d.body.ends_with("---\n"),
        "{:?}",
        d.body
    );
    assert!(
        d.get("nope").is_empty()
            && d.list("nope").is_none()
            && d.map("nope").is_none()
            && !d.has("nope")
    );
    assert!(d.get("depends_on").is_empty() && d.list("id").is_none() && d.map("id").is_none());
}

#[test]
fn unchanged_round_trip_is_byte_stable() {
    let crlf = SAMPLE.replace('\n', "\r\n");
    for (name, text) in [
        ("sample", SAMPLE),
        ("crlf", crlf.as_str()),
        ("empty", "---\n---\nbody"),
        ("only", "---\n# just a comment\n\n---\n"),
    ] {
        assert_eq!(
            String::from_utf8(parse(text).to_bytes()).unwrap(),
            text,
            "{name}"
        );
    }
}

#[test]
fn editing_one_field_leaves_the_rest_alone() {
    let mut d = parse(SAMPLE);
    d.set("id", "S-0015").unwrap();
    let want = SAMPLE.replacen(
        "id: S-0014                 # type prefix + sequence",
        "id: S-0015",
        1,
    );
    assert_eq!(String::from_utf8(d.to_bytes()).unwrap(), want);
}

/// AC4 with CRLF: an edit keeps every other line's bytes and the CRLF endings, and
/// the body (which may mix endings) is untouched.
#[test]
fn editing_a_crlf_file_keeps_its_bytes() {
    let text = SAMPLE.replace('\n', "\r\n") + "mixed\nending\r\n";
    let mut d = parse(&text);
    d.set("id", "S-0015").unwrap();
    let want = text.replacen(
        "id: S-0014                 # type prefix + sequence",
        "id: S-0015",
        1,
    );
    assert_eq!(String::from_utf8(d.to_bytes()).unwrap(), want);
}

#[test]
fn set_sub_adds_and_updates_keeping_others() {
    let mut d = parse(SAMPLE);
    d.set_sub("external", "jira", "ABC-1").unwrap();
    d.set_sub("external", "ado", "1").unwrap();
    d.set_sub("origin", "path", "specs/x.md").unwrap();
    let again = parse(&String::from_utf8(d.to_bytes()).unwrap());
    assert_eq!(
        again.map("external"),
        Some(m(&[
            ("ado", "1"),
            ("github", "org/repo#7"),
            ("jira", "ABC-1")
        ]))
    );
    assert_eq!(again.map("origin").unwrap()["path"], "specs/x.md");
}

#[test]
fn written_values_read_back() {
    for value in [
        "plain",
        "two words",
        "with: colon",
        "with # hash",
        "quo\"te",
        "back\\slash",
        "it's",
        "[not a list]",
        "{not a map}",
        "&anchor",
        "*alias",
        "| pipe",
        "> fold",
        "- dash",
        "---",
        "trailing ",
        " leading",
        "#start",
        "",
        "日本語",
    ] {
        let mut d = Document::new("body\n");
        d.set("k", value).unwrap();
        let list = s(&[&format!("{value}x"), "a, b", "c]"]);
        d.set_list("l", &list).unwrap();
        if !value.is_empty() {
            d.set_sub("m", "s", value).unwrap();
        }
        let bytes = String::from_utf8(d.to_bytes()).unwrap();
        let again = Document::parse(&bytes).unwrap_or_else(|e| panic!("{value:?}: {e}\n{bytes}"));
        assert_eq!(again.get("k"), value);
        assert_eq!(again.list("l"), Some(list));
        if !value.is_empty() {
            assert_eq!(again.map("m").unwrap()["s"], value);
        }
        assert_eq!(again.body, "body\n");
    }
}

#[test]
fn set_refuses_values_that_would_inject_lines() {
    let mut d = Document::new("");
    for value in ["a\nstatus: published", "a\r\nb", "x\n---\nbody"] {
        assert!(d.set("title", value).is_err());
        assert!(d.set_list("l", &s(&[value])).is_err());
        assert!(d.set_sub("m", "s", value).is_err());
    }
    assert!(
        !d.has("title") && !d.has("l") && !d.has("m"),
        "{:?}",
        d.keys()
    );
    assert!(d.set("bad key", "x").is_err());
    assert!(d.set_sub("m", "s", "").is_err());
}

#[test]
fn delete_and_keys() {
    let mut d = parse("---\na: 1\nb: 2\n---\n");
    d.delete("a");
    d.delete("missing");
    assert_eq!(d.keys(), s(&["b"]));
    assert_eq!(String::from_utf8(d.to_bytes()).unwrap(), "---\nb: 2\n---\n");
}

#[test]
fn rejects_unsupported_yaml() {
    for (name, text, want) in [
        ("no opening", "title: x\n---\n", "must start"),
        ("no closing", "---\ntitle: x\n", "no closing"),
        ("anchor", "---\na: &x 1\n---\n", "anchors"),
        ("alias", "---\na: *x\n---\n", "anchors"),
        ("literal", "---\na: |\n  text\n---\n", "multiline"),
        ("folded", "---\na: >\n  text\n---\n", "multiline"),
        ("flow map", "---\na: {b: 1}\n---\n", "inline maps"),
        ("deep", "---\na:\n  b:\n    c: 1\n---\n", "one level"),
        ("deep indent", "---\na:\n    c: 1\n---\n", "one level"),
        ("block list", "---\n- a\n---\n", "block lists"),
        ("block in map", "---\na:\n  - b\n---\n", "block lists"),
        ("list in map", "---\na:\n  b: [1]\n---\n", "scalar"),
        ("stray indent", "---\n  a: 1\n---\n", "indentation"),
        ("no colon", "---\njust text\n---\n", "key: value"),
        ("no space", "---\na:b\n---\n", "space after"),
        ("bad key", "---\na b: 1\n---\n", "invalid key"),
        ("duplicate", "---\na: 1\na: 2\n---\n", "duplicate"),
        (
            "duplicate sub",
            "---\na:\n  b: 1\n  b: 2\n---\n",
            "duplicate",
        ),
        ("open quote", "---\na: \"x\n---\n", "unterminated"),
        ("after quote", "---\na: \"x\" y\n---\n", "after a quoted"),
        ("open list", "---\na: [x\n---\n", "unterminated"),
        ("after list", "---\na: [x] y\n---\n", "after a list"),
        ("nested list", "---\na: [[x]]\n---\n", "scalars"),
        ("trailing", "---\na: [x,]\n---\n", "trailing comma"),
        ("empty item", "---\na: [x,,y]\n---\n", "empty list item"),
        ("bad escape", "---\na: \"x\\ny\"\n---\n", "escapes"),
    ] {
        let e = Document::parse(text).unwrap_err().to_string();
        assert!(e.contains(want), "{name}: want {want:?}, got {e}");
    }
}

/// The exact texts the fixtures show (S-0002 cases), byte for byte.
#[test]
fn messages_match_the_fixtures() {
    let e = |t: &str| Document::parse(t).unwrap_err().to_string();
    assert_eq!(
        e("---\nid: [\n---\n"),
        "frontmatter: line 2: unterminated list"
    );
    assert_eq!(
        e("---\nschema: 1\nid: E-0001\ntype: epic\ntitle: [unclosed\n---\n"),
        "frontmatter: line 5: unterminated list"
    );
    assert_eq!(
        e("no front matter\n"),
        "frontmatter: file must start with \"---\""
    );
    assert_eq!(
        e("---\n- a\n---\n"),
        "frontmatter: line 2: block lists aren't supported — use [a, b]"
    );
}

#[test]
fn errors_name_the_line() {
    let e = Document::parse("---\na: 1\nb: &x\n---\n")
        .unwrap_err()
        .to_string();
    assert!(e.contains("line 3"), "{e}");
}

#[test]
fn fields_are_a_copy() {
    let d = parse(SAMPLE);
    let view =
        |key: &str, kind: Kind, value: &str, items: &[&str], subs: &[(&str, &str)]| FieldView {
            key: key.into(),
            kind,
            value: value.into(),
            items: s(items),
            subs: m(subs),
        };
    let want = vec![
        view("schema", Kind::Scalar, "1", &[], &[]),
        view("id", Kind::Scalar, "S-0014", &[], &[]),
        view("title", Kind::Scalar, "Add: release workflow", &[], &[]),
        view("depends_on", Kind::List, "", &["S-0011", "S-0012"], &[]),
        view("labels", Kind::List, "", &[], &[]),
        view(
            "external",
            Kind::Map,
            "",
            &[],
            &[("ado", "48213"), ("github", "org/repo#7")],
        ),
        view("quoted", Kind::Scalar, "it's fine", &[], &[]),
        view("empty", Kind::Scalar, "", &[], &[]),
    ];
    let mut got = d.fields();
    assert_eq!(got, want);
    let before = d.to_bytes();
    got[3].items[0] = "X-9999".into();
    got[5].subs.insert("jira".into(), "J-1".into());
    assert_eq!(d.to_bytes(), before);
    assert_eq!(d.fields(), want);
    assert!(Document::new("body").fields().is_empty());
}
