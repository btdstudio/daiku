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

    /// A read outside the workspace (`read_text`).
    #[error("daiku: won't read {path}: it's outside the workspace")]
    ReadOutside { path: String },

    /// A read whose path passes through a symlink.
    #[error("daiku: won't read {rel}: {link} is a symlink, which could lead outside the workspace")]
    ReadThroughSymlink { rel: String, link: String },

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

    /// Go's `os.Getwd` failing, for a client that gave core no current folder.
    #[error("getwd: the current folder is unknown")]
    NoCwd,

    /// A repo name that doesn't match `NAME_RE`. `path` is the file it was read
    /// from, `None` for a name given to `repo add`.
    #[error("daiku: {}invalid repo name {} (use lowercase letters, digits, '.', '_' and '-')", file_prefix(.path), gofmt::quote(.name))]
    InvalidRepoName { name: String, path: Option<String> },

    #[error("daiku: repo {} is already registered", gofmt::quote(.name))]
    RepoExists { name: String },

    #[error("daiku: repo {} is already archived", gofmt::quote(.name))]
    RepoAlreadyArchived { name: String },

    #[error("daiku: repo {} isn't archived", gofmt::quote(.name))]
    RepoNotArchived { name: String },

    #[error("daiku: repo {} is archived — run `daiku repo unarchive {name}` to work on it", gofmt::quote(.name))]
    RepoIsArchived { name: String },

    #[error("daiku: a remote is required")]
    RemoteRequired,

    /// `remote` holds the remote with any userinfo masked.
    #[error("daiku: invalid remote {}", gofmt::quote(.remote))]
    InvalidRemote { remote: String },

    /// `remote` holds the remote with any userinfo masked.
    #[error("daiku: remote helpers aren't allowed: {}", gofmt::quote(.remote))]
    RemoteHelper { remote: String },

    #[error("daiku: remote has credentials in it, and REPOS.json is committed — remove them")]
    RemoteCredentials,

    #[error("daiku: a path is required")]
    PathRequired,

    #[error("daiku: {path} already exists")]
    CloneExists { path: String },

    #[error("daiku: {} isn't a ticket id", gofmt::quote(.arg))]
    NotATicketId { arg: String },

    #[error("daiku: {id} is already a change — run `daiku switch {id}`")]
    BuildExists { id: String },

    #[error("daiku: no ticket {id} in plan/specs/*/tickets/ or build/tickets/")]
    NoBuildTicket { id: String },

    #[error("daiku: ticket {id} is in more than one place: {places}")]
    TicketInManyPlaces { id: String, places: String },

    #[error(
        "daiku: {id} is a planned ticket in the inbox — planned tickets live under plan/specs/"
    )]
    PlannedInInbox { id: String },

    #[error("daiku: {rel}: {cause}")]
    BuildTicketParse { rel: String, cause: String },

    #[error("daiku: {rel} holds ticket {}, not {id}", gofmt::quote(.found))]
    TicketIdMismatch {
        rel: String,
        found: String,
        id: String,
    },

    #[error("daiku: invalid ticket: {problem}")]
    InvalidBuildTicket { problem: String },

    #[error("daiku: commit and pr need an active build — run `daiku build new <ticket-id>`")]
    NoActiveBuild,

    #[error("daiku: {repo} has no usable local path ({dir}) — run `daiku repo path {repo} <dir>`")]
    NoUsablePath { repo: String, dir: String },

    #[error(
        "daiku: {repo}'s local path {dir} isn't a git checkout — run `daiku repo path {repo} <dir>`"
    )]
    NotACheckout { repo: String, dir: String },

    #[error(
        "daiku commit: nothing staged — stage one logical change with `git add`, then commit it"
    )]
    NothingStaged,

    /// A program run with the terminal attached failed, worded as Go's
    /// `exec.Cmd.Run` error (`exit status 1`).
    #[error("{text}")]
    ShipRun { text: String },

    #[error("daiku: {id} is {status}; only a ready or published planned ticket can be built")]
    NotBuildable { id: String, status: String },

    #[error("daiku: {id} has no repo — a build needs one")]
    NoBuildRepo { id: String },

    #[error("daiku: {id}'s repo {} isn't registered — run `daiku repo add {repo} <remote>`", gofmt::quote(.repo))]
    BuildRepoNotRegistered { id: String, repo: String },

    #[error("daiku: cloning {name}: {cause}: {output}")]
    CloneFailed {
        name: String,
        cause: String,
        output: String,
    },

    #[error("daiku: repo {} is missing on disk: {path} — run `daiku repo doctor`", gofmt::quote(.name))]
    RepoMissingOnDisk { name: String, path: String },

    #[error(
        "daiku: {repo} is locked by {holder} ({phase} {status}) — finish, close or reopen it to tasks"
    )]
    RepoLocked {
        repo: String,
        holder: String,
        phase: String,
        status: String,
    },

    // Ideas (Go's `internal/plan/idea` and `ideaPromote`).
    #[error("daiku: an idea needs a title")]
    IdeaTitleRequired,
    #[error("daiku: a reason is required")]
    ReasonRequired,
    #[error("daiku: idea id out of range: the highest idea id leaves no next id")]
    IdeaIdOutOfRange,
    /// `name` is `ideas/<n>-<slug>`.
    #[error("daiku: {name} already exists")]
    IdeaExists { name: String },
    #[error("daiku: idea {id} not found")]
    IdeaNotFound { id: i64 },
    #[error("daiku: idea {id} is ambiguous: more than one folder in ideas/ starts with {id}-")]
    IdeaAmbiguous { id: i64 },
    #[error("daiku: cannot read {name}/summary.md: {cause}")]
    IdeaUnreadable { name: String, cause: String },
    /// Reading: the summary parsed to no frontmatter.
    #[error("daiku: {name}/summary.md has no frontmatter")]
    IdeaNoFrontmatter { name: String },
    /// Writing: the summary has no frontmatter block to edit.
    #[error("daiku: {name}/summary.md: it has no frontmatter")]
    IdeaFrontmatter { name: String },
    #[error("daiku: idea {id} has an unknown status {}", gofmt::quote(.status))]
    IdeaUnknownStatus { id: i64, status: String },
    /// A status change the idea's status doesn't allow; `rule` says what does.
    #[error("daiku: idea {id} is {status}: {rule}")]
    IdeaWrongStatus {
        id: i64,
        status: String,
        rule: String,
    },
    #[error("daiku: idea {id} is now {status}, but its log couldn't be updated: {cause}")]
    IdeaLogFailed {
        id: i64,
        status: String,
        cause: String,
    },
    #[error("daiku: a change name is required")]
    IdeaChangeNameRequired,
    /// `idea promote` created the change, then couldn't record it on the idea.
    #[error(
        "daiku: created change {change}, but couldn't update idea {id}: {cause}; add the change to its changes line by hand"
    )]
    IdeaNotMarked {
        change: String,
        id: i64,
        cause: String,
    },

    // Overview (Go's `buildOverview` and `requirePlan`).
    #[error("daiku: no active change — name one, or run `daiku switch <change>` first")]
    NoActiveOverview,
    #[error("daiku: {name} is a build change — {verb} works on plan changes")]
    BuildChange { name: String, verb: &'static str },
    #[error("daiku: no change named {}", gofmt::quote(.name))]
    NoChangeNamed { name: String },

    // Sources.
    #[error("daiku: source {id} not found at {path}")]
    SourceNotFound { id: i64, path: String },

    #[error("daiku: source {id} already exists at {path}; not overwriting")]
    SourceExists { id: i64, path: String },

    #[error("daiku: intake id out of range: the highest intake id ({highest}) leaves no next id")]
    IntakeIdExhausted { highest: i64 },

    // ADRs.
    #[error("daiku: adr {name} already exists; not overwriting")]
    AdrExists { name: String },

    #[error("daiku: adr {number:04} not found")]
    AdrNotFound { number: i64 },

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

    /// A ticket model refusal, worded as Go's `ticket` package (`ticket S-0001: …`).
    #[error("{message}")]
    Ticket { message: String },

    /// A ticket model refusal that Go's handler wraps with `daiku: %v`.
    #[error("daiku: {message}")]
    TicketRefused { message: String },

    /// A write to a ticket whose frontmatter isn't valid UTF-8, which would change
    /// it (S-0007 D2; Go writes the bytes back).
    #[error("daiku: ticket {id}: its frontmatter isn't valid UTF-8 — fix the file first")]
    TicketNotUtf8 { id: String },

    #[error("daiku: no ticket {id}")]
    TicketNotFound { id: String },

    #[error("daiku: more than one ticket has id {id}")]
    TicketAmbiguous { id: String },

    #[error("daiku: a {typ} needs --repo <name>")]
    TicketNeedsRepo { typ: String },

    #[error("daiku: repo {} isn't registered — run `daiku repo add`", gofmt::quote(.repo))]
    TicketRepoNotRegistered { repo: String },

    #[error("daiku tickets new: {}", .problems.join("; "))]
    UnknownRequirements { problems: Vec<String> },

    #[error("daiku: can't allocate an id while a ticket is unreadable: {problem}")]
    IdUnallocatable { problem: String },

    #[error("daiku: unreadable ticket: {problem}")]
    UnreadableTicket { problem: String },

    #[error("daiku: {message}")]
    TicketOrder { message: String },

    #[error(
        "daiku: an inbox ticket has no --parent, --depends or --requirements; those belong to a plan change"
    )]
    InboxPlanFields,

    #[error("daiku: an inbox ticket is a task, not a {typ}")]
    InboxNotTask { typ: String },

    #[error("daiku: an inbox ticket needs a repo (--repo <name>)")]
    InboxNeedsRepo,

    #[error("daiku: repo {} isn't registered — run `daiku repo add {repo} <remote>`", gofmt::quote(.repo))]
    InboxRepoNotRegistered { repo: String },

    #[error("daiku: cannot read {file}: {cause}")]
    PullUnreadable { file: String, cause: String },

    #[error("daiku: draft: {cause}")]
    PullDraft { cause: String },

    #[error("daiku: a pulled ticket needs exactly one external id, the draft has {n}")]
    PullExternalCount { n: usize },

    #[error("daiku: {id}: {cause}")]
    PullParse { id: String, cause: String },

    #[error("daiku: invalid ticket: {problem}")]
    PullInvalid { problem: String },

    /// A file operation failed, worded as Go's `*os.PathError` (`open x: no such
    /// file or directory`).
    #[error("{op} {path}: {}", gofmt::io_error_text(.source))]
    Io {
        op: &'static str,
        path: String,
        source: std::io::Error,
    },
}

/// `"<path>: "` for an error read from a file, or nothing.
fn file_prefix(path: &Option<String>) -> String {
    path.as_ref().map(|p| format!("{p}: ")).unwrap_or_default()
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
