//! daiku's settings: defaults, then the global file, then the project file (R6,
//! D7). A port of Go's `internal/core/settings` and `internal/settings`.

use std::path::Path;

use serde_json::{Map, Value};

use crate::env::Dirs;
use crate::{Error, gofmt, gojson};

/// The effective settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    pub base_source: String,
    pub gates: String,
    pub source_dir: String,
    pub adr_dir: String,
    pub sync_file: String,
    pub overview: String,
}

impl Default for Settings {
    /// Go's `DefaultSettings`.
    fn default() -> Settings {
        Settings {
            base_source: String::new(),
            gates: "block".into(),
            source_dir: "plan/00_intake".into(),
            adr_dir: "plan/04_architecture/adr".into(),
            sync_file: "sync.md".into(),
            overview: "auto".into(),
        }
    }
}

/// The settings with the raw layers they came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Loaded {
    pub global: Map<String, Value>,
    pub project: Map<String, Value>,
    pub effective: Settings,
    /// The effective `vcs`: `git` (the default) or `none`.
    pub vcs: String,
}

/// The global settings file: `$XDG_CONFIG_HOME/daiku/settings.json` when it's set
/// and not empty, else `<home>/.config/daiku/settings.json`, on every OS (N9). The
/// part daiku adds is joined with `/`, so it prints the same on Windows.
pub fn global_path(dirs: &Dirs) -> String {
    let base = match &dirs.xdg_config_home {
        Some(x) if !x.is_empty() => clean_base(&x.to_string_lossy()),
        _ => match &dirs.home {
            Some(h) if !h.as_os_str().is_empty() => {
                let home = clean_base(&h.to_string_lossy());
                format!("{}/.config", home.trim_end_matches('/'))
            }
            _ => return "daiku/settings.json".into(),
        },
    };
    format!("{}/daiku/settings.json", base.trim_end_matches('/'))
}

/// A folder from the environment, cleaned as Go's `filepath.Join` cleans it and
/// slash-separated. On Windows a `\\server\share` (UNC) prefix written by the user
/// is kept, as Go's Windows `Clean` keeps the volume.
fn clean_base(raw: &str) -> String {
    let slashed = gofmt::to_slash(raw);
    match slashed.strip_prefix("//") {
        Some(rest) if cfg!(windows) && !rest.is_empty() && !rest.starts_with('/') => {
            format!("//{}", gofmt::clean(rest).trim_start_matches('/'))
        }
        _ => gofmt::clean(&slashed),
    }
}

/// The project settings file for the workspace at `root`, as Go prints it
/// (`.daiku/settings.json` when `root` is `.`).
pub fn project_path(root: &Path) -> String {
    gofmt::join(&[
        gofmt::to_slash(&root.to_string_lossy()).as_str(),
        ".daiku/settings.json",
    ])
}

/// Go's `ReadIfExists`: the file's JSON object, an empty map when it's missing.
pub fn read_if_exists(path: &str) -> Result<Map<String, Value>, Error> {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(source) => {
            let op = if source.kind() == std::io::ErrorKind::IsADirectory {
                "read"
            } else {
                "open"
            };
            return Err(Error::Io {
                op,
                path: path.to_string(),
                source,
            });
        }
    };
    match gojson::parse(&data) {
        Err(detail) => Err(Error::SettingsParse {
            path: path.to_string(),
            detail,
        }),
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => Err(Error::SettingsNotObject {
            path: path.to_string(),
        }),
    }
}

/// Go's `Merge`: the layers in order over the defaults (a present but empty string
/// still overrides; `null` and non-string values don't), then validation.
pub fn merge(layers: &[&Map<String, Value>]) -> Result<Settings, Error> {
    let mut s = Settings::default();
    for layer in layers {
        for (key, value) in layer.iter() {
            let Value::String(v) = value else { continue };
            let slot = match key.as_str() {
                "baseSource" => &mut s.base_source,
                "gates" => &mut s.gates,
                "sourceDir" => &mut s.source_dir,
                "adrDir" => &mut s.adr_dir,
                "syncFile" => &mut s.sync_file,
                "overview" => &mut s.overview,
                _ => continue,
            };
            *slot = v.clone();
        }
    }
    validate(&s)?;
    vcs_of(layers)?;
    Ok(s)
}

fn validate(s: &Settings) -> Result<(), Error> {
    if !matches!(s.gates.as_str(), "block" | "prompt" | "auto") {
        return Err(Error::InvalidGates {
            value: s.gates.clone(),
        });
    }
    // Go walks a map here, so with two bad folders it reports either; this order
    // is fixed.
    for (name, dir) in [
        ("sourceDir", &s.source_dir),
        ("adrDir", &s.adr_dir),
        ("syncFile", &s.sync_file),
    ] {
        if !dir.is_empty() && !gofmt::is_local(dir) {
            return Err(Error::DirOutsideWorkspace {
                name,
                value: dir.clone(),
            });
        }
    }
    if !matches!(s.overview.as_str(), "auto" | "manual") {
        return Err(Error::InvalidOverview {
            value: s.overview.clone(),
        });
    }
    Ok(())
}

/// Go's `VCSOf`: the last layer's `vcs` that isn't `null`, defaulting to `git`.
pub fn vcs_of(layers: &[&Map<String, Value>]) -> Result<String, Error> {
    let mut vcs = "git".to_string();
    for layer in layers {
        match layer.get("vcs") {
            None | Some(Value::Null) => {}
            Some(Value::String(v)) => vcs = v.clone(),
            Some(other) => {
                return Err(Error::InvalidVcs {
                    shown: gofmt::value_v(other),
                });
            }
        }
    }
    if !matches!(vcs.as_str(), "git" | "none") {
        return Err(Error::InvalidVcs {
            shown: gofmt::quote(&vcs),
        });
    }
    Ok(vcs)
}

/// Go's `Load`: the global and project layers for the workspace at `root`, merged.
pub fn load(root: &Path, dirs: &Dirs) -> Result<Loaded, Error> {
    let global = read_if_exists(&global_path(dirs))?;
    let project = read_if_exists(&project_path(root))?;
    let effective = merge(&[&global, &project])?;
    let vcs = vcs_of(&[&global, &project])?;
    Ok(Loaded {
        global,
        project,
        effective,
        vcs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn map(json: &str) -> Map<String, Value> {
        match serde_json::from_str(json).unwrap() {
            Value::Object(m) => m,
            _ => panic!("not an object"),
        }
    }

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Temp {
            use std::sync::atomic::{AtomicU32, Ordering};
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "daiku-settings-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            Temp(p)
        }
        fn write(&self, rel: &str, text: &str) {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn xdg(t: &Temp) -> Dirs {
        Dirs {
            home: None,
            xdg_config_home: Some(t.0.clone().into_os_string()),
        }
    }

    // core/settings_test.go, TestLoadLayersDefaultsGlobalProject.
    #[test]
    fn layers_defaults_then_global_then_project() {
        let (config, root) = (Temp::new(), Temp::new());
        config.write(
            "daiku/settings.json",
            r#"{"gates":"prompt","baseSource":"global"}"#,
        );
        root.write(".daiku/settings.json", r#"{"baseSource":"project"}"#);
        let loaded = load(&root.0, &xdg(&config)).unwrap();
        assert_eq!(loaded.effective.gates, "prompt");
        assert_eq!(loaded.effective.base_source, "project");
        assert_eq!(loaded.global["baseSource"], "global");
        assert_eq!(loaded.project["baseSource"], "project");
        assert_eq!(loaded.vcs, "git");
    }

    // TestLoadDefaultsWhenNoFiles, TestDefaultsApplyWhenNoFiles.
    #[test]
    fn defaults_when_no_files() {
        let (config, root) = (Temp::new(), Temp::new());
        let loaded = load(&root.0, &xdg(&config)).unwrap();
        assert_eq!(loaded.effective, Settings::default());
        assert!(loaded.global.is_empty() && loaded.project.is_empty());
    }

    // TestMergeEmptyOverridesAndNullIgnored.
    #[test]
    fn empty_overrides_and_null_and_non_strings_are_ignored() {
        let got = merge(&[
            &map(r#"{"baseSource":"x"}"#),
            &map(r#"{"baseSource":"","gates":null,"overview":3}"#),
        ])
        .unwrap();
        assert_eq!(got.base_source, "");
        assert_eq!(got.gates, "block");
        assert_eq!(got.overview, "auto");
    }

    // TestMergeValidationFails, TestInvalidGatesThrows, TestOverviewSetting.
    #[test]
    fn validation_messages_match_go() {
        let err = |json: &str| merge(&[&map(json)]).unwrap_err().to_string();
        assert_eq!(
            err(r#"{"gates":"nope"}"#),
            r#"daiku: invalid gates "nope" (expected block|prompt|auto)"#
        );
        for v in ["sometimes", "", "Auto"] {
            assert_eq!(
                err(&format!(r#"{{"overview":"{v}"}}"#)),
                format!(r#"daiku: invalid overview "{v}" (expected auto|manual)"#)
            );
        }
        for v in ["auto", "manual"] {
            assert_eq!(
                merge(&[&map(&format!(r#"{{"overview":"{v}"}}"#))])
                    .unwrap()
                    .overview,
                v
            );
        }
        assert_eq!(
            err(r#"{"adrDir":"../escape"}"#),
            r#"daiku: adrDir "../escape" must be a path inside the workspace"#
        );
        assert_eq!(
            err(r#"{"sourceDir":"/abs","syncFile":"../x"}"#),
            r#"daiku: sourceDir "/abs" must be a path inside the workspace"#
        );
    }

    // TestVCSValidationFails, TestVCSOfIgnoresNull, TestVCSProjectSupersedesGlobal.
    #[test]
    fn vcs_matches_go() {
        let err = |json: &str| merge(&[&map(json)]).unwrap_err().to_string();
        assert_eq!(
            err(r#"{"vcs":"svn"}"#),
            r#"daiku: invalid vcs "svn" (expected git|none)"#
        );
        assert_eq!(
            err(r#"{"vcs":true}"#),
            "daiku: invalid vcs true (expected git|none)"
        );
        assert_eq!(
            vcs_of(&[&map(r#"{"vcs":"none"}"#), &map(r#"{"vcs":null}"#)]).unwrap(),
            "none"
        );
        for (g, p, want) in [
            ("none", r#"{"vcs":"git"}"#, "git"),
            ("git", r#"{"vcs":"none"}"#, "none"),
            ("none", "{}", "none"),
        ] {
            let global = map(&format!(r#"{{"vcs":"{g}"}}"#));
            assert_eq!(vcs_of(&[&global, &map(p)]).unwrap(), want);
        }
    }

    // TestReadIfExistsErrors, TestUnreadableJSONThrows.
    #[test]
    fn unreadable_files_report_go_messages() {
        let t = Temp::new();
        t.write("bad.json", "{ not json\n");
        t.write("list.json", "[]");
        t.write("null.json", "null");
        let p = |n: &str| t.0.join(n).to_string_lossy().into_owned();
        assert_eq!(
            read_if_exists(&p("bad.json")).unwrap_err().to_string(),
            format!(
                "daiku: cannot parse settings {}: invalid character 'n' looking for beginning of object key string",
                p("bad.json")
            )
        );
        for n in ["list.json", "null.json"] {
            assert_eq!(
                read_if_exists(&p(n)).unwrap_err().to_string(),
                format!("daiku: settings must be a JSON object: {}", p(n))
            );
        }
        assert!(read_if_exists(&p("missing.json")).unwrap().is_empty());
    }

    // AC6: the global layer comes from XDG_CONFIG_HOME, or .config under home.
    #[test]
    fn global_path_from_dirs() {
        let home = |h: &str, x: Option<&str>| Dirs {
            home: Some(h.into()),
            xdg_config_home: x.map(Into::into),
        };
        assert_eq!(
            global_path(&home("/h", None)),
            "/h/.config/daiku/settings.json"
        );
        assert_eq!(
            global_path(&home("/h", Some(""))),
            "/h/.config/daiku/settings.json"
        );
        assert_eq!(
            global_path(&home("/h", Some("/xdg"))),
            "/xdg/daiku/settings.json"
        );
        assert_eq!(global_path(&Dirs::default()), "daiku/settings.json");
        // Cleaned as Go's filepath.Join cleans it (code review finding 2).
        assert_eq!(
            global_path(&home("/h", Some("/s/r/../c2"))),
            "/s/c2/daiku/settings.json"
        );
        assert_eq!(
            global_path(&home("/h", Some("/a//b/./c/"))),
            "/a/b/c/daiku/settings.json"
        );
        assert_eq!(
            global_path(&home("/h", Some("rel/../x"))),
            "x/daiku/settings.json"
        );
        assert_eq!(
            global_path(&home("/h/", None)),
            "/h/.config/daiku/settings.json"
        );
        assert_eq!(
            global_path(&home("/", None)),
            "/.config/daiku/settings.json"
        );
        assert_eq!(global_path(&home("/h", Some("/"))), "/daiku/settings.json");
        assert_eq!(project_path(Path::new(".")), ".daiku/settings.json");
    }

    /// Windows spellings: backslashes become slashes, a drive is kept, and a UNC
    /// prefix the user wrote is kept (the CI failure at ship: a home of `/` must not
    /// turn into `//`).
    #[cfg(windows)]
    #[test]
    fn global_path_on_windows() {
        let dirs = |h: &str, x: Option<&str>| Dirs {
            home: Some(h.into()),
            xdg_config_home: x.map(Into::into),
        };
        assert_eq!(
            global_path(&dirs(r"C:\Users\me", None)),
            "C:/Users/me/.config/daiku/settings.json"
        );
        assert_eq!(
            global_path(&dirs(r"C:\h", Some(r"\\server\share\cfg"))),
            "//server/share/cfg/daiku/settings.json"
        );
        assert_eq!(
            global_path(&dirs(r"C:\h", Some(r"C:\a\..\b\"))),
            "C:/b/daiku/settings.json"
        );
    }

    #[test]
    fn global_layer_is_read_from_home_when_xdg_is_unset() {
        let (home, root) = (Temp::new(), Temp::new());
        home.write(".config/daiku/settings.json", r#"{"gates":"auto"}"#);
        let dirs = Dirs {
            home: Some(home.0.clone()),
            xdg_config_home: None,
        };
        assert_eq!(load(&root.0, &dirs).unwrap().effective.gates, "auto");
    }
}
