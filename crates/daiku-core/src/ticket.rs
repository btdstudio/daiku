//! Tickets: the read side of Go's `internal/core/ticket` (D8).

use std::sync::LazyLock;

use regex::Regex;

/// Any valid ticket id: planned (`S-0001`), native (`N-0001`) or pulled (`X-…`).
/// A build change is named by one, so it doubles as a safe folder name.
pub static ID_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:([EFSTN])-([0-9]{4})|X-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*)$")
        .expect("valid pattern")
});
