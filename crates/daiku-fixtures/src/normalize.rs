//! Replacing run-specific values with fixed tokens (R6, D6, D9, D10).

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;

use crate::run::Sandbox;

/// Seconds since the Unix epoch, now.
pub(crate) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

/// Unix seconds to the UTC (`YYYY-MM-DD`, `HH:MM:SS`), with no date crate.
#[doc(hidden)]
pub fn civil(secs: i64) -> (String, String) {
    let days = secs.div_euclid(86_400);
    let sod = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        format!("{y:04}-{m:02}-{d:02}"),
        format!("{:02}:{:02}:{:02}", sod / 3600, sod % 3600 / 60, sod % 60),
    )
}

/// What a stored workspace gets in place of a run's timestamps and dates (D3).
pub const FIXED_TIMESTAMP: &str = "2026-01-15T00:00:00.000Z";
pub const FIXED_DATE: &str = "2026-01-15";

pub(crate) struct Normalizer {
    /// Write fixed past values instead of `{TIMESTAMP}` and `{TODAY}` (D3).
    fixed: bool,
    timestamp: Regex,
    hex: Regex,
    dates: Vec<String>,
    /// (text, token), longest text first.
    paths: Vec<(String, &'static str)>,
    commits: Vec<String>,
    /// A path after a token, to write with `/`: Windows prints `{ROOT}\web`, where
    /// the cases were captured from Go on Unix (spec open question: Windows
    /// fixtures may normalise path separators).
    tokened_path: Regex,
    unify_separators: bool,
}

impl Normalizer {
    pub(crate) fn new(sb: &Sandbox, secs: &[i64], commits: &[String]) -> Normalizer {
        let pairs = [
            (&sb.root, &sb.given[0], "{ROOT}"),
            (&sb.home, &sb.given[1], "{HOME}"),
            (&sb.config, &sb.given[2], "{CONFIG}"),
        ];
        let mut paths = Vec::new();
        for (canon, given, token) in pairs {
            for p in [canon, given] {
                paths.extend(path_forms(p).into_iter().map(|f| (f, token)));
            }
        }
        let dates = secs.iter().map(|s| civil(*s).0).collect();
        Normalizer::from_parts(paths, dates, commits.to_vec())
    }

    /// The mode for stored workspaces: timestamps and the run's dates become fixed
    /// past values; paths still become tokens; commit ids are left alone.
    pub(crate) fn fixed(sb: &Sandbox, secs: &[i64]) -> Normalizer {
        let mut n = Normalizer::new(sb, secs, &[]);
        n.fixed = true;
        n
    }

    fn from_parts(
        mut paths: Vec<(String, &'static str)>,
        mut dates: Vec<String>,
        commits: Vec<String>,
    ) -> Normalizer {
        paths.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then(a.0.cmp(&b.0)));
        paths.dedup();
        dates.dedup();
        Normalizer {
            fixed: false,
            timestamp: Regex::new(
                r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})",
            )
            .expect("timestamp pattern"),
            hex: Regex::new(r"\b[0-9a-f]{7,40}\b").expect("hex pattern"),
            dates,
            paths,
            commits,
            // A token, then segments each led by a backslash, or by two when the
            // text is JSON-escaped.
            tokened_path: Regex::new(r#"\{(?:ROOT|HOME|CONFIG)\}(?:(?:\\\\|\\)[^\s"'\\{}<>|]+)+"#)
                .expect("tokened path pattern"),
            unify_separators: cfg!(windows),
        }
    }

    /// [`Normalizer::apply`] on bytes that may not be valid UTF-8: each valid run
    /// is normalised and every other byte kept. Exact, because every token (a date,
    /// timestamp, path or commit id) is ASCII and so lies inside one valid run.
    pub(crate) fn apply_bytes(&self, b: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(b.len());
        for chunk in b.utf8_chunks() {
            out.extend_from_slice(self.apply(chunk.valid()).as_bytes());
            out.extend_from_slice(chunk.invalid());
        }
        out
    }

    pub(crate) fn apply(&self, s: &str) -> String {
        let (stamp, today) = if self.fixed {
            (FIXED_TIMESTAMP, FIXED_DATE)
        } else {
            ("{TIMESTAMP}", "{TODAY}")
        };
        let mut s = self.timestamp.replace_all(s, stamp).into_owned();
        for d in &self.dates {
            s = s.replace(d.as_str(), today);
        }
        for (p, token) in &self.paths {
            s = s.replace(p.as_str(), token);
        }
        if self.unify_separators {
            s = self
                .tokened_path
                .replace_all(&s, |c: &regex::Captures| {
                    let m = &c[0];
                    let (token, rest) = m.split_at(m.find('}').map_or(0, |i| i + 1));
                    format!("{token}{}", rest.replace("\\\\", "/").replace('\\', "/"))
                })
                .into_owned();
        }
        if self.commits.is_empty() {
            return s;
        }
        self.hex
            .replace_all(&s, |c: &regex::Captures| {
                let m = &c[0];
                if self.commits.iter().any(|id| id.starts_with(m)) {
                    "{COMMIT}".to_string()
                } else {
                    m.to_string()
                }
            })
            .into_owned()
    }
}

/// The spellings of a path a binary may print: as is, with either slash, without a
/// Windows verbatim prefix, and JSON-escaped.
fn path_forms(p: &Path) -> Vec<String> {
    let raw = p.to_string_lossy().into_owned();
    let mut bases = vec![raw.clone()];
    if let Some(stripped) = raw.strip_prefix(r"\\?\") {
        bases.push(stripped.to_string());
    }
    let mut out = Vec::new();
    for b in bases {
        let fwd = b.replace('\\', "/");
        let back = b.replace('/', "\\");
        let json = back.replace('\\', r"\\");
        for f in [b, fwd, back, json] {
            if !out.contains(&f) {
                out.push(f);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(commits: &[&str]) -> Normalizer {
        let mut paths = Vec::new();
        for (p, t) in [
            ("/private/var/T/fx-root-abc", "{ROOT}"),
            ("/var/T/fx-root-abc", "{ROOT}"),
            ("/var/T/fx-home-abc", "{HOME}"),
            (r"C:\Temp\fx-config-abc", "{CONFIG}"),
        ] {
            paths.extend(path_forms(Path::new(p)).into_iter().map(|f| (f, t)));
        }
        let mut n = Normalizer::from_parts(
            paths,
            vec!["2026-11-18".into(), "2026-11-19".into()],
            commits.iter().map(|c| c.to_string()).collect(),
        );
        // These tests are about tokens, on every platform: the Windows separator
        // rule has its own test.
        n.unify_separators = false;
        n
    }

    /// On Windows a path after a token is written with `/`, plain or JSON-escaped,
    /// and nothing else changes.
    #[test]
    fn separators_after_a_token_are_unified_when_asked() {
        let mut n = n(&[]);
        n.unify_separators = true;
        assert_eq!(
            n.apply(r"cloned self to {HOME}\clones\self"),
            "cloned self to {HOME}/clones/self"
        );
        assert_eq!(
            n.apply(r#"  "web": "{ROOT}\\web\\sub""#),
            r#"  "web": "{ROOT}/web/sub""#
        );
        assert_eq!(
            n.apply(r"api: {ROOT}\elsewhere ok C:\keep"),
            r"api: {ROOT}/elsewhere ok C:\keep"
        );
        assert_eq!(n.apply("{ROOT} alone"), "{ROOT} alone");
        assert_eq!(n.apply(r"no token a\b"), r"no token a\b");
        // Off: untouched, on every platform.
        assert_eq!(super::tests::n(&[]).apply(r"{ROOT}\x"), r"{ROOT}\x");
    }

    /// Tokens next to bytes that aren't valid UTF-8 are normalised, and the bytes
    /// are kept.
    #[test]
    fn bytes_keep_invalid_runs() {
        let n = n(&[]);
        let got = n.apply_bytes(b"\xff2026-11-19\xfe /var/T/fx-root-abc/x\xe9\n");
        assert_eq!(got, b"\xff{TODAY}\xfe {ROOT}/x\xe9\n");
        assert_eq!(n.apply_bytes(b"plain 2026-11-18"), b"plain {TODAY}");
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), ("1970-01-01".into(), "00:00:00".into()));
        assert_eq!(civil(1_767_225_600).0, "2026-01-01");
        assert_eq!(civil(951_782_400).0, "2000-02-29");
        assert_eq!(civil(1_763_464_000).1, "11:06:40");
    }

    #[test]
    fn timestamps_and_dates() {
        let n = n(&[]);
        assert_eq!(
            n.apply(
                "at 2026-11-18T10:00:00.123Z and 2025-01-02T03:04:05+02:00 and 2026-11-18T10:00:00Z"
            ),
            "at {TIMESTAMP} and {TIMESTAMP} and {TIMESTAMP}"
        );
        assert_eq!(
            n.apply("on 2026-11-18, 2026-11-19, not 2026-11-17"),
            "on {TODAY}, {TODAY}, not 2026-11-17"
        );
    }

    #[test]
    fn paths_in_both_forms() {
        let n = n(&[]);
        assert_eq!(n.apply("/private/var/T/fx-root-abc/x"), "{ROOT}/x");
        assert_eq!(n.apply("/var/T/fx-root-abc/x"), "{ROOT}/x");
        assert_eq!(n.apply(r"\var\T\fx-home-abc\x"), r"{HOME}\x");
        assert_eq!(n.apply(r"C:\Temp\fx-config-abc\y"), r"{CONFIG}\y");
        assert_eq!(n.apply("C:/Temp/fx-config-abc/y"), "{CONFIG}/y");
        assert_eq!(n.apply(r#""C:\\Temp\\fx-config-abc""#), r#""{CONFIG}""#);
    }

    #[test]
    fn commits_by_exact_match_only() {
        let id = "0123456789abcdef0123456789abcdef01234567";
        let n = n(&[id]);
        let hash = "a".repeat(64);
        assert_eq!(n.apply(&format!("{id} x")), "{COMMIT} x");
        assert_eq!(n.apply("[main 0123456] msg"), "[main {COMMIT}] msg");
        assert_eq!(n.apply(&format!("hash {hash}")), format!("hash {hash}"));
        assert_eq!(n.apply("deadbeef1"), "deadbeef1");
    }

    #[test]
    fn fixed_mode_writes_past_values() {
        let mut n = n(&["0123456789abcdef0123456789abcdef01234567"]);
        n.fixed = true;
        assert_eq!(
            n.apply("at 2026-11-18T10:00:00.123Z on 2026-11-19 in /var/T/fx-root-abc/x"),
            "at 2026-01-15T00:00:00.000Z on 2026-01-15 in {ROOT}/x"
        );
        assert_eq!(n.apply("not 2026-11-17"), "not 2026-11-17");
        n.commits.clear();
        assert_eq!(n.apply("0123456"), "0123456");
    }

    #[test]
    fn no_commits_no_change() {
        assert_eq!(n(&[]).apply("0123456"), "0123456");
    }
}
