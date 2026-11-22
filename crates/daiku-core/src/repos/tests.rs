//! Registry tests: decoding against `tests/repos-go.txt` (Go's wording, captured from
//! `v0.2.0` and kept as daiku's expected wording), `load` and `resolve`, and the
//! writes (S-0008).

use super::*;

#[test]
fn decoding_matches_go() {
    let mut n = 0;
    for line in include_str!("../../tests/repos-go.txt").lines() {
        let mut parts = line.splitn(3, '\t');
        let (kind, hex, want) = (
            parts.next().unwrap(),
            parts.next().unwrap(),
            parts.next().unwrap(),
        );
        let input: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let s = gojson::marshal_string;
        let got = if kind == "repos" {
            decode_repos(&input).map(|m| {
                let parts: Vec<_> = m
                    .iter()
                    .map(|(k, r)| {
                        format!(
                            "{}:{{\"remote\":{},\"defaultBranch\":{}}}",
                            s(k),
                            s(&r.remote),
                            s(&r.default_branch)
                        )
                    })
                    .collect();
                format!("{{\"repos\":{{{}}}}}", parts.join(","))
            })
        } else {
            decode_paths(&input).map(|m| {
                let parts: Vec<_> = m
                    .iter()
                    .map(|(k, v)| format!("{}:{}", s(k), s(v)))
                    .collect();
                format!("{{{}}}", parts.join(","))
            })
        };
        let got = got.unwrap_or_else(|e| format!("error: {e}"));
        // Go keeps nil apart from empty; daiku treats them alike.
        let want = want
            .replace("{\"repos\":null}", "{\"repos\":{}}")
            .replace("null", "{}");
        assert_eq!(got, want, "{kind} {}", String::from_utf8_lossy(&input));
        n += 1;
    }
    assert!(n > 25);
}

#[test]
fn load_and_resolve() {
    let t = crate::testutil::Temp::new();
    let dirs = Dirs {
        home: Some("/home/u".into()),
        xdg_config_home: None,
        cwd: None,
    };
    assert_eq!(load(t.path()).unwrap(), Registry::default());
    t.write(
        "REPOS.json",
        r#"{"repos": {"web": {"remote": "w"}, "api": {"remote": "a"}, "lib": {}}}"#,
    );
    t.write(
        ".daiku/paths.json",
        r#"{"api": "~/src/api", "web": "/abs/web"}"#,
    );
    let reg = load(t.path()).unwrap();
    assert_eq!(reg.names(), ["api", "lib", "web"]);
    assert_eq!(
        reg.resolve("api", &dirs).unwrap(),
        PathBuf::from("/home/u/src/api")
    );
    assert_eq!(
        reg.resolve("web", &dirs).unwrap(),
        PathBuf::from("/abs/web")
    );
    assert_eq!(
        reg.resolve("lib", &dirs).unwrap_err().to_string(),
        "daiku: repo \"lib\" has no local path — run `daiku repo path lib <path>`"
    );
    assert_eq!(
        reg.resolve("nope", &dirs).unwrap_err().to_string(),
        "daiku: unknown repo \"nope\" — run `daiku repo add`"
    );
    t.write("REPOS.json", "{\"repos\": 1}");
    assert_eq!(
        load(t.path()).unwrap_err().to_string(),
        format!(
            "daiku: cannot parse {}: json: cannot unmarshal number into Go struct field .repos of type map[string]repos.Repo",
            shown(t.path(), "REPOS.json")
        )
    );
}

// ---- The writes (S-0008 T2): Go's repos_test.go, by behaviour. ----

use crate::testutil::Temp;

fn dirs(home: &str, cwd: Option<&str>) -> Dirs {
    Dirs {
        home: Some(home.into()),
        xdg_config_home: None,
        cwd: cwd.map(Into::into),
    }
}

fn registry_with(name: &str) -> Registry {
    let mut r = Registry::default();
    r.add(name, "https://github.com/org/x", "").unwrap();
    r
}

/// Unix only: the expectations spell paths with `/`, which Windows writes with `\`;
/// the fixture cases cover Windows.
#[cfg(unix)]
#[test]
fn add_splits_committed_and_local() {
    let t = Temp::new();
    let d = dirs("/home/u", Some("/w"));
    let mut r = Registry::default();
    r.add("actions", "git@github.com:org/actions.git", "trunk")
        .unwrap();
    r.set_path("actions", "/work/actions", &d).unwrap();
    r.save(t.path()).unwrap();
    let committed = t.read("REPOS.json");
    assert!(!committed.contains("/work/actions"), "{committed}");
    assert_eq!(
        committed,
        "{\n  \"repos\": {\n    \"actions\": {\n      \"remote\": \"git@github.com:org/actions.git\",\n      \"defaultBranch\": \"trunk\"\n    }\n  }\n}\n"
    );
    assert_eq!(
        t.read(".daiku/paths.json"),
        "{\n  \"actions\": \"/work/actions\"\n}\n"
    );
    let back = load(t.path()).unwrap();
    assert_eq!(
        back.resolve("actions", &d).unwrap(),
        PathBuf::from("/work/actions")
    );
}

#[test]
fn add_defaults_the_branch_and_refuses_bad_input() {
    let mut r = Registry::default();
    r.add("good", "https://github.com/org/good", "").unwrap();
    assert_eq!(r.repos["good"].default_branch, DEFAULT_BRANCH);
    for (name, remote, want) in [
        (
            "good",
            "https://github.com/org/x",
            "daiku: repo \"good\" is already registered",
        ),
        (
            "../x",
            "https://github.com/org/x",
            "daiku: invalid repo name \"../x\" (use lowercase letters, digits, '.', '_' and '-')",
        ),
        (
            "a/b",
            "https://github.com/org/x",
            "daiku: invalid repo name \"a/b\" (use lowercase letters, digits, '.', '_' and '-')",
        ),
        (
            "ext",
            "ext::sh -c x",
            "daiku: remote helpers aren't allowed: \"ext::sh -c x\"",
        ),
        (
            "tok",
            "https://x-access-token:secret@github.com/org/x",
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
        ),
        (
            "tok",
            "https://secret@github.com/org/x",
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
        ),
        ("tok", "", "daiku: a remote is required"),
        (
            "tok",
            "-oProxyCommand=x",
            "daiku: invalid remote \"-oProxyCommand=x\"",
        ),
    ] {
        assert_eq!(
            r.add(name, remote, "").unwrap_err().to_string(),
            want,
            "{name} {remote}"
        );
    }
    assert_eq!(r.repos.len(), 1, "a refused add changed nothing");
}

#[test]
fn check_remote_follows_gos_url_parse() {
    for ok in [
        "https://github.com/org/x",
        "git@github.com:org/x.git",
        "ssh://git@github.com/org/x.git",
        "/local/path/x",
        "file:///srv/x.git",
        "https://github.com:8443/org/x",
        "HTTP://github.com/x",
    ] {
        assert!(check_remote(ok).is_ok(), "{ok}");
    }
    // Go's parser rejects these, so its credentials test never runs on them; the
    // raw userinfo scan still sees the password (N-0015).
    for unparsed in [
        "https://user:pw@exa mple.com/x",
        "https://user:pw@host:port/x",
    ] {
        assert!(url::parse(unparsed).is_none(), "{unparsed}");
        assert!(check_remote(unparsed).is_err(), "{unparsed}");
    }
    for bad in [
        "HTTPS://user@github.com/x",
        "http://tok:x@host/y",
        "https://a%41b:c@host/x",
    ] {
        assert!(check_remote(bad).is_err(), "{bad}");
    }
    // ssh with a user is how git remotes are written; not credentials.
    assert!(check_remote("ssh://git@github.com/org/x").is_ok());
    assert!(check_remote("ssh://user:pw@github.com/org/x").is_err());
}

#[test]
fn a_password_in_any_scheme_is_refused_and_a_user_alone_is_not() {
    for remote in [
        "ssh://u:p@h/x",
        "git://u:p@h/x",
        "ftp://u:p@h/x",
        "ssh://u:@h/x",
        " ssh://u:p@h/x",
        "ssh://u:p@h",
    ] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
            "{remote:?}"
        );
    }
    for ok in ["ssh://git@h/x", "git://git@h/x", "git@github.com:org/x.git"] {
        assert!(check_remote(ok).is_ok(), "{ok}");
    }
    // A refused add leaves the registry as it was.
    let mut r = Registry::default();
    assert!(r.add("x", "ssh://u:p@h/x", "").is_err());
    assert!(r.repos.is_empty());
}

#[test]
fn url_parse_matches_go_for_what_the_registry_reads() {
    let p = url::parse("https://GitHub.com:8443/org/x%20y.git?q=1#f").unwrap();
    assert_eq!(p.scheme, "https");
    assert_eq!(p.host, "GitHub.com:8443");
    assert_eq!(p.hostname(), "GitHub.com");
    assert_eq!(p.path, "/org/x y.git");
    assert!(!p.has_user);
    assert!(url::parse("a b\x01").is_none(), "control characters");
    assert!(
        url::parse("git@github.com:org/x").is_none(),
        "colon in the first segment"
    );
    assert!(url::parse("https://h%zz/x").is_none());
    assert!(url::parse("https://us er@h/x").is_none());
    assert_eq!(url::parse("github.com:org/x").unwrap().scheme, "github.com");
    assert_eq!(url::parse("github.com:org/x").unwrap().host, "");
    assert_eq!(url::parse("https://[::1]:80/x").unwrap().hostname(), "::1");
    assert_eq!(url::parse("").unwrap(), url::Url::default());
}

#[test]
fn set_path_makes_absolute_but_keeps_tilde() {
    let mut r = registry_with("x");
    let d = dirs("/home/u", Some("/w/dir"));
    r.set_path("x", "~/code/x", &d).unwrap();
    assert_eq!(r.paths["x"], "~/code/x");
    assert_eq!(r.resolve("x", &d).unwrap(), PathBuf::from("/home/u/code/x"));
    r.set_path("x", "~", &d).unwrap();
    assert_eq!(r.paths["x"], "~");
    assert_eq!(
        r.set_path("nope", "/x", &d).unwrap_err().to_string(),
        "daiku: unknown repo \"nope\" — run `daiku repo add`"
    );
    assert_eq!(
        r.set_path("x", "", &d).unwrap_err().to_string(),
        "daiku: a path is required"
    );
}

/// AC5: `--path ../x` is joined to the folder daiku runs in and cleaned; `~/` is
/// kept as written.
#[cfg(unix)]
#[test]
fn set_path_joins_the_current_folder() {
    let mut r = registry_with("x");
    let d = dirs("/home/u", Some("/w/dir"));
    for (given, want) in [
        ("../x", "/w/x"),
        ("relative/x", "/w/dir/relative/x"),
        ("./a/../b", "/w/dir/b"),
        ("/abs//y/", "/abs/y"),
    ] {
        r.set_path("x", given, &d).unwrap();
        assert_eq!(r.paths["x"], want, "{given}");
    }
    r.set_path("x", "~/code/x", &d).unwrap();
    assert_eq!(r.paths["x"], "~/code/x");
    // A client that gave core no current folder can't resolve a relative path.
    assert_eq!(
        r.set_path("x", "rel", &dirs("/h", None))
            .unwrap_err()
            .to_string(),
        "getwd: the current folder is unknown"
    );
    assert!(r.set_path("x", "/abs", &dirs("/h", None)).is_ok());
}

#[test]
fn resolve_errors() {
    let r = registry_with("x");
    let d = dirs("/h", None);
    assert!(
        r.resolve("x", &d)
            .unwrap_err()
            .to_string()
            .contains("no local path")
    );
    assert!(
        r.resolve("nope", &d)
            .unwrap_err()
            .to_string()
            .contains("unknown repo")
    );
}

#[test]
fn a_corrupt_registry_is_left_alone() {
    let t = Temp::new();
    t.write("REPOS.json", "{");
    let err = load(t.path()).unwrap_err().to_string();
    assert!(err.contains("cannot parse"), "{err}");
    assert_eq!(t.read("REPOS.json"), "{");
}

/// `REPOS.json` holding `ok` and `bad`, each with a plain remote.
fn registry_json(bad: &str) -> String {
    let entry = r#"{"remote": "https://example.invalid/r.git", "defaultBranch": "main"}"#;
    format!(
        "{{\"repos\": {{\"ok\": {entry}, {}: {entry}}}}}\n",
        serde_json::to_string(bad).unwrap()
    )
}

/// N-0004 AC1: a hand-edited name that `add` would refuse fails the load, with the
/// file named, and nothing is written.
#[test]
fn load_refuses_a_bad_name() {
    for bad in ["../outside", "a/b", ".hidden", "Web", ""] {
        let t = crate::testutil::Temp::new();
        let json = registry_json(bad);
        t.write("REPOS.json", &json);
        let err = load(t.path()).unwrap_err();
        assert!(
            matches!(&err, Error::InvalidRepoName { name, path: Some(_) } if name == bad),
            "{bad:?}: {err:?}"
        );
        let want = format!(
            "daiku: {}: invalid repo name {} (use lowercase letters, digits, '.', '_' and '-')",
            t.path()
                .join("REPOS.json")
                .to_string_lossy()
                .replace('\\', "/"),
            crate::gofmt::quote(bad)
        );
        assert_eq!(err.to_string(), want, "{bad:?}");
        assert_eq!(t.read("REPOS.json"), json, "{bad:?}: left as it was");
        assert!(!t.exists(".daiku/paths.json"), "{bad:?}: nothing written");
    }
}

/// N-0004 AC5: a parse error comes before the name check, and the first bad name
/// in sorted order is the one reported.
#[test]
fn load_reports_parse_errors_first_then_the_first_bad_name() {
    let t = crate::testutil::Temp::new();
    t.write("REPOS.json", "{\"repos\": {\"../x\": {}, ");
    let err = load(t.path()).unwrap_err();
    assert!(matches!(err, Error::StateParse { .. }), "{err:?}");

    let entry = r#"{"remote": "https://example.invalid/r.git"}"#;
    t.write(
        "REPOS.json",
        &format!("{{\"repos\": {{\"Zed\": {entry}, \"ok\": {entry}, \"Abe\": {entry}}}}}"),
    );
    match load(t.path()).unwrap_err() {
        Error::InvalidRepoName { name, .. } => assert_eq!(name, "Abe"),
        other => panic!("{other:?}"),
    }
}

/// N-0004 AC4: `add` still says nothing about a file.
#[test]
fn add_refuses_a_bad_name_without_a_path() {
    let err = Registry::default()
        .add("../x", "https://github.com/org/x", "")
        .unwrap_err();
    assert!(
        matches!(err, Error::InvalidRepoName { path: None, .. }),
        "{err:?}"
    );
}

#[cfg(unix)]
#[test]
fn save_refuses_symlinks() {
    let (t, outside) = (Temp::new(), Temp::new());
    std::os::unix::fs::symlink(outside.p("r.json"), t.p("REPOS.json")).unwrap();
    let err = registry_with("x").save(t.path()).unwrap_err().to_string();
    assert!(err.contains("is a symlink"), "{err}");
    assert!(!outside.exists("r.json"), "nothing was written through it");
}

/// N-0008: a symlinked `.daiku/paths.json` is refused before `REPOS.json` is
/// touched, so a refused save writes nothing.
#[cfg(unix)]
#[test]
fn save_checks_both_files_before_writing() {
    let (t, outside) = (Temp::new(), Temp::new());
    t.write("REPOS.json", "{}\n");
    std::fs::create_dir_all(t.p(".daiku")).unwrap();
    std::os::unix::fs::symlink(outside.p("p.json"), t.p(".daiku/paths.json")).unwrap();
    let err = registry_with("x").save(t.path()).unwrap_err().to_string();
    assert!(err.contains("is a symlink"), "{err}");
    assert_eq!(t.read("REPOS.json"), "{}\n", "REPOS.json is untouched");
    assert!(!outside.exists("p.json"));
}

/// N-0005 AC2, AC4: what `redact_remote` masks and what it leaves alone.
#[test]
fn redact_remote_masks_userinfo_for_display() {
    for (remote, want) in [
        // http(s): any userinfo.
        (
            "https://x-access-token:s3cr3t@github.com/org/api.git",
            "https://***@github.com/org/api.git",
        ),
        (
            "https://tok@github.com/org/api.git",
            "https://***@github.com/org/api.git",
        ),
        (
            "http://user:pa%40ss@host:8080/x?y=1",
            "http://***@host:8080/x?y=1",
        ),
        ("HTTPS://u:p@Host/x", "HTTPS://***@Host/x"),
        // Userinfo url::parse rejects, and a '#' before the '@'.
        ("https://u:p w@host/x", "https://***@host/x"),
        ("https://u:p#ss@host/x", "https://***@host/x"),
        // Leading whitespace is kept, and doesn't hide the scheme.
        (" https://u:p@host/x", " https://***@host/x"),
        ("\thttps://u:p@host/x", "\thttps://***@host/x"),
        // Two '@'s: masked up to the last.
        ("https://a@b:c@host/x", "https://***@host/x"),
        // Other schemes: only with a password.
        (
            "ssh://git@github.com/org/api.git",
            "ssh://git@github.com/org/api.git",
        ),
        ("ssh://u:p@host/x", "ssh://***@host/x"),
        ("git://u:p@host/x", "git://***@host/x"),
        // Unchanged.
        ("git@github.com:org/api.git", "git@github.com:org/api.git"),
        (
            "https://github.com/org/api.git",
            "https://github.com/org/api.git",
        ),
        ("https://github.com/org/a@b", "https://github.com/org/a@b"),
        ("/srv/git/api.git", "/srv/git/api.git"),
        ("C:\\src\\api", "C:\\src\\api"),
        ("file:///srv/git/api.git", "file:///srv/git/api.git"),
        // `1x` isn't a valid scheme, so the fallback masks it (N-0018 D5).
        ("1x://u:p@host/x", "1x://***@host/x"),
        // An unreadable scheme is masked, whatever hides it (N-0018).
        ("-ssh://u:p@h/x", "-ssh://***@h/x"),
        ("ext::https://tok@h/x", "ext::https://***@h/x"),
        ("\u{1}https://tok@h/x", "\u{1}https://***@h/x"),
        ("-ssh://git@h/x", "-ssh://***@h/x"),
        ("", ""),
    ] {
        assert_eq!(redact_remote(remote), want, "{remote}");
    }
}

/// N-0008 D1, AC1: the archived fields are read, written only when set, and kept
/// by every later save.
#[test]
fn archived_fields_round_trip_and_survive_saves() {
    let t = crate::testutil::Temp::new();
    let live = "{\n  \"repos\": {\n    \"api\": {\n      \"remote\": \"https://example.invalid/api.git\",\n      \"defaultBranch\": \"main\"\n    }\n  }\n}\n";
    t.write("REPOS.json", live);
    let mut reg = load(t.path()).unwrap();
    reg.save(t.path()).unwrap();
    assert_eq!(
        t.read("REPOS.json"),
        live,
        "a live registry saves byte for byte"
    );

    reg.archive("api", "old\nrepo").unwrap();
    reg.save(t.path()).unwrap();
    let saved = t.read("REPOS.json");
    assert!(
        saved.contains("\"defaultBranch\": \"main\",\n      \"archived\": true,\n      \"archivedReason\": \"old repo\""),
        "{saved}"
    );

    // A later `repo add` of another repo and a `repo path` keep the state.
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut reg = load(t.path()).unwrap();
    reg.add("web", "https://example.invalid/web.git", "")
        .unwrap();
    reg.set_path("api", "/srv/api", &d).unwrap();
    reg.save(t.path()).unwrap();
    let back = load(t.path()).unwrap();
    assert!(back.is_archived("api") && !back.is_archived("web"));
    assert_eq!(back.repos["api"].archived_reason, "old repo");
}

/// N-0008 R1–R3: archive and unarchive, and their refusals.
#[test]
fn archive_and_unarchive() {
    let mut r = registry_with("api");
    assert_eq!(
        r.unarchive("api").unwrap_err().to_string(),
        "daiku: repo \"api\" isn't archived"
    );
    r.archive("api", "  ").unwrap();
    assert!(r.is_archived("api"));
    assert_eq!(
        r.repos["api"].archived_reason, "",
        "a blank reason is no reason"
    );
    assert_eq!(
        r.archive("api", "x").unwrap_err().to_string(),
        "daiku: repo \"api\" is already archived"
    );
    r.unarchive("api").unwrap();
    assert!(!r.is_archived("api"));
    for err in [r.archive("nope", ""), r.unarchive("nope")] {
        assert!(matches!(err, Err(Error::UnknownRepo { .. })), "{err:?}");
    }
    assert!(!r.is_archived("nope"));
}

/// N-0008 D1: a `null` archived field reads as live, and a wrong type is an error.
#[test]
fn archived_decodes_like_go() {
    let entry =
        |v: &str| format!("{{\"repos\": {{\"a\": {{\"remote\": \"r\", \"archived\": {v}}}}}}}");
    assert!(!decode_repos(entry("null").as_bytes()).unwrap()["a"].archived);
    assert!(decode_repos(entry("true").as_bytes()).unwrap()["a"].archived);
    assert!(decode_repos(entry("\"yes\"").as_bytes()).is_err());
}

#[test]
fn names_are_folder_safe() {
    for ok in ["a", "api", "a.b", "a_b", "a-b", "0x", "a1.2_3-4"] {
        assert!(NAME_RE.is_match(ok), "{ok}");
    }
    for bad in [
        "", ".", "..", "../x", "a/b", "A", "-a", ".a", "a b", "a\\b", "a\nb",
    ] {
        assert!(!NAME_RE.is_match(bad), "{bad:?}");
    }
}

// ---- Doctor, clone, env flags (S-0008 T3) ----

use crate::env::Output;
use crate::fake::ScriptedProcess;

fn ok_out(stdout: &str) -> Output {
    Output {
        code: Some(0),
        stdout: stdout.as_bytes().to_vec(),
        stderr: Vec::new(),
    }
}

fn failed(code: i32, stderr: &str) -> Output {
    Output {
        code: Some(code),
        stdout: Vec::new(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

#[cfg(unix)]
fn calls(p: &ScriptedProcess) -> Vec<Vec<String>> {
    p.calls.borrow().iter().map(|c| c.args.clone()).collect()
}

#[test]
fn normalize_remote_makes_spellings_equal() {
    let want = "github.com/org/x";
    for input in [
        "git@github.com:org/x.git",
        "git@github.com:org/x",
        "https://github.com/org/x",
        "https://github.com/org/x.git",
        "https://GitHub.com/org/x/",
        "ssh://git@github.com/org/x.git",
        "  https://github.com/org/x\n",
        "https://github.com:8443/org/x",
        "github.com:org/x",
    ] {
        assert_eq!(normalize_remote(input), want, "{input:?}");
    }
    assert_ne!(
        normalize_remote("https://github.com/org/x"),
        normalize_remote("https://github.com/org/y")
    );
    assert_eq!(normalize_remote("/local/path/x.git"), "/local/path/x");
}

/// N-0005 AC1, AC3: a credentialed origin is masked in the mismatch line, and
/// still compares by its raw value.
#[test]
fn doctor_masks_credentials_in_a_mismatch() {
    let t = Temp::new();
    std::fs::create_dir_all(t.p("api")).unwrap();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let at = t.p("api").to_string_lossy().into_owned();
    for (origin, secret) in [
        (
            "https://x-access-token:s3cr3t@github.com/org/other.git",
            "s3cr3t",
        ),
        ("https://user:pa%40ss@github.com/org/other.git", "pa%40ss"),
        ("https://tok@github.com/org/other.git", "tok"),
    ] {
        let mut r = Registry::default();
        r.add("api", "https://github.com/org/api", "").unwrap();
        r.paths.insert("api".into(), at.clone());
        let p = ScriptedProcess::default();
        p.reply(ok_out(&format!("{origin}\n")));
        let got = r.doctor(&d, &p);
        assert_eq!(got.len(), 1, "{origin}: {got:?}");
        assert_eq!(
            got[0].detail,
            format!(
                "remote mismatch: {at} has origin https://***@github.com/org/other.git, registered as https://github.com/org/api"
            )
        );
        assert!(!got[0].detail.contains(secret), "{origin}");
    }

    // Credentials aside, the same repo: not a mismatch.
    let mut r = Registry::default();
    r.add("api", "https://github.com/org/api", "").unwrap();
    r.paths.insert("api".into(), at.clone());
    let p = ScriptedProcess::default();
    p.reply(ok_out("https://tok@github.com/org/api.git\n"));
    assert_eq!(r.doctor(&d, &p), []);
}

/// N-0008 security review: an archived repo whose checkout is still on disk is
/// still checked for a swapped remote; everything else about it is skipped.
#[test]
fn doctor_still_checks_an_archived_checkout_for_a_swapped_remote() {
    let t = Temp::new();
    for d in ["kept", "notgit"] {
        std::fs::create_dir_all(t.p(d)).unwrap();
    }
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let at = |d: &str| t.p(d).to_string_lossy().into_owned();
    let mut r = Registry::default();
    for name in ["a-nopath", "b-gone", "c-notgit", "d-kept"] {
        r.add(name, "https://evil.invalid/x.git", "").unwrap();
        r.archive(name, "").unwrap();
    }
    r.paths.insert("b-gone".into(), at("gone"));
    r.paths.insert("c-notgit".into(), at("notgit"));
    r.paths.insert("d-kept".into(), at("kept"));
    let p = ScriptedProcess::default();
    // get-url for c-notgit (fails), then d-kept (the real origin).
    p.reply(failed(128, "fatal: not a git repository"));
    p.reply(ok_out("https://github.com/org/x.git\n"));
    let got = r.doctor(&d, &p);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].repo, "d-kept");
    assert!(
        got[0].detail.starts_with("remote mismatch:"),
        "{}",
        got[0].detail
    );
}

#[test]
fn shell_quote_keeps_safe_words_and_quotes_the_rest() {
    assert_eq!(shell_quote("/a/b-c_d.e"), "/a/b-c_d.e");
    assert_eq!(shell_quote("my repo's"), "'my repo'\\''s'");
    assert_eq!(shell_quote(""), "''");
    assert_eq!(shell_quote("a$b"), "'a$b'");
}

/// The shell reads a quoted path back as the original.
#[cfg(unix)]
#[test]
fn shell_quote_round_trips_through_sh() {
    for path in ["/tmp/my repo's", "/tmp/a$b `c` \"d\"", "/tmp/ünï\u{2028}x"] {
        let out = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("printf %s {}", shell_quote(path)))
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), path);
    }
}

#[test]
fn doctor_finds_each_problem() {
    let t = Temp::new();
    for d in ["good", "notgit", "other"] {
        std::fs::create_dir_all(t.p(d)).unwrap();
    }
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = Registry::default();
    for (name, remote) in [
        ("a-good", "git@github.com:org/x.git"),
        ("b-unmapped", "https://github.com/org/x"),
        ("c-missing", "https://github.com/org/x"),
        ("d-notgit", "https://github.com/org/x"),
        ("e-mismatch", "https://github.com/org/elsewhere"),
    ] {
        r.add(name, remote, "").unwrap();
    }
    let at = |d: &str| t.p(d).to_string_lossy().into_owned();
    r.paths.insert("a-good".into(), at("good"));
    r.paths.insert("c-missing".into(), at("gone"));
    r.paths.insert("d-notgit".into(), at("notgit"));
    r.paths.insert("e-mismatch".into(), at("other"));
    let p = ScriptedProcess::default();
    // One `git remote get-url origin` per checkout, in name order: good, notgit, other.
    p.reply(ok_out("https://github.com/org/x.git\n"));
    p.reply(failed(128, "fatal: not a git repository"));
    p.reply(ok_out("https://github.com/org/x\n"));
    let got = r.doctor(&d, &p);
    let by: std::collections::BTreeMap<_, _> = got
        .iter()
        .map(|p| (p.repo.as_str(), p.detail.as_str()))
        .collect();
    assert_eq!(by.len(), 4, "{got:?}");
    assert!(!by.contains_key("a-good"));
    assert_eq!(by["b-unmapped"], "no local path");
    assert_eq!(by["c-missing"], format!("missing on disk: {}", at("gone")));
    assert_eq!(
        by["d-notgit"],
        format!("not a git checkout with an origin: {}", at("notgit"))
    );
    assert_eq!(
        by["e-mismatch"],
        format!(
            "remote mismatch: {} has origin https://github.com/org/x, registered as https://github.com/org/elsewhere",
            at("other")
        )
    );
    let calls = p.calls.borrow();
    assert_eq!(calls.len(), 3);
    assert!(
        calls
            .iter()
            .all(|c| c.program == "git" && c.args == ["remote", "get-url", "origin"])
    );
    assert_eq!(calls[0].dir.as_deref(), Some(t.p("good").as_path()));
    // Problems come back in name order.
    assert_eq!(
        got.iter().map(|p| p.repo.as_str()).collect::<Vec<_>>(),
        ["b-unmapped", "c-missing", "d-notgit", "e-mismatch"]
    );
}

/// Unix only: the expectations spell paths with `/`, which Windows writes with `\`;
/// the fixture cases cover Windows.
#[cfg(unix)]
#[test]
fn clone_runs_git_and_records_the_path() {
    let t = Temp::new();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = registry_with("x");
    let p = ScriptedProcess::default();
    p.reply(ok_out(""));
    r.clone_repo("x", "nested/deeper/x", &d, &p).unwrap();
    let want = t.p("nested/deeper/x").to_string_lossy().into_owned();
    assert_eq!(r.paths["x"], want);
    assert_eq!(
        calls(&p),
        [[
            "clone",
            "--quiet",
            "--",
            "https://github.com/org/x",
            want.as_str()
        ]]
    );
    assert!(t.exists("nested/deeper"), "the parent folders are created");
}

#[test]
fn clone_refuses_an_existing_destination() {
    let t = Temp::new();
    let d = dirs("/h", Some("/w"));
    let mut r = registry_with("x");
    let dest = t.path().to_string_lossy().into_owned();
    let p = ScriptedProcess::default();
    assert_eq!(
        r.clone_repo("x", &dest, &d, &p).unwrap_err().to_string(),
        format!("daiku: {dest} already exists")
    );
    assert!(p.calls.borrow().is_empty());
    assert!(!r.paths.contains_key("x"));
}

/// A failed clone must not leave doctor believing the repo has a path.
#[test]
fn a_failed_clone_records_no_path() {
    let t = Temp::new();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = registry_with("x");
    let p = ScriptedProcess::default();
    p.reply(failed(128, "fatal: repository not found\n"));
    let err = r.clone_repo("x", "x", &d, &p).unwrap_err().to_string();
    assert_eq!(
        err,
        "daiku: cloning x: exit status 128: fatal: repository not found"
    );
    assert!(!r.paths.contains_key("x"));
    // Without git on PATH, Go says so, with its trailing empty output.
    let p = ScriptedProcess::default();
    let err = r.clone_repo("x", "y", &d, &p).unwrap_err().to_string();
    let path = if cfg!(windows) { "%PATH%" } else { "$PATH" };
    assert_eq!(
        err,
        format!("daiku: cloning x: exec: \"git\": executable file not found in {path}: ")
    );
}

#[test]
fn clone_refuses_a_hostile_committed_remote() {
    let t = Temp::new();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = Registry::default();
    r.repos.insert(
        "x".into(),
        Repo {
            remote: "--upload-pack=touch pwned".into(),
            default_branch: "main".into(),
            ..Repo::default()
        },
    );
    let p = ScriptedProcess::default();
    assert_eq!(
        r.clone_repo("x", "x", &d, &p).unwrap_err().to_string(),
        "daiku: invalid remote \"--upload-pack=touch pwned\""
    );
    assert!(p.calls.borrow().is_empty());
    assert_eq!(
        r.clone_repo("nope", "x", &d, &p).unwrap_err().to_string(),
        "daiku: unknown repo \"nope\" — run `daiku repo add`"
    );
}

#[test]
fn clone_refuses_a_committed_remote_with_a_password() {
    let t = Temp::new();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = Registry::default();
    r.repos.insert(
        "x".into(),
        Repo {
            remote: "ssh://u:p@h/x".into(),
            default_branch: "main".into(),
            ..Repo::default()
        },
    );
    let p = ScriptedProcess::default();
    assert_eq!(
        r.clone_repo("x", "x", &d, &p).unwrap_err().to_string(),
        "daiku: remote has credentials in it, and REPOS.json is committed — remove them"
    );
    assert!(p.calls.borrow().is_empty());
}

#[test]
fn clone_refuses_a_committed_control_character_remote_masked() {
    let t = Temp::new();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = Registry::default();
    r.repos.insert(
        "x".into(),
        Repo {
            remote: "\u{1}https://tok@h/x".into(),
            default_branch: "main".into(),
            ..Repo::default()
        },
    );
    let p = ScriptedProcess::default();
    let err = r.clone_repo("x", "x", &d, &p).unwrap_err();
    assert!(matches!(&err, Error::InvalidRemote { remote } if remote == "\u{1}https://***@h/x"));
    assert!(!err.to_string().contains("tok@"));
    assert!(p.calls.borrow().is_empty());
}

#[test]
fn clone_refuses_a_committed_http_remote_with_a_token_user() {
    let t = Temp::new();
    let d = dirs("/h", Some(t.path().to_str().unwrap()));
    let mut r = Registry::default();
    r.repos.insert(
        "x".into(),
        Repo {
            remote: "https://tok@h b/x".into(),
            default_branch: "main".into(),
            ..Repo::default()
        },
    );
    let p = ScriptedProcess::default();
    assert_eq!(
        r.clone_repo("x", "x", &d, &p).unwrap_err().to_string(),
        "daiku: remote has credentials in it, and REPOS.json is committed — remove them"
    );
    assert!(p.calls.borrow().is_empty());
}

#[test]
fn env_flags_quote_and_refuse_partial_lists() {
    let t = Temp::new();
    std::fs::create_dir_all(t.p("plain")).unwrap();
    std::fs::create_dir_all(t.p("my repo's")).unwrap();
    let d = dirs("/h", None);
    let mut r = Registry::default();
    for name in ["plain", "spaced", "unmapped", "gone"] {
        r.add(name, &format!("https://github.com/org/{name}"), "")
            .unwrap();
    }
    let at = |d: &str| t.p(d).to_string_lossy().into_owned();
    r.paths.insert("plain".into(), at("plain"));
    r.paths.insert("spaced".into(), at("my repo's"));
    r.paths.insert("gone".into(), at("gone"));
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let flags = r.env_flags(&names(&["plain", "spaced"]), &d).unwrap();
    assert_eq!(
        flags,
        format!(
            "--add-dir {} --add-dir {}",
            shell_quote(&at("plain")),
            shell_quote(&at("my repo's"))
        )
    );
    assert_eq!(r.env_flags(&[], &d).unwrap(), "");
    for name in ["unmapped", "gone", "nope"] {
        assert!(
            r.env_flags(&names(&["plain", name]), &d).is_err(),
            "{name}: an error, not a partial list"
        );
    }
    assert_eq!(
        r.env_flags(&names(&["gone"]), &d).unwrap_err().to_string(),
        format!(
            "daiku: repo \"gone\" is missing on disk: {} — run `daiku repo doctor`",
            at("gone")
        )
    );
}

/// Whether Go's `repo add` refuses each remote, as the Go CLI at 5aa7a78 answered
/// (S-0008 review: an 11,198-remote comparison, reduced to its distinguishing
/// classes). `true` means refused.
#[test]
fn check_remote_agrees_with_go_on_the_distinguishing_classes() {
    let go: &[(&str, bool)] = &[
        ("https://u@h/x", true),
        ("https://u:p@h/x", true),
        ("https://:p@h/x", true),
        ("https://@h/x", true),
        ("https://u:@h/x", true),
        ("https://u:p:q@h/x", true),
        ("https://a%40b:c@h/x", true),
        ("https://u@v@h/x", true),
        ("https://u!$&'()*+,;=@h/x", true),
        ("https://u/x@h/y", false),
        ("HTTPS://U:P@H/x", true),
        ("HtTp://u@h/x", true),
        ("git://u@h/x", false),
        ("file://u@h/x", false),
        ("https://u@[::1]/x", true),
        ("https://u@[::1]:80/x", true),
        ("https://u@h:80/x", true),
        ("https://u@h:/x", true),
        ("https://u@h<x/x", true),
        ("https://u@/x", true),
        ("https://u@ü/x", true),
        ("https:u@h/x", false),
        ("https:/u@h/x", false),
        ("https:///u@h/x", false),
        ("//u:p@h/x", false),
        ("git@github.com:org/x.git", false),
        ("u:p@h:o/r", false),
        ("/local/x", false),
        ("x.git", false),
        ("https://h/x?u@h", false),
        ("https://u@h?x", true),
        ("https://u@h#x", true),
        ("https://u@h/x ", true),
        ("", true),
        ("-x", true),
        ("--upload-pack=x", true),
        ("ext::sh -c x", true),
        ("fd::3", true),
    ];
    for (remote, refused) in go {
        assert_eq!(check_remote(remote).is_err(), *refused, "{remote:?}");
    }
}

/// One place the port is stricter than Go, kept on purpose (S-0008 review,
/// finding 1; `a_password_in_any_scheme_is_refused_unlike_go` (N-0015) and
/// `a_user_in_an_unparsed_http_remote_is_refused_unlike_go` (N-0017) are
/// others). Go's `url.Parse` rejects a percent-escaped ASCII character in a
/// host (`%41`), and `checkRemote` only looks for credentials when the parse
/// succeeds, so Go accepts these and writes the credentials into the committed
/// `REPOS.json`. The port refuses them.
#[test]
fn a_credentialed_remote_with_an_escaped_ascii_host_is_refused_unlike_go() {
    for remote in [
        "https://u:p@h%41/x",
        "https://u@h%41",
        "HTTPS://:p@h%41/x.git",
        "http://a%40b:c@h%41/",
    ] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
            "{remote}"
        );
    }
    // Without credentials the same host is fine, as in Go.
    assert!(check_remote("https://h%41/x").is_ok());
}

/// Where the port refuses more than Go, on purpose (N-0015). Go's `checkRemote`
/// looks for credentials only in an http(s) URL its parser accepts. The port also
/// refuses a password in the userinfo of any `scheme://` remote, by the scan
/// `redact_remote` uses. `https://h#u:p@h` has its `@` after a `#`, so Go reads
/// `u:p@h` as a fragment; the scan ends the authority only at `/`.
#[test]
fn a_password_in_any_scheme_is_refused_unlike_go() {
    for remote in ["ssh://u:p@h/x", "ftp://u:p@h/x", "https://h#u:p@h"] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
            "{remote}"
        );
    }
}

/// Where the port refuses more than Go, on purpose (N-0017). Go's parser rejects
/// each of these http(s) URLs (a bad escape, a space or other character in the
/// user or host, a bad port, an unclosed bracket, a leading space, a trailing
/// newline), and `checkRemote` only looks for credentials after a successful
/// parse, so Go never checks them. The port refuses what `redact_remote` masks:
/// any userinfo of an http(s) remote, whether or not the URL parses.
#[test]
fn a_user_in_an_unparsed_http_remote_is_refused_unlike_go() {
    for remote in [
        "https://a%zzb@h/x",
        "https://u v@h/x",
        "https://ü@h/x",
        "https://u@[::1/x",
        "https://u@h:x/x",
        "https://u@h b/x",
        "https://u@h{x/x",
        "https://u@h%zz/x",
        "https://u@h%/x",
        "https://u@h\u{7f}/x",
        " https://u@h/x",
        "https://u@h/x\n",
    ] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
            "{remote:?}"
        );
    }
}

/// N-0017: any http(s) userinfo is refused, parsed or not, and an `@` with none
/// in the authority is not userinfo.
#[test]
fn an_http_user_is_refused_whether_or_not_the_url_parses() {
    for remote in [
        " https://tok@h/x",
        "https://tok@h b/x",
        "HTTPS://tok@h b/x",
        "https://u v@h/x",
        "https://@h b/x",
        "https://u:p@h b/x",
        // The `?` before the first `/` doesn't hide the `@`, as in `redact_remote`.
        "https://h?u@h/x",
    ] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
            "{remote:?}"
        );
    }
    for ok in [
        "https://h b/x",
        "https://h/x?u@h",
        "https:///u@h/x",
        "ssh://git@h/x",
        "git://u@h/x",
    ] {
        assert!(check_remote(ok).is_ok(), "{ok}");
    }
    // A refused add leaves the registry as it was.
    let mut r = Registry::default();
    assert!(r.add("x", "https://tok@h b/x", "").is_err());
    assert!(r.repos.is_empty());
}

/// Where the port refuses more than Go, on purpose (N-0018). Go accepts a remote
/// holding an ASCII control character, and one led by a control character hides
/// its scheme from the credentials check. The port refuses it, last, with the
/// remote masked.
#[test]
fn a_control_character_in_a_remote_is_refused_unlike_go() {
    for (remote, want) in [
        (
            "\u{1}https://tok@h/x",
            "daiku: invalid remote \"\\x01https://***@h/x\"",
        ),
        (
            "https://h/x\u{1}",
            "daiku: invalid remote \"https://h/x\\x01\"",
        ),
        ("\thttps://h/x", "daiku: invalid remote \"\\thttps://h/x\""),
        ("https://h/x\n", "daiku: invalid remote \"https://h/x\\n\""),
    ] {
        let got = check_remote(remote).unwrap_err().to_string();
        assert_eq!(got, want, "{remote:?}");
        assert!(!got.contains("tok@"), "{remote:?}");
    }
    // A readable http(s) userinfo still gets the credentials message.
    for remote in ["https://tok@h\u{1}/x", "\thttps://tok@h/x"] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            "daiku: remote has credentials in it, and REPOS.json is committed — remove them",
            "{remote:?}"
        );
    }
    let mut r = Registry::default();
    assert!(r.add("x", "\u{1}https://tok@h/x", "").is_err());
    assert!(r.repos.is_empty());
}

/// N-0018: the refusals that run before the credentials check mask the remote.
#[test]
fn refusals_before_the_credentials_check_mask_the_remote() {
    for (remote, want) in [
        ("-ssh://u:p@h/x", "daiku: invalid remote \"-ssh://***@h/x\""),
        ("-ssh://git@h/x", "daiku: invalid remote \"-ssh://***@h/x\""),
        (
            "-https://tok@h/x",
            "daiku: invalid remote \"-https://***@h/x\"",
        ),
        (
            "ext::https://tok@h/x",
            "daiku: remote helpers aren't allowed: \"ext::https://***@h/x\"",
        ),
        // Without userinfo the message is as it was.
        ("-x", "daiku: invalid remote \"-x\""),
        (
            "--upload-pack=x",
            "daiku: invalid remote \"--upload-pack=x\"",
        ),
    ] {
        assert_eq!(
            check_remote(remote).unwrap_err().to_string(),
            want,
            "{remote}"
        );
    }
}

#[test]
fn mask_userinfo_masks_every_authority_after_a_scheme_separator() {
    for (remote, want) in [
        (
            "ext::sh -c \"git clone https://tok@h/x; git clone ssh://u:p@g/y\"",
            "ext::sh -c \"git clone https://***@h/x; git clone ssh://***@g/y\"",
        ),
        ("a://u@h/x b://v@h", "a://***@h/x b://***@h"),
        ("git@github.com:org/x.git", "git@github.com:org/x.git"),
        ("https://h/x", "https://h/x"),
        ("-ssh://h", "-ssh://h"),
        ("https://h/a@b", "https://h/a@b"),
        ("https://a@b@h/x", "https://***@h/x"),
        ("", ""),
    ] {
        assert_eq!(mask_userinfo(remote), want, "{remote}");
    }
}
