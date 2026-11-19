//! Repos: the read side of Go's `internal/core/repos` (D9).

use std::sync::LazyLock;

use regex::Regex;

/// A logical repo name.
pub static NAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]*$").expect("valid pattern"));
