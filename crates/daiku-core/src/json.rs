//! JSON helpers and the versioned `--json` contract (ADR-0004).
//!
//! File writing (ADR-0006): two-space indentation, struct field order, sorted map
//! keys (`BTreeMap`, or serde_json's sorted `Map`), and a trailing newline, with
//! serde_json's normal escaping.
//!
//! Read-command envelope (spec R5, ADR-0004): `{"schema":1,"data":<value>}` on
//! success, or `{"schema":1,"error":{"kind":"<Variant>","message":"<text>"}}`
//! with exit 1 on failure. `message` is the error's `Display` (Go-matching text)
//! and `kind` is the stable `Error` variant name.

use schemars::JsonSchema;
use serde::Serialize;

use crate::Error;

/// The envelope schema version.
pub const SCHEMA: u32 = 1;

/// `value` as pretty JSON plus a newline.
pub fn to_pretty<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut out = serde_json::to_vec_pretty(value)?;
    out.push(b'\n');
    Ok(out)
}

/// A successful read: `data` is the serialised core value the command reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Success<T> {
    pub schema: u32,
    pub data: T,
}

impl<T> Success<T> {
    pub fn new(data: T) -> Self {
        Self {
            schema: SCHEMA,
            data,
        }
    }
}

/// A failed read command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Failure {
    pub schema: u32,
    pub error: ErrorBody,
}

/// The typed error carried in a [`Failure`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ErrorBody {
    /// The stable `Error` variant name.
    pub kind: String,
    /// The error's display text (what the CLI would print without `--json`).
    pub message: String,
}

/// The envelope for an error.
pub fn failure(err: &Error) -> Failure {
    Failure {
        schema: SCHEMA,
        error: error_body(err),
    }
}

/// Map a core error to its wire form: variant name and display text.
pub fn error_body(err: &Error) -> ErrorBody {
    ErrorBody {
        kind: error_kind(err).to_string(),
        message: err.to_string(),
    }
}

/// The stable variant name for a core error.
pub fn error_kind(err: &Error) -> &'static str {
    match err {
        Error::WriteOutside { .. } => "WriteOutside",
        Error::WriteThroughSymlink { .. } => "WriteThroughSymlink",
        Error::SettingsParse { .. } => "SettingsParse",
        Error::SettingsNotObject { .. } => "SettingsNotObject",
        Error::InvalidGates { .. } => "InvalidGates",
        Error::InvalidOverview { .. } => "InvalidOverview",
        Error::InvalidVcs { .. } => "InvalidVcs",
        Error::DirOutsideWorkspace { .. } => "DirOutsideWorkspace",
        Error::Frontmatter { .. } => "Frontmatter",
        Error::NotAWorkspace { .. } => "NotAWorkspace",
        Error::Manifest { .. } => "Manifest",
        Error::InitSettingsParse { .. } => "InitSettingsParse",
        Error::InitSettingsNotObject { .. } => "InitSettingsNotObject",
        Error::RoomSymlink { .. } => "RoomSymlink",
        Error::UnknownRoom { .. } => "UnknownRoom",
        Error::LegacyWorkspace { .. } => "LegacyWorkspace",
        Error::Git { .. } => "Git",
        Error::OutOfOrder { .. } => "OutOfOrder",
        Error::ChangeClosed { .. } => "ChangeClosed",
        Error::UnknownPhase { .. } => "UnknownPhase",
        Error::CantSkip { .. } => "CantSkip",
        Error::UnknownStatus { .. } => "UnknownStatus",
        Error::SkipNeedsReason { .. } => "SkipNeedsReason",
        Error::ReopenNeedsReason { .. } => "ReopenNeedsReason",
        Error::NotStarted { .. } => "NotStarted",
        Error::AlreadyClosed { .. } => "AlreadyClosed",
        Error::CloseNeedsReason { .. } => "CloseNeedsReason",
        Error::InvalidChangeName { .. } => "InvalidChangeName",
        Error::BuildNoRepo { .. } => "BuildNoRepo",
        Error::BuildInvalidRepo { .. } => "BuildInvalidRepo",
        Error::ChangeFolderOutside { .. } => "ChangeFolderOutside",
        Error::UnknownLoopFor { .. } => "UnknownLoopFor",
        Error::CannotRead { .. } => "CannotRead",
        Error::StateParse { .. } => "StateParse",
        Error::StateSchema { .. } => "StateSchema",
        Error::StateNoLoop { .. } => "StateNoLoop",
        Error::StateUnknownLoop { .. } => "StateUnknownLoop",
        Error::UnknownChange { .. } => "UnknownChange",
        Error::ChangeExists { .. } => "ChangeExists",
        Error::WontCreate { .. } => "WontCreate",
        Error::NoActiveChange => "NoActiveChange",
        Error::ActiveInvalid { .. } => "ActiveInvalid",
        Error::ActiveNoFolder { .. } => "ActiveNoFolder",
        Error::ActiveNoFolderAt { .. } => "ActiveNoFolderAt",
        Error::NotCommitted { .. } => "NotCommitted",
        Error::UnknownRepo { .. } => "UnknownRepo",
        Error::RepoNoPath { .. } => "RepoNoPath",
        Error::NoHome => "NoHome",
        Error::NoCwd => "NoCwd",
        Error::InvalidRepoName { .. } => "InvalidRepoName",
        Error::RepoExists { .. } => "RepoExists",
        Error::RemoteRequired => "RemoteRequired",
        Error::InvalidRemote { .. } => "InvalidRemote",
        Error::RemoteHelper { .. } => "RemoteHelper",
        Error::RemoteCredentials => "RemoteCredentials",
        Error::PathRequired => "PathRequired",
        Error::CloneExists { .. } => "CloneExists",
        Error::NotATicketId { .. } => "NotATicketId",
        Error::BuildExists { .. } => "BuildExists",
        Error::NoBuildTicket { .. } => "NoBuildTicket",
        Error::TicketInManyPlaces { .. } => "TicketInManyPlaces",
        Error::PlannedInInbox { .. } => "PlannedInInbox",
        Error::BuildTicketParse { .. } => "BuildTicketParse",
        Error::TicketIdMismatch { .. } => "TicketIdMismatch",
        Error::InvalidBuildTicket { .. } => "InvalidBuildTicket",
        Error::NoActiveBuild => "NoActiveBuild",
        Error::NoUsablePath { .. } => "NoUsablePath",
        Error::NotACheckout { .. } => "NotACheckout",
        Error::NothingStaged => "NothingStaged",
        Error::ShipRun { .. } => "ShipRun",
        Error::NotBuildable { .. } => "NotBuildable",
        Error::NoBuildRepo { .. } => "NoBuildRepo",
        Error::BuildRepoNotRegistered { .. } => "BuildRepoNotRegistered",
        Error::CloneFailed { .. } => "CloneFailed",
        Error::RepoMissingOnDisk { .. } => "RepoMissingOnDisk",
        Error::RepoLocked { .. } => "RepoLocked",
        Error::IdeaTitleRequired => "IdeaTitleRequired",
        Error::ReasonRequired => "ReasonRequired",
        Error::IdeaIdOutOfRange => "IdeaIdOutOfRange",
        Error::IdeaExists { .. } => "IdeaExists",
        Error::IdeaNotFound { .. } => "IdeaNotFound",
        Error::IdeaAmbiguous { .. } => "IdeaAmbiguous",
        Error::IdeaUnreadable { .. } => "IdeaUnreadable",
        Error::IdeaNoFrontmatter { .. } => "IdeaNoFrontmatter",
        Error::IdeaFrontmatter { .. } => "IdeaFrontmatter",
        Error::IdeaUnknownStatus { .. } => "IdeaUnknownStatus",
        Error::IdeaWrongStatus { .. } => "IdeaWrongStatus",
        Error::IdeaLogFailed { .. } => "IdeaLogFailed",
        Error::IdeaChangeNameRequired => "IdeaChangeNameRequired",
        Error::IdeaNotMarked { .. } => "IdeaNotMarked",
        Error::NoActiveOverview => "NoActiveOverview",
        Error::BuildChange { .. } => "BuildChange",
        Error::NoChangeNamed { .. } => "NoChangeNamed",
        Error::SourceNotFound { .. } => "SourceNotFound",
        Error::SourceExists { .. } => "SourceExists",
        Error::IntakeIdExhausted { .. } => "IntakeIdExhausted",
        Error::AdrExists { .. } => "AdrExists",
        Error::AdrNotFound { .. } => "AdrNotFound",
        Error::PathError { .. } => "PathError",
        Error::Overview { .. } => "Overview",
        Error::GateRefused { .. } => "GateRefused",
        Error::RepoNotRegistered { .. } => "RepoNotRegistered",
        Error::NotAPlan { .. } => "NotAPlan",
        Error::Ticket { .. } => "Ticket",
        Error::TicketRefused { .. } => "TicketRefused",
        Error::TicketNotUtf8 { .. } => "TicketNotUtf8",
        Error::TicketNotFound { .. } => "TicketNotFound",
        Error::TicketAmbiguous { .. } => "TicketAmbiguous",
        Error::TicketNeedsRepo { .. } => "TicketNeedsRepo",
        Error::TicketRepoNotRegistered { .. } => "TicketRepoNotRegistered",
        Error::UnknownRequirements { .. } => "UnknownRequirements",
        Error::IdUnallocatable { .. } => "IdUnallocatable",
        Error::UnreadableTicket { .. } => "UnreadableTicket",
        Error::TicketOrder { .. } => "TicketOrder",
        Error::InboxPlanFields => "InboxPlanFields",
        Error::InboxNotTask { .. } => "InboxNotTask",
        Error::InboxNeedsRepo => "InboxNeedsRepo",
        Error::InboxRepoNotRegistered { .. } => "InboxRepoNotRegistered",
        Error::PullUnreadable { .. } => "PullUnreadable",
        Error::PullDraft { .. } => "PullDraft",
        Error::PullExternalCount { .. } => "PullExternalCount",
        Error::PullParse { .. } => "PullParse",
        Error::PullInvalid { .. } => "PullInvalid",
        Error::Io { .. } => "Io",
    }
}

/// A read command's version string (`version --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Version {
    pub version: String,
}

/// A read command's environment flags (`env --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct EnvFlags {
    pub flags: String,
}

/// An idea's summary (`idea show --json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct IdeaSummary {
    pub summary: crate::text::Text,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Serialize)]
    struct State<'a> {
        schema: u32,
        changes: BTreeMap<&'a str, Vec<&'a str>>,
        note: &'a str,
    }

    #[test]
    fn two_spaces_field_order_sorted_keys_newline() {
        let mut changes = BTreeMap::new();
        changes.insert("b", vec!["x"]);
        changes.insert("a", vec![]);
        let s = State {
            schema: 1,
            changes,
            note: "<a> & 'b'",
        };
        let text = String::from_utf8(to_pretty(&s).unwrap()).unwrap();
        assert_eq!(
            text,
            "{\n  \"schema\": 1,\n  \"changes\": {\n    \"a\": [],\n    \"b\": [\n      \"x\"\n    ]\n  },\n  \"note\": \"<a> & 'b'\"\n}\n"
        );
    }

    #[test]
    fn serde_json_maps_are_sorted() {
        let v: serde_json::Value = serde_json::from_str(r#"{"z":1,"a":2}"#).unwrap();
        assert_eq!(to_pretty(&v).unwrap(), b"{\n  \"a\": 2,\n  \"z\": 1\n}\n");
    }
}
