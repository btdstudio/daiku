//! The write guard: every file core writes goes through here (N6, D6).
//!
//! A port of Go's `internal/core/workspace/write.go`: a path must lie inside the
//! workspace, and no component of it, the file itself included, may be a symlink,
//! so a committed link can't send a write anywhere the user can write.

use std::fs::{File, OpenOptions};
use std::path::{Component, Path};

use crate::Error;
use crate::gofmt;

/// Go's `CheckPath`: `Ok` when `path` (the full path, joined to `root`) is safe to
/// write; a path that doesn't exist yet is fine.
pub fn check_path(root: &Path, path: &Path) -> Result<(), Error> {
    let shown = gofmt::to_slash(&path.to_string_lossy());
    let rel = match relative(root, path) {
        Some(rel) if gofmt::is_local(&rel) => rel,
        _ => return Err(Error::WriteOutside { path: shown }),
    };
    if let Some(link) = through_symlink(root, &rel) {
        return Err(Error::WriteThroughSymlink { rel, link });
    }
    Ok(())
}

/// Go's `MkdirAll` under `root`, after [`check_path`].
pub fn create_dir_all(root: &Path, path: &Path) -> Result<(), Error> {
    check_path(root, path)?;
    std::fs::create_dir_all(path).map_err(|source| Error::Io {
        op: "mkdir",
        path: gofmt::to_slash(&path.to_string_lossy()),
        source,
    })
}

/// Go's `WriteFile` under `root`: checks and creates the file's folder, checks the
/// file, then writes it.
pub fn write_file(root: &Path, path: &Path, data: &[u8]) -> Result<(), Error> {
    create_dir_all(root, &parent(path))?;
    check_path(root, path)?;
    std::fs::write(path, data).map_err(|source| Error::Io {
        op: "open",
        path: gofmt::to_slash(&path.to_string_lossy()),
        source,
    })
}

/// Go's `OpenFile` under `root`, after [`check_path`]. It creates no folders.
pub fn open_file(root: &Path, path: &Path, options: &OpenOptions) -> Result<File, Error> {
    check_path(root, path)?;
    options.open(path).map_err(|source| Error::Io {
        op: "open",
        path: gofmt::to_slash(&path.to_string_lossy()),
        source,
    })
}

/// The folder holding `path`, `.` for a bare file name, as Go's `filepath.Dir`.
fn parent(path: &Path) -> std::path::PathBuf {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => ".".into(),
    }
}

/// `path` relative to `root`, slash-separated and cleaned, as `filepath.Rel` gives
/// it; `None` when one is absolute and the other isn't.
fn relative(root: &Path, path: &Path) -> Option<String> {
    let norm = |p: &Path| gofmt::clean(&gofmt::to_slash(&p.to_string_lossy()));
    let (root, path) = (norm(root), norm(path));
    if root.starts_with('/') != path.starts_with('/') || has_prefix(&root) != has_prefix(&path) {
        return None;
    }
    if root == "." {
        return Some(path);
    }
    if path == root {
        return Some(".".into());
    }
    let base = if root.ends_with('/') {
        root.clone()
    } else {
        format!("{root}/")
    };
    if let Some(rest) = path.strip_prefix(&base) {
        return Some(rest.to_string());
    }
    // Not under root: climb out, as Rel would, which is_local then refuses.
    Some(format!("../{path}"))
}

/// A Windows drive or UNC prefix (`C:`, `//server`), which `clean` doesn't model.
fn has_prefix(p: &str) -> bool {
    Path::new(p)
        .components()
        .next()
        .is_some_and(|c| matches!(c, Component::Prefix(_)))
}

/// Go's `ThroughSymlink`: the first component of `rel` under `root` that is a
/// symlink, as root joined with the parts so far (slash form), or `None`. It stops
/// at the first component that doesn't exist.
pub fn through_symlink(root: &Path, rel: &str) -> Option<String> {
    let mut cur = root.to_path_buf();
    let mut shown = gofmt::to_slash(&root.to_string_lossy());
    for part in gofmt::clean(rel).split('/') {
        // Go's filepath.Join(cur, ".") is cur itself, so for `.` the root is checked.
        if part != "." {
            cur.push(part);
            shown = gofmt::join(&[shown.as_str(), part]);
        }
        match std::fs::symlink_metadata(&cur) {
            Err(_) => return None,
            Ok(m) if m.file_type().is_symlink() => return Some(shown),
            Ok(_) => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> (tempdir::Dir, std::path::PathBuf) {
        let d = tempdir::Dir::new();
        let p = d.path.clone();
        (d, p)
    }

    /// A minimal temp folder, so core's tests need no extra crate.
    mod tempdir {
        pub struct Dir {
            pub path: std::path::PathBuf,
        }
        impl Dir {
            pub fn new() -> Dir {
                use std::sync::atomic::{AtomicU32, Ordering};
                static N: AtomicU32 = AtomicU32::new(0);
                let path = std::env::temp_dir().join(format!(
                    "daiku-core-test-{}-{}",
                    std::process::id(),
                    N.fetch_add(1, Ordering::Relaxed)
                ));
                std::fs::create_dir_all(&path).unwrap();
                Dir { path }
            }
        }
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.path);
            }
        }
    }

    // workspace_test.go, TestCheckPath.
    #[test]
    fn check_path_matches_go() {
        let (_d, root) = temp();
        std::fs::create_dir_all(root.join("a")).unwrap();
        for rel in ["a/x.md", "STATE.json", "new/deeper/x.md"] {
            check_path(&root, &root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        }
        for rel in ["../escape/x.md", "a/../../x"] {
            let err = check_path(&root, &root.join(rel)).unwrap_err().to_string();
            assert!(err.contains("outside the workspace"), "{rel}: {err}");
        }
        let (_e, elsewhere) = temp();
        assert!(check_path(&root, &elsewhere.join("x")).is_err());
    }

    #[test]
    fn messages_match_go_with_a_relative_root() {
        let err = check_path(Path::new("."), Path::new("../x.md")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "daiku: won't write ../x.md: it's outside the workspace"
        );
        let err = check_path(Path::new("."), Path::new("/etc/x")).unwrap_err();
        assert_eq!(
            err.to_string(),
            "daiku: won't write /etc/x: it's outside the workspace"
        );
        check_path(Path::new("."), Path::new("plan/x.md")).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_refused_with_go_messages() {
        use std::os::unix::fs::symlink;
        let (_d, root) = temp();
        let (_o, outside) = temp();
        std::fs::write(outside.join("victim"), "precious").unwrap();
        symlink(&outside, root.join("linkdir")).unwrap();
        symlink(outside.join("victim"), root.join("linkfile")).unwrap();
        for (rel, want) in [
            ("linkdir/x.md", "linkdir is a symlink"),
            ("linkfile", "linkfile is a symlink"),
        ] {
            let err = check_path(&root, &root.join(rel)).unwrap_err().to_string();
            assert!(err.contains(want), "{rel}: {err}");
            assert!(
                err.starts_with(&format!("daiku: won't write {rel}: ")),
                "{err}"
            );
        }
    }

    /// A root that is itself a symlink is refused for writes at the root, as Go
    /// refuses them (code review finding 4).
    #[cfg(unix)]
    #[test]
    fn a_symlinked_root_is_refused_at_the_root() {
        let (_d, real) = temp();
        let (_l, holder) = temp();
        let root = holder.join("ws");
        std::os::unix::fs::symlink(&real, &root).unwrap();
        let err = write_file(&root, &root.join("STATE.json"), b"{}")
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            format!(
                "daiku: won't write .: {} is a symlink, which could lead outside the workspace",
                root.display()
            )
        );
        assert!(!real.join("STATE.json").exists());
    }

    // workspace_test.go, TestWriteHelpers.
    #[test]
    fn write_helpers_write_inside() {
        let (_d, root) = temp();
        let p = root.join("x").join("y.md");
        write_file(&root, &p, b"hi").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"hi");
        use std::io::Write;
        let mut f = open_file(&root, &p, OpenOptions::new().append(true)).unwrap();
        f.write_all(b"!").unwrap();
        drop(f);
        assert_eq!(std::fs::read(&p).unwrap(), b"hi!");
    }

    #[cfg(unix)]
    #[test]
    fn refused_writes_leave_nothing_behind() {
        use std::os::unix::fs::symlink;
        let (_d, root) = temp();
        let (_o, outside) = temp();
        let victim = outside.join("victim");
        std::fs::write(&victim, "precious").unwrap();
        symlink(&victim, root.join("link")).unwrap();
        symlink(&outside, root.join("dir")).unwrap();
        type Attempt<'a> = (&'a str, Box<dyn Fn() -> Result<(), Error> + 'a>);
        let attempts: [Attempt; 4] = [
            (
                "file link",
                Box::new(|| write_file(&root, &root.join("link"), b"x")),
            ),
            (
                "folder link",
                Box::new(|| write_file(&root, &root.join("dir/n"), b"x")),
            ),
            (
                "mkdir through a link",
                Box::new(|| create_dir_all(&root, &root.join("dir/sub"))),
            ),
            (
                "append through a link",
                Box::new(|| {
                    open_file(&root, &root.join("link"), OpenOptions::new().append(true)).map(drop)
                }),
            ),
        ];
        for (name, run) in attempts {
            let err = run().unwrap_err().to_string();
            assert!(err.contains("is a symlink"), "{name}: {err}");
        }
        assert_eq!(std::fs::read(&victim).unwrap(), b"precious");
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
        // An outside write creates no folder either.
        let err = write_file(&root, &root.join("../escape-dir/x"), b"x").unwrap_err();
        assert!(err.to_string().contains("outside"));
        assert!(!root.join("../escape-dir").exists());
    }
}
