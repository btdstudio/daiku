//! AC2: the built binary alone initialises an empty folder outside the source tree.

use std::process::Command;

#[test]
fn init_from_the_binary_alone() {
    let dir = std::env::temp_dir().join(format!("daiku-init-ac2-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config");
    let out = Command::new(env!("CARGO_BIN_EXE_daiku"))
        .args(["init", "--no-git"])
        .current_dir(&dir)
        .env("XDG_CONFIG_HOME", &config)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    for f in [
        "AGENTS.md",
        "CLAUDE.md",
        "STATE.json",
        "REPOS.json",
        "sync.md",
        ".daiku/settings.json",
        ".daiku/manifest.json",
    ] {
        assert!(dir.join(f).is_file(), "{f}");
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.ends_with("scaffolded daiku workspace in .\n"),
        "{stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
