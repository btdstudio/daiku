//! The part of Go's `net/url.Parse` that the registry's remote checks use: a
//! scheme, whether there's userinfo, the host name and the path. It fails where
//! Go's parser does for the same input, since `checkRemote` skips its credentials
//! test when `url.Parse` errors (no URL crate: ADR-0005).

/// A parsed URL, as far as the registry needs it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Url {
    /// Lower-cased, as Go's `Scheme`.
    pub scheme: String,
    pub has_user: bool,
    /// `Host`, with any port: `""` when there's no authority.
    pub host: String,
    /// The unescaped `Path`.
    pub path: String,
}

impl Url {
    /// Go's `Hostname`: the host without its port or IPv6 brackets.
    pub fn hostname(&self) -> String {
        let (host, _) = split_host_port(&self.host);
        host.to_string()
    }
}

fn split_host_port(host_port: &str) -> (&str, &str) {
    let mut host = host_port;
    let mut port = "";
    if let Some(colon) = host_port.rfind(':')
        && valid_optional_port(&host_port[colon..])
    {
        host = &host_port[..colon];
        port = &host_port[colon + 1..];
    }
    if host.starts_with('[') && host.ends_with(']') && host.len() >= 2 {
        host = &host[1..host.len() - 1];
    }
    (host, port)
}

fn valid_optional_port(port: &str) -> bool {
    match port.strip_prefix(':') {
        None => port.is_empty(),
        Some(digits) => digits.bytes().all(|b| b.is_ascii_digit()),
    }
}

/// Go's `getScheme`: the scheme and the rest, or `None` for an invalid first
/// character that rules a scheme out.
fn get_scheme(raw: &str) -> (&str, &str) {
    for (i, c) in raw.char_indices() {
        match c {
            'a'..='z' | 'A'..='Z' => {}
            '0'..='9' | '+' | '-' | '.' => {
                if i == 0 {
                    return ("", raw);
                }
            }
            ':' => {
                if i == 0 {
                    return ("", raw);
                }
                return (&raw[..i], &raw[i + 1..]);
            }
            _ => return ("", raw),
        }
    }
    ("", raw)
}

/// `%` must be followed by two hex digits.
fn valid_escapes(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            match (b.get(i + 1), b.get(i + 2)) {
                (Some(h), Some(l)) if h.is_ascii_hexdigit() && l.is_ascii_hexdigit() => i += 3,
                _ => return false,
            }
        } else {
            i += 1;
        }
    }
    true
}

/// Go's `unescape` for a path: `%XX` decoded (invalid UTF-8 kept lossy). Callers
/// check [`valid_escapes`] first.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: Option<&u8>| c.and_then(|c| (*c as char).to_digit(16)).map(|d| d as u8);
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (hex(b.get(i + 1)), hex(b.get(i + 2)))
        {
            out.push(h << 4 | l);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Go's `validUserinfo`.
fn valid_userinfo(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "-._:~!$&'()*+,;=%@".contains(c))
}

/// Go's `parseHost` checks: a bracketed literal or a name, an optional port, and
/// only characters that may appear in a host.
///
/// This is deliberately more lenient than Go in one corner, and so stricter for the
/// registry: Go also rejects a host with a percent-escaped ASCII character (`%41`;
/// only non-ASCII bytes and `%25` may be escaped there). Go then skips its
/// credentials test, because it only runs on a URL that parses, and so accepts
/// `https://u:p@h%41/x`. This port parses it, sees the credentials, and refuses it.
/// Matching Go here would let credentials into the committed `REPOS.json`
/// (S-0008 review, finding 1; pinned by `repos::tests`).
fn valid_host(host: &str) -> bool {
    if host.starts_with('[') {
        let Some(i) = host.rfind(']') else {
            return false;
        };
        return valid_optional_port(&host[i + 1..]);
    }
    if let Some(i) = host.rfind(':')
        && !valid_optional_port(&host[i..])
    {
        return false;
    }
    if !valid_escapes(host) {
        return false;
    }
    host.bytes().all(|b| {
        b >= 0x80
            || b == b'%'
            || b.is_ascii_alphanumeric()
            || b"-_.~!$&'()*+,;=:[]<>\"".contains(&b)
    })
}

/// Go's `url.Parse`, reduced to what the registry reads. `None` is Go's error.
pub fn parse(raw: &str) -> Option<Url> {
    if raw.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return None;
    }
    let (raw, _fragment) = raw.split_once('#').unwrap_or((raw, ""));
    if raw.is_empty() {
        return Some(Url::default());
    }
    let (scheme, rest) = get_scheme(raw);
    let scheme = scheme.to_ascii_lowercase();
    let rest = rest.split_once('?').map_or(rest, |(before, _)| before);
    if !rest.starts_with('/') {
        if !scheme.is_empty() {
            // Opaque: no authority and no path of interest.
            return Some(Url {
                scheme,
                ..Url::default()
            });
        }
        // `a:b/c` with no scheme: a colon in the first segment is an error.
        let segment = rest.split('/').next().unwrap_or("");
        if segment.contains(':') {
            return None;
        }
    }
    let mut url = Url {
        scheme: scheme.clone(),
        ..Url::default()
    };
    let mut path = rest;
    if rest.starts_with("//") && (!scheme.is_empty() || !rest.starts_with("///")) {
        let after = &rest[2..];
        let end = after.find('/').unwrap_or(after.len());
        let authority = &after[..end];
        path = &after[end..];
        let host = match authority.rfind('@') {
            None => authority,
            Some(i) => {
                let userinfo = &authority[..i];
                if !valid_userinfo(userinfo) || !valid_escapes(userinfo) {
                    return None;
                }
                url.has_user = true;
                &authority[i + 1..]
            }
        };
        if !valid_host(host) {
            return None;
        }
        url.host = host.to_string();
    }
    if !valid_escapes(path) {
        return None;
    }
    url.path = unescape(path);
    Some(url)
}
