//! The plan overview around transitions: which plan a change refreshes, the
//! build rows, the check before a transition, and the refresh after it. A port
//! of Go's `internal/state/overview.go` and `builds.go`.

use std::collections::BTreeMap;
use std::path::Path;

use super::store::{at, read_file, shown};
use super::{BUILD_LOOP, Change, PLAN_DIR, PLAN_LOOP, Store, machine};
use crate::plan::overview::{BuildRow, Input, markers, render};
use crate::plan::{SPECS_DIR, model};
use crate::settings::Loaded;
use crate::ticket::{self, Ticket};
use crate::{Error, gofmt, repos, write};

/// Go's `OverviewPath`: a plan's README, relative to the root.
pub fn overview_path(change: &str) -> String {
    format!("{SPECS_DIR}/{change}/README.md")
}

/// Go's `overviewError`: `daiku: <rel>: <err>` without a doubled prefix.
fn overview_error(rel: &str, err: &str) -> Error {
    Error::Overview {
        rel: rel.to_string(),
        detail: err.strip_prefix("daiku: ").unwrap_or(err).to_string(),
    }
}

/// Go's `BuildTicket`: a build's ticket, read in place from the first source
/// that parses as a ticket with an id.
pub fn build_ticket(root: &Path, c: &Change) -> Option<Ticket> {
    for src in &c.sources {
        let Ok(Some(data)) = read_file(&at(root, src)) else {
            continue;
        };
        if let Ok(mut t) = Ticket::parse(&data)
            && !t.id.is_empty()
        {
            t.path = shown(root, src);
            return Some(t);
        }
    }
    None
}

/// Go's `isPlan`: a recorded plan change with a folder.
fn is_plan(root: &Path, changes: &BTreeMap<String, Change>, name: &str) -> bool {
    if name.is_empty() || !gofmt::is_local(name) || name.contains(['/', '\\']) {
        return false;
    }
    if changes.get(name).is_none_or(|c| c.loop_ != PLAN_LOOP) {
        return false;
    }
    std::fs::metadata(at(root, &format!("{PLAN_DIR}/{name}"))).is_ok_and(|m| m.is_dir())
}

/// Go's `planOf`: the plan a build came from, by its ticket's path or its
/// `change` field; "" for a plan change or when there's none.
pub fn plan_of(root: &Path, changes: &BTreeMap<String, Change>, c: &Change) -> String {
    if c.loop_ != BUILD_LOOP {
        return String::new();
    }
    let mut name = String::new();
    for src in &c.sources {
        if let Some(rest) = src.strip_prefix(&format!("{SPECS_DIR}/")) {
            let parts: Vec<&str> = rest.split('/').collect();
            if parts.len() == 3 && parts[1] == "tickets" {
                name = parts[0].to_string();
                break;
            }
        }
    }
    if name.is_empty()
        && let Some(t) = build_ticket(root, c)
    {
        name = t.change;
    }
    if is_plan(root, changes, &name) {
        name
    } else {
        String::new()
    }
}

/// Go's `overviewTarget`: the plan whose overview a change refreshes.
fn target(store: &Store, c: &Change) -> String {
    if c.loop_ == BUILD_LOOP {
        return match store.read_changes() {
            Ok(changes) => plan_of(store.root, &changes, c),
            Err(_) => String::new(),
        };
    }
    c.name.clone()
}

/// Go's `BuildRows`: the plan's tickets that name a repo, and builds that came
/// from the plan, by id, with each build's progress.
pub fn build_rows(
    root: &Path,
    plan: &str,
    changes: &BTreeMap<String, Change>,
) -> Result<Vec<BuildRow>, Error> {
    let rel = format!("{PLAN_DIR}/{plan}/tickets");
    let (ts, _) = ticket::load_dir(&at(root, &rel), &shown(root, &rel))?;
    let mut rows: BTreeMap<String, BuildRow> = BTreeMap::new();
    for t in ts {
        if !rows.contains_key(&t.id) && !t.repo.is_empty() {
            rows.insert(
                t.id.clone(),
                BuildRow {
                    id: t.id.clone(),
                    repo: t.repo.clone(),
                    ..BuildRow::default()
                },
            );
        }
    }
    for (name, c) in changes {
        if c.loop_ != BUILD_LOOP || rows.contains_key(name) || plan_of(root, changes, c) != plan {
            continue;
        }
        rows.insert(
            name.clone(),
            BuildRow {
                id: name.clone(),
                repo: c.repos.first().cloned().unwrap_or_default(),
                ..BuildRow::default()
            },
        );
    }
    Ok(rows
        .into_values()
        .map(|mut row| {
            if let Some(c) = changes.get(&row.id).filter(|c| c.loop_ == BUILD_LOOP) {
                row.started = true;
                row.phase = c.phase.clone();
                row.status = c.status.clone();
                row.closed = c.is_closed();
                if let Some(r) = c.repos.first() {
                    row.repo = r.clone();
                }
            }
            row
        })
        .collect())
}

/// Go's `Overview`: the plan's README with its generated region rendered, the
/// README as it was (`None` when missing), and its path.
pub fn overview(
    store: &Store,
    change: &str,
    settings: &Loaded,
) -> Result<(String, Option<String>), Error> {
    let root = store.root;
    let adr_dir = &settings.effective.adr_dir;
    let m = model::load(root, change, adr_dir)?;
    let changes = store.read_changes()?;
    let empty = BTreeMap::new();
    let (gates, reasons) = match changes.get(change) {
        Some(c) => (&c.gates, &c.reasons),
        None => (&empty, &empty),
    };
    let reg = repos::load(root)?;
    let rel = overview_path(change);
    let existing = read_file(&at(root, &rel))
        .map_err(|text| Error::PathError { text })?
        .map(|d| String::from_utf8_lossy(&d).into_owned());
    let base = existing.clone().unwrap_or_else(|| markers::stub(&m.title));
    let narrative = markers::narrative(&base).map_err(|text| Error::PathError { text })?;
    let builds = build_rows(root, change, &changes)?;
    let region = render(&Input {
        model: &m,
        phases: machine::PLAN.names().collect(),
        gates,
        reasons,
        repos: reg.names(),
        adr_dir,
        narrative_empty: markers::narrative_empty(&narrative),
        builds,
    });
    let merged = markers::merge(&base, &region).map_err(|text| Error::PathError { text })?;
    Ok((merged, existing))
}

/// Go's `RefreshOverview` for one plan: writes its README when it changed and
/// returns the paths written.
pub fn refresh(store: &Store, plan: &str, settings: &Loaded) -> Result<Vec<String>, Error> {
    let root = store.root;
    if settings.effective.overview != "auto" || plan.is_empty() {
        return Ok(Vec::new());
    }
    let rel = overview_path(plan);
    let (merged, existing) =
        overview(store, plan, settings).map_err(|e| overview_error(&rel, &e.to_string()))?;
    if existing.as_deref() == Some(merged.as_str()) {
        return Ok(Vec::new());
    }
    write::write_file(root, &at(root, &rel), merged.as_bytes())
        .map_err(|e| overview_error(&rel, &e.to_string()))?;
    Ok(vec![rel])
}

/// The store's refresh hook: the overview of the plan `c` belongs to.
pub(crate) fn refresh_for(store: &Store, c: &Change) -> Result<Vec<String>, Error> {
    let Some(settings) = store.settings() else {
        return Ok(Vec::new());
    };
    if settings.effective.overview != "auto" {
        return Ok(Vec::new());
    }
    refresh(store, &target(store, c), settings)
}

/// Go's `CheckOverview`: the README the transition will refresh must be safe to
/// write, checked before anything changes.
pub fn check(store: &Store, c: &Change) -> Result<(), Error> {
    let settings = crate::settings::load(store.root, &store.env.dirs)?;
    if settings.effective.overview != "auto" {
        return Ok(());
    }
    let plan = target(store, c);
    if plan.is_empty() {
        return Ok(());
    }
    write::check_path(store.root, &at(store.root, &overview_path(&plan)))
}

/// Go's `CheckClose`: [`check`] for the change `close` names, recorded or a
/// plan folder.
pub fn check_close(store: &Store, name: &str) -> Result<(), Error> {
    let changes = store.read_changes()?;
    if let Some(c) = changes.get(name) {
        return check(store, c);
    }
    if name.is_empty() || !gofmt::is_local(name) || name.contains(['/', '\\']) {
        return Ok(());
    }
    if !std::fs::symlink_metadata(at(store.root, &format!("{PLAN_DIR}/{name}")))
        .is_ok_and(|m| m.is_dir())
    {
        return Ok(());
    }
    let c = Change {
        name: name.to_string(),
        loop_: PLAN_LOOP.to_string(),
        ..Change::default()
    };
    check(store, &c)
}
