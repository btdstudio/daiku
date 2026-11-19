//! daiku-core's port of Go's JSON syntax errors against Go's own messages, captured
//! by `scripts/gojson-messages.go` into `gojson-go.txt` (AC8).

use daiku_core::gojson;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}

fn table() -> Vec<(Vec<u8>, String)> {
    include_str!("gojson-go.txt")
        .lines()
        .map(|line| {
            let (input, msg) = line.split_once('\t').unwrap();
            (unhex(input), msg.to_string())
        })
        .collect()
}

#[test]
fn messages_match_go() {
    let table = table();
    assert!(
        table.len() > 50,
        "the table should cover Go's scanner states"
    );
    for (input, want) in table {
        let got = match gojson::check(&input) {
            Ok(()) => "ok".to_string(),
            Err(e) => e,
        };
        assert_eq!(got, want, "input {:?}", String::from_utf8_lossy(&input));
    }
}

/// Every document Go accepts parses with serde_json too, except nesting deeper than
/// serde_json's limit of 128, which daiku's files never reach.
#[test]
fn what_go_accepts_serde_json_parses() {
    for (input, want) in table() {
        if want != "ok" || input.starts_with(&[b'['; 129]) {
            continue;
        }
        assert!(
            gojson::parse(&input).is_ok(),
            "input {:?}",
            String::from_utf8_lossy(&input)
        );
    }
    // Invalid UTF-8 inside a string is kept as U+FFFD, as Go's decoder does.
    let v = gojson::parse(b"{\"a\":\"\xff\"}").unwrap();
    assert_eq!(v["a"], "\u{fffd}");
}
