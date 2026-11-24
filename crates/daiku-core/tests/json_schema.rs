//! Keeps `docs/json-schema/` in sync with the `--json` contract (ADR-0004, AC4).
//! Run with `UPDATE_SCHEMAS=1` to regenerate.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use daiku_core::build::RepoRow;
use daiku_core::changes::{ActiveStatus, PlanStatus, RepoBuilds, Status};
use daiku_core::json::{EnvFlags, ErrorBody, Failure, IdeaSummary, Success, Version};
use daiku_core::plan::adr;
use daiku_core::plan::idea;
use daiku_core::plan::overview::BuildRow;
use daiku_core::plan::reference::{Check, Entry, Finding};
use daiku_core::plan::source::Source;
use daiku_core::room::OverviewDoc;
use daiku_core::settings::{Loaded, Settings};
use daiku_core::state::Change;
use daiku_core::state::store::Known;
use daiku_core::ticket::Problem;
use daiku_core::tickets::{Bucket, CheckReport, Ordered, StatusReport};
use daiku_core::{Artifact, ArtifactList, NextItem, NextList, TicketList, TicketView};
use schemars::schema_for;

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("docs/json-schema")
}

fn pretty(value: &impl serde::Serialize) -> String {
    let mut out = serde_json::to_string_pretty(value).expect("schema serializes");
    out.push('\n');
    out
}

fn schemas() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    macro_rules! add {
        ($name:expr, $ty:ty) => {
            m.insert($name.to_string(), pretty(&schema_for!($ty)));
        };
    }
    // Error envelope and contract types.
    add!("Failure", Failure);
    add!("ErrorBody", ErrorBody);
    add!("Version", Version);
    add!("EnvFlags", EnvFlags);
    add!("IdeaSummary", IdeaSummary);

    // R5 success envelopes (full contract per read shape).
    add!("Success_Version", Success<Version>);
    add!("Success_EnvFlags", Success<EnvFlags>);
    add!("Success_Loaded", Success<Loaded>);
    add!("Success_Status", Success<Status>);
    add!("Success_PlanStatus", Success<PlanStatus>);
    add!("Success_KnownList", Success<Vec<Known>>);
    add!("Success_RepoList", Success<Vec<RepoRow>>);
    add!("Success_ReferenceCheck", Success<Check>);
    add!("Success_SourceList", Success<Vec<Source>>);
    add!("Success_Source", Success<Source>);
    add!("Success_AdrList", Success<Vec<adr::Record>>);
    add!("Success_Adr", Success<adr::Record>);
    add!("Success_ArtifactList", Success<ArtifactList>);
    add!("Success_TicketList", Success<TicketList>);
    add!("Success_NextList", Success<NextList>);
    add!("Success_IdeaList", Success<Vec<idea::Idea>>);
    add!("Success_IdeaSummary", Success<IdeaSummary>);
    add!("Success_TicketsCheck", Success<CheckReport>);
    add!("Success_TicketsStatus", Success<StatusReport>);
    add!("Success_TicketsOrder", Success<Vec<Ordered>>);

    // Component/view types.
    add!("Loaded", Loaded);
    add!("Settings", Settings);
    add!("Status", Status);
    add!("ActiveStatus", ActiveStatus);
    add!("Change", Change);
    add!("RepoBuilds", RepoBuilds);
    add!("PlanStatus", PlanStatus);
    add!("BuildRow", BuildRow);
    add!("Known", Known);
    add!("RepoRow", RepoRow);
    add!("Check", Check);
    add!("Entry", Entry);
    add!("Finding", Finding);
    add!("Source", Source);
    add!("AdrRecord", adr::Record);
    add!("Idea", idea::Idea);
    add!("CheckReport", CheckReport);
    add!("Problem", Problem);
    add!("TicketsStatusReport", StatusReport);
    add!("Bucket", Bucket);
    add!("Ordered", Ordered);
    add!("OverviewDoc", OverviewDoc);
    add!("ArtifactList", ArtifactList);
    add!("Artifact", Artifact);
    add!("TicketList", TicketList);
    add!("TicketView", TicketView);
    add!("NextList", NextList);
    add!("NextItem", NextItem);
    m
}

#[test]
fn json_schemas_match_docs() {
    let dir = dir();
    let schemas = schemas();
    if std::env::var_os("UPDATE_SCHEMAS").is_some() {
        std::fs::create_dir_all(&dir).expect("create docs/json-schema");
        // Remove stale schema files we own (only *.json in this dir).
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if !schemas.contains_key(name) {
                        let _ = std::fs::remove_file(p);
                    }
                }
            }
        }
        for (name, body) in &schemas {
            std::fs::write(dir.join(format!("{name}.json")), body).expect("write schema");
        }
        return;
    }

    let mut missing = Vec::new();
    let mut changed = Vec::new();
    for (name, body) in &schemas {
        let path = dir.join(format!("{name}.json"));
        match std::fs::read_to_string(&path) {
            Ok(existing) if existing.replace("\r\n", "\n") == *body => {}
            Ok(_) => changed.push(name.clone()),
            Err(_) => missing.push(name.clone()),
        }
    }
    // Stale committed schemas not in the expected set.
    let mut stale = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|s| s.to_str()) == Some("json") {
                let name = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                if !schemas.contains_key(&name) {
                    stale.push(name);
                }
            }
        }
    }
    assert!(
        missing.is_empty() && changed.is_empty() && stale.is_empty(),
        "docs/json-schema out of date (regenerate with UPDATE_SCHEMAS=1): missing={missing:?} changed={changed:?} stale={stale:?}"
    );
}
