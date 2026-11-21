//! daiku-core's port of Go's JSON syntax errors against `gojson-go.txt`: Go's own
//! messages, captured from `v0.2.0` and kept as daiku's expected wording (AC8).

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
        // Through parse, so Go's decode errors count too. Nesting beyond serde_json's
        // limit of 128 is checked by the scanner alone.
        let deep = input.starts_with(&[b'['; 129]);
        let got = match (deep, gojson::check(&input)) {
            (true, Ok(())) => "ok".to_string(),
            (true, Err(e)) => e,
            (false, _) => match gojson::parse(&input) {
                Ok(_) => "ok".to_string(),
                Err(e) => e,
            },
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

#[test]
fn first_value_len_finds_the_end_in_one_pass() {
    for (input, want) in [
        (&b"{} x"[..], Some(2)),
        (b"  {\"a\": [1, {}]}  }", Some(16)),
        (b"12 x", Some(2)),
        (b"12", Some(2)),
        (b"\"s\" x", Some(3)),
        (b"true}", Some(4)),
        (b"[1]", Some(3)),
        (b"{", None),
        (b"{ x", None),
        (b"", None),
    ] {
        assert_eq!(
            gojson::first_value_len(input),
            want,
            "{:?}",
            String::from_utf8_lossy(input)
        );
    }
    // Linear: a megabyte string with trailing data is measured at once.
    let big = format!("{{\"pad\": \"{}\"}} x", "x".repeat(1 << 20));
    let start = std::time::Instant::now();
    assert_eq!(gojson::first_value_len(big.as_bytes()), Some(big.len() - 2));
    assert!(
        start.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        start.elapsed()
    );
}
