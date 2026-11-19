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

    /// `update` (prefix `daiku update`) or room creation (prefix `daiku`) outside a
    /// workspace.
    #[error("{prefix}: {dir} is not a daiku workspace (run `daiku init`)")]
    NotAWorkspace { prefix: &'static str, dir: String },

    /// The manifest couldn't be read; `detail` is Go's error text.
    #[error("{prefix}: {detail}")]
    Manifest {
        prefix: &'static str,
        detail: String,
    },

    /// `init` couldn't read existing settings to record `vcs: none`.
    #[error("daiku init: cannot parse settings {file}: {detail}")]
    InitSettingsParse { file: String, detail: String },

    #[error("daiku init: settings must be a JSON object: {file}")]
    InitSettingsNotObject { file: String },

    #[error(
        "daiku: {room}/ is a symlink, which could lead outside the workspace — replace it with a folder"
    )]
    RoomSymlink { room: String },

    #[error("daiku: unknown room {}", gofmt::quote(.room))]
    UnknownRoom { room: String },

    /// A zumen or tomo workspace.
    #[error(
        "daiku: this is a {tool} workspace (found .{tool}/) — use {tool}, or start a new daiku workspace"
    )]
    LegacyWorkspace { tool: &'static str },

    /// A git command failed: Go's `git <args>: <err>: <combined output>`.
    #[error("git {args}: {err}: {output}")]
    Git {
        args: String,
        err: String,
        output: String,
    },

    // Changes and gates (state).
    /// Go's `CheckGate` violation.
    #[error("daiku: {phase} requires {earlier} to be {need} (currently {})", gofmt::quote(.got))]
    OutOfOrder {
        phase: String,
        earlier: &'static str,
        need: &'static str,
        got: String,
    },

    #[error("daiku: {name} is closed")]
    ChangeClosed { name: String },

    #[error("unknown phase: {phase}")]
    UnknownPhase { phase: String },

    #[error("daiku: {phase} can't be skipped")]
    CantSkip { phase: String },

    #[error("unknown gate status: {status}")]
    UnknownStatus { status: String },

    #[error("daiku: skipping {phase} requires a reason")]
    SkipNeedsReason { phase: String },

    #[error("daiku: reopening {phase} requires a reason")]
    ReopenNeedsReason { phase: String },

    #[error("daiku: {phase} hasn't started")]
    NotStarted { phase: String },

    #[error("daiku: {name} is already closed")]
    AlreadyClosed { name: String },

    #[error("daiku: closing {name} requires a reason")]
    CloseNeedsReason { name: String },

    #[error("daiku: invalid change name {}", gofmt::quote(.name))]
    InvalidChangeName { name: String },

    #[error("build change {} has no repo", gofmt::quote(.name))]
    BuildNoRepo { name: String },

    #[error("build change {} has an invalid repo name {}", gofmt::quote(.name), gofmt::quote(.repo))]
    BuildInvalidRepo { name: String, repo: String },

    #[error("daiku: change {} has a folder outside the workspace: {rel}", gofmt::quote(.name))]
    ChangeFolderOutside { name: String, rel: String },

    #[error("daiku: unknown loop {} for change {}", gofmt::quote(.loop_), gofmt::quote(.name))]
    UnknownLoopFor { loop_: String, name: String },

    /// A file or folder that exists but can't be read; `detail` is Go's error.
    #[error("daiku: cannot read {path}: {detail}")]
    CannotRead { path: String, detail: String },

    /// `STATE.json` that isn't valid or doesn't decode; `detail` is Go's message.
    #[error("daiku: cannot parse {path}: {detail}")]
    StateParse { path: String, detail: String },

    #[error(
        "daiku: {path} has schema {schema}, newer than this version supports (1) — upgrade daiku"
    )]
    StateSchema { path: String, schema: i64 },

    #[error("daiku: {path}: change {} records no loop", gofmt::quote(.name))]
    StateNoLoop { path: String, name: String },

    #[error("daiku: {path}: change {} records unknown loop {}", gofmt::quote(.name), gofmt::quote(.loop_))]
    StateUnknownLoop {
        path: String,
        name: String,
        loop_: String,
    },

    /// Go writes this one with literal quotes, not `%q`.
    #[error("daiku: unknown change \"{name}\"")]
    UnknownChange { name: String },

    #[error("daiku: change {} already exists in {path}", gofmt::quote(.name))]
    ChangeExists { name: String, path: String },

    #[error(
        "daiku: won't create {rel}: {link} is a symlink, which could lead outside the workspace"
    )]
    WontCreate { rel: String, link: String },

    #[error("daiku: no active change — run `daiku switch <change>` first")]
    NoActiveChange,

    #[error("daiku: active change {} isn't a valid change folder — run `daiku switch <change>`", gofmt::quote(.name))]
    ActiveInvalid { name: String },

    #[error("daiku: active change {} has no folder — run `daiku switch <change>`", gofmt::quote(.name))]
    ActiveNoFolder { name: String },

    #[error("daiku: active change {} has no folder {rel}/ — run `daiku switch <change>`", gofmt::quote(.name))]
    ActiveNoFolderAt { name: String, rel: String },

    /// The state was saved but a git step failed; `detail` is `git <cmd>: …`.
    #[error("daiku: state saved but not committed: {detail}")]
    NotCommitted { detail: String },

    // Repos.
    #[error("daiku: unknown repo {} — run `daiku repo add`", gofmt::quote(.name))]
    UnknownRepo { name: String },

    #[error("daiku: repo {} has no local path — run `daiku repo path {name} <path>`", gofmt::quote(.name))]
    RepoNoPath { name: String },

    /// Go's `os.UserHomeDir` error.
    #[error("$HOME is not defined")]
    NoHome,

    #[error(
        "daiku: {repo} is locked by {holder} ({phase} {status}) — finish, close or reopen it to tasks"
    )]
    RepoLocked {
        repo: String,
        holder: String,
        phase: String,
        status: String,
    },

    // Sources.
    #[error("daiku: source {id} not found at {path}")]
    SourceNotFound { id: i64, path: String },

    #[error("daiku: source {id} already exists at {path}; not overwriting")]
    SourceExists { id: i64, path: String },

    #[error("daiku: intake id out of range: the highest intake id ({highest}) leaves no next id")]
    IntakeIdExhausted { highest: i64 },

    /// A file error worded as Go's `*PathError` (`open x: permission denied`).
    #[error("{text}")]
    PathError { text: String },

    /// A plan overview that couldn't be built or written.
    #[error("daiku: {rel}: {detail}")]
    Overview { rel: String, detail: String },

    /// A gate refused under `gates: block`: every violation, then how to override.
    #[error("{}", gate_refused(.violations))]
    GateRefused { violations: Vec<String> },

    #[error("daiku: {} isn't registered — run `daiku repo add` for each repo in a multi-repo change", gofmt::quote(.name))]
    RepoNotRegistered { name: String },

    #[error("daiku: {name} isn't a plan change in this workspace{}", if *.build { " (it's a build — bare daiku status lists it)" } else { "" })]
    NotAPlan { name: String, build: bool },

    /// A file operation failed, worded as Go's `*os.PathError` (`open x: no such
    /// file or directory`).
    #[error("{op} {path}: {}", gofmt::io_error_text(.source))]
    Io {
        op: &'static str,
        path: String,
        source: std::io::Error,
    },
}

/// Go's `gatePolicyAll` refusal: the violations joined by newlines, and the
/// override hint on its own line when there's more than one line.
fn gate_refused(violations: &[String]) -> String {
    let msg = violations.join("\n");
    if msg.contains('\n') {
        format!("{msg}\n(pass --force to override)")
    } else {
        format!("{msg} (pass --force to override)")
    }
}
