//! Core's errors. Each variant's `Display` is the Go CLI's message, so a client
//! prints `{err}` and nothing else (R12, D3).

use crate::gofmt;

/// Everything core can fail with. Variants are added as operations are ported.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// A write outside the workspace (`write.go`, `CheckPath`).
    #[error("daiku: won't write {path}: it's outside the workspace")]
    WriteOutside { path: String },

    /// A write whose path passes through a symlink.
    #[error(
        "daiku: won't write {rel}: {link} is a symlink, which could lead outside the workspace"
    )]
    WriteThroughSymlink { rel: String, link: String },

    /// A settings file that isn't valid JSON; `detail` is Go's JSON message.
    #[error("daiku: cannot parse settings {path}: {detail}")]
    SettingsParse { path: String, detail: String },

    /// A settings file whose JSON isn't an object.
    #[error("daiku: settings must be a JSON object: {path}")]
    SettingsNotObject { path: String },

    #[error("daiku: invalid gates {} (expected block|prompt|auto)", gofmt::quote(.value))]
    InvalidGates { value: String },

    #[error("daiku: invalid overview {} (expected auto|manual)", gofmt::quote(.value))]
    InvalidOverview { value: String },

    /// `shown` is the value as Go prints it: quoted for a string, `%v` otherwise.
    #[error("daiku: invalid vcs {shown} (expected git|none)")]
    InvalidVcs { shown: String },

    /// `sourceDir`, `adrDir` or `syncFile` naming a path outside the workspace.
    #[error("daiku: {name} {} must be a path inside the workspace", gofmt::quote(.value))]
    DirOutsideWorkspace { name: &'static str, value: String },

    /// Frontmatter that can't be read or a value that can't be written; the message
    /// starts `frontmatter:` as Go's does.
    #[error("{message}")]
    Frontmatter { message: String },

    /// A file operation failed, worded as Go's `*os.PathError` (`open x: no such
    /// file or directory`).
    #[error("{op} {path}: {}", gofmt::io_error_text(.source))]
    Io {
        op: &'static str,
        path: String,
        source: std::io::Error,
    },
}
