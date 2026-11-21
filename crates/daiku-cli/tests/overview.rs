//! `daiku overview` against Go's golden workspace: a port of Go's
//! `internal/cli/overview_test.go`. The workspace and the golden README are Go's,
//! copied byte for byte into `testdata/` (marked `-text`), and never regenerated
//! from Rust's output.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const README: &str = "plan/specs/1-auth-flow/README.md";
const USAGE: &str = "usage: daiku overview [<change>] [--check] [--stdout]";

fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel)
}

/// A copy of Go's `testdata/overview/workspace`, removed when dropped.
struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Fixture {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "daiku-overview-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        copy_dir(&testdata("overview/workspace"), &dir.join("ws"));
        Fixture(dir)
    }

    fn root(&self) -> PathBuf {
        self.0.join("ws")
    }

    /// Runs `daiku <args>` in the workspace: exit code, stdout, stderr.
    fn run(&self, args: &[&str]) -> (i32, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_daiku"))
            .args(args)
            .current_dir(self.root())
            .env("XDG_CONFIG_HOME", self.0.join("config"))
            .output()
            .unwrap();
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn overview(&self, args: &[&str]) -> (i32, String, String) {
        let mut all = vec!["overview"];
        all.extend_from_slice(args);
        self.run(&all)
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.root().join(rel)
    }

    fn readme(&self) -> String {
        std::fs::read_to_string(self.path(README)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// AC2 (Go's TestOverviewGolden): two runs give byte-identical files that match
/// Go's golden, and another copy of the workspace renders the same bytes.
#[test]
fn overview_golden() {
    let f = Fixture::new();
    let (code, out, err) = f.overview(&["1-auth-flow"]);
    assert_eq!(
        (code, out.as_str()),
        (0, "wrote plan/specs/1-auth-flow/README.md\n"),
        "{err}"
    );
    let first = f.readme();
    let (code, out, _) = f.overview(&["1-auth-flow"]);
    assert_eq!(
        (code, out.as_str()),
        (0, "plan/specs/1-auth-flow/README.md is up to date\n")
    );
    assert_eq!(f.readme(), first, "the second run changed the file");
    let golden = std::fs::read_to_string(testdata("golden/overview/README.md")).unwrap();
    assert!(
        first == golden,
        "the overview differs from Go's golden:\n{first}"
    );

    let other = Fixture::new();
    other.overview(&["1-auth-flow"]);
    assert!(
        other.readme() == first,
        "a second copy rendered differently"
    );
}

/// Go's TestOverviewKeepsHandEdits: everything outside the generated region is
/// kept byte for byte, and the region is refreshed.
#[test]
fn overview_keeps_hand_edits() {
    let f = Fixture::new();
    f.overview(&["1-auth-flow"]);
    let mut edited = f.readme().replacen(
        "# Auth flow\n",
        "# Auth flow\r\n\r\nA note above, by hand.\r\n",
        1,
    );
    edited = edited.replacen(
        "<!-- About 250 words",
        "Sign-in, written by a person.\n<!-- About 250 words",
        1,
    );
    edited.push_str("\nNotes below, by hand.\n");
    let gen_at = edited.find("<!-- daiku:generated").unwrap();
    let stale = format!(
        "{}{}",
        &edited[..gen_at],
        edited[gen_at..].replacen("## Status", "## Status\n\nstale text", 1)
    );
    std::fs::write(f.path(README), &stale).unwrap();
    let (code, out, _) = f.overview(&["1-auth-flow"]);
    assert!(code == 0 && out.starts_with("wrote "), "{code} {out}");
    let got = f.readme();
    let outside = |s: &str| {
        let start = s.find("<!-- daiku:generated").unwrap();
        let end = s.find("<!-- /daiku:generated").unwrap();
        (s[..start].to_string(), s[end..].to_string())
    };
    assert_eq!(outside(&got), outside(&edited));
    assert!(!got.contains("stale text"), "{got}");
    assert!(!got.contains("narrative is empty"), "{got}");
    assert!(got.contains("## Problems"), "{got}");
}

/// Go's TestOverviewMalformedMarkers: refused, nothing written, and `--check`
/// exits 2.
#[test]
fn overview_malformed_markers() {
    let f = Fixture::new();
    f.overview(&["1-auth-flow"]);
    let dup = f.readme().replacen(
        "<!-- /daiku:generated -->",
        "<!-- daiku:generated again -->\n<!-- /daiku:generated -->",
        1,
    );
    std::fs::write(f.path(README), &dup).unwrap();
    let (code, out, err) = f.overview(&["1-auth-flow"]);
    assert_eq!((code, out.as_str()), (1, ""));
    let want = "plan/specs/1-auth-flow/README.md: duplicated marker <!-- daiku:generated -->";
    assert!(err.contains(want), "{err}");
    assert_eq!(f.readme(), dup, "a refused run changed the file");
    let (code, _, err) = f.overview(&["1-auth-flow", "--check"]);
    assert!(
        code == 2 && err.contains("duplicated marker"),
        "{code} {err}"
    );
    let (code, out, _) = f.overview(&["1-auth-flow", "--stdout"]);
    assert_eq!((code, out.as_str()), (1, ""));
}

/// Go's TestOverviewCheck: 1 when missing or stale, 0 when current, and it never
/// writes.
#[test]
fn overview_check() {
    let f = Fixture::new();
    let (code, out, _) = f.overview(&["1-auth-flow", "--check"]);
    assert!(code == 1 && out.contains("is out of date"), "{code} {out}");
    assert!(!f.path(README).exists(), "--check created the file");
    f.overview(&["1-auth-flow"]);
    let (code, out, _) = f.overview(&["1-auth-flow", "--check"]);
    assert_eq!(
        (code, out.as_str()),
        (0, "plan/specs/1-auth-flow/README.md is up to date\n")
    );
    let before = f.readme();
    let ticket = f.path("plan/specs/1-auth-flow/tickets/T-0007.md");
    let data = std::fs::read_to_string(&ticket).unwrap();
    let changed = data.replacen("title: Rotate signing keys", "title: Rotate all keys", 1);
    std::fs::write(&ticket, changed).unwrap();
    let (code, out, _) = f.overview(&["1-auth-flow", "--check"]);
    assert!(
        code == 1 && out.contains("is out of date — run daiku overview"),
        "{code} {out}"
    );
    assert_eq!(f.readme(), before, "--check changed the file");
}

/// Go's TestOverviewStdout: prints the file and writes nothing.
#[test]
fn overview_stdout() {
    let f = Fixture::new();
    let (code, out, _) = f.overview(&["1-auth-flow", "--stdout"]);
    assert_eq!(code, 0);
    assert!(
        out.starts_with("# Auth flow\n") && out.contains("## Problems"),
        "{out}"
    );
    assert!(!f.path(README).exists(), "--stdout wrote the file");
    f.overview(&["1-auth-flow"]);
    assert!(out == f.readme(), "--stdout differs from the written file");
}

/// Go's TestOverviewArguments: the active change by default; unknown names and
/// bad flags are refused.
#[test]
fn overview_arguments() {
    let f = Fixture::new();
    let (code, _, err) = f.overview(&[]);
    assert!(
        code == 1 && err.contains("no active change"),
        "{code} {err}"
    );
    let (code, _, err) = f.run(&["switch", "1-auth-flow"]);
    assert_eq!(code, 0, "{err}");
    let (code, out, _) = f.overview(&[]);
    assert_eq!(
        (code, out.as_str()),
        (0, "wrote plan/specs/1-auth-flow/README.md\n")
    );
    assert!(
        f.readme()
            .starts_with("# Auth flow\n\n<!-- daiku:narrative -->\n")
    );
    for name in ["nope", "../x", "plan/specs/1-auth-flow"] {
        let (code, _, err) = f.overview(&[name]);
        assert!(
            code == 1 && err.contains("no change named"),
            "{name}: {code} {err}"
        );
    }
    let (code, _, err) = f.overview(&["nope", "--check"]);
    assert!(code == 2 && err.contains("no change named"), "{code} {err}");
    for args in [
        &["--check", "--stdout"][..],
        &["--force"],
        &["1-auth-flow", "2-other-work"],
    ] {
        let (code, _, err) = f.overview(args);
        assert!(code == 1 && err.contains(USAGE), "{args:?}: {code} {err}");
    }
    assert_eq!(f.overview(&["2-other-work"]).0, 0, "another named change");
}
