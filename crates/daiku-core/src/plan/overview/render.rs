//! The generated region of a plan's README. A port of Go's
//! `internal/plan/overview/overview.go`; every section's text matches Go's.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::plan::model::{Model, ticket_change};
use crate::plan::problems::problems;
use crate::plan::spec::Severity;
use crate::ticket::{self, Ticket};

/// One ticket of a plan and its build, if there is one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildRow {
    pub id: String,
    pub repo: String,
    /// The build's; empty when it isn't started.
    pub phase: String,
    pub status: String,
    /// A build change by the ticket's id exists.
    pub started: bool,
    pub closed: bool,
}

impl BuildRow {
    /// The row's build as one cell.
    pub fn build(&self) -> String {
        if !self.started {
            "not started".into()
        } else if self.closed {
            "closed".into()
        } else if self.phase == "ship" && self.status == "complete" {
            format!("{} {} (done)", self.phase, self.status)
        } else {
            format!("{} {}", self.phase, self.status)
        }
    }
}

/// What the overview is rendered from.
pub struct Input<'a> {
    pub model: &'a Model,
    /// The machine's phases, in order.
    pub phases: Vec<&'static str>,
    pub gates: &'a BTreeMap<String, String>,
    pub reasons: &'a BTreeMap<String, String>,
    /// `REPOS.json` names, for validation.
    pub repos: Vec<String>,
    /// The `adrDir` setting, for links.
    pub adr_dir: &'a str,
    pub narrative_empty: bool,
    /// The plan's tickets and their builds, by id.
    pub builds: Vec<BuildRow>,
}

const NONE: &str = "_None yet._";

/// Past this many nodes, a dependency group is a table rather than a diagram.
const MAX_NODES: usize = 25;

/// From `plan/specs/<change>/` back to the root.
const TO_ROOT: &str = "../../../";

/// Go's `Render`: the region between the generated markers.
pub fn render(input: &Input) -> String {
    let r = Renderer::new(input);
    let sections: [(&str, String); 9] = [
        ("Status", r.status()),
        ("Diagrams", r.diagrams()),
        ("Work map", r.work_map()),
        ("Dependencies", r.dependencies()),
        ("Repos", r.repos()),
        ("Builds", r.builds()),
        ("Decisions", r.decisions()),
        ("Traceability", r.traceability()),
        ("Problems", r.problems()),
    ];
    let mut b = String::from("\n");
    for (i, (title, body)) in sections.iter().enumerate() {
        if i > 0 {
            b.push('\n');
        }
        b.push_str(&format!("## {title}\n\n{body}\n"));
    }
    b.push('\n');
    b
}

struct Renderer<'a> {
    input: &'a Input<'a>,
    m: &'a Model,
    /// The change's tickets, by id (indexes into the model's tickets).
    mine: Vec<usize>,
    /// Every ticket in the workspace, the first for each id.
    by_id: HashMap<&'a str, usize>,
    /// Ids of the change's tickets.
    in_this: BTreeSet<&'a str>,
    /// The change's tickets, by path.
    by_path: BTreeSet<&'a str>,
}

fn cell(s: &str) -> String {
    s.chars()
        .map(|c| {
            let n = c as u32;
            if n < 0x20 || n == 0x7f || n == 0x85 || n == 0x2028 || n == 0x2029 {
                ' '
            } else {
                c
            }
        })
        .collect::<String>()
        .replace('|', "\\|")
}

fn ticket_link(id: &str) -> String {
    format!("[{id}](tickets/{id}.md)")
}

fn node_id(id: &str) -> String {
    id.replace('-', "_")
}

fn label(id: &str, title: &str) -> String {
    let runes: Vec<char> = title.chars().collect();
    let title = if runes.len() > 40 {
        let cut: String = runes[..39].iter().collect();
        format!("{}…", cut.trim_end_matches(' '))
    } else {
        title.to_string()
    };
    format!("{id} · {title}").replace('"', "#quot;")
}

/// The base name of a slash path.
fn base(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

/// The folder of a slash path, `.` for none.
fn dir(p: &str) -> &str {
    match p.rsplit_once('/') {
        Some(("", _)) => "/",
        Some((d, _)) => d,
        None => ".",
    }
}

impl<'a> Renderer<'a> {
    fn new(input: &'a Input<'a>) -> Renderer<'a> {
        let m = input.model;
        let mut r = Renderer {
            input,
            m,
            mine: Vec::new(),
            by_id: HashMap::new(),
            in_this: BTreeSet::new(),
            by_path: BTreeSet::new(),
        };
        for (i, t) in m.tickets.iter().enumerate() {
            r.by_id.entry(t.id.as_str()).or_insert(i);
            if ticket_change(t) == m.change {
                r.mine.push(i);
                r.in_this.insert(&t.id);
                r.by_path.insert(&t.path);
            }
        }
        r.mine
            .sort_by(|&a, &b| m.tickets[a].id.cmp(&m.tickets[b].id));
        r
    }

    fn t(&self, i: usize) -> &'a Ticket {
        &self.m.tickets[i]
    }

    fn status(&self) -> String {
        let mut b = String::from("| Phase | Status | Note |\n|---|---|---|\n");
        for p in &self.input.phases {
            let g = self.input.gates.get(*p).map(String::as_str).unwrap_or("");
            let g = if g.is_empty() { "—" } else { g };
            let note = self.input.reasons.get(*p).map(String::as_str).unwrap_or("");
            b.push_str(&format!("| {p} | {} | {} |\n", cell(g), cell(note)));
        }
        b.trim_end_matches('\n').to_string()
    }

    fn diagrams(&self) -> String {
        let link = "From [architecture.md](architecture.md).";
        if self.m.diagrams.is_empty() {
            return if self.m.has_architecture {
                format!("{NONE}\n\n{link}")
            } else {
                NONE.into()
            };
        }
        let parts: Vec<String> = self
            .m
            .diagrams
            .iter()
            .map(|d| {
                let body = if d.source.is_empty() {
                    "_No diagram yet._".to_string()
                } else {
                    format!("```mermaid\n{}\n```", d.source)
                };
                format!("### {}\n\n{body}", d.heading)
            })
            .collect();
        format!("{}\n\n{link}", parts.join("\n\n"))
    }

    fn children(&self) -> HashMap<&'a str, Vec<usize>> {
        let mut kids: HashMap<&str, Vec<usize>> = HashMap::new();
        for &i in &self.mine {
            let t = self.t(i);
            if !t.parent.is_empty() && self.in_this.contains(t.parent.as_str()) {
                kids.entry(&t.parent).or_default().push(i);
            }
        }
        kids
    }

    fn work_map(&self) -> String {
        if self.mine.is_empty() {
            return NONE.into();
        }
        let kids = self.children();
        let mut seen = BTreeSet::new();
        let mut b = String::new();
        // An explicit stack in place of Go's recursion: children in order.
        let walk = |root: usize, b: &mut String, seen: &mut BTreeSet<usize>| {
            let mut stack = vec![(root, 0usize)];
            while let Some((i, depth)) = stack.pop() {
                if !seen.insert(i) {
                    continue;
                }
                b.push_str(&format!(
                    "{}- {}\n",
                    "  ".repeat(depth),
                    self.work_line(self.t(i))
                ));
                if let Some(ks) = kids.get(self.t(i).id.as_str()) {
                    for &k in ks.iter().rev() {
                        stack.push((k, depth + 1));
                    }
                }
            }
        };
        for &i in &self.mine {
            let t = self.t(i);
            if t.parent.is_empty() || !self.in_this.contains(t.parent.as_str()) {
                walk(i, &mut b, &mut seen);
            }
        }
        for &i in &self.mine {
            walk(i, &mut b, &mut seen);
        }
        b.trim_end_matches('\n').to_string()
    }

    fn work_line(&self, t: &Ticket) -> String {
        let mut parts = vec![format!("[**{}**](tickets/{}.md) {}", t.id, t.id, t.title)];
        if !t.repo.is_empty() {
            parts.push(format!("`{}`", t.repo));
        }
        if !t.status.is_empty() {
            parts.push(t.status.clone());
        }
        if !t.external.is_empty() {
            let ext: Vec<String> = t.external.iter().map(|(k, v)| format!("{k} {v}")).collect();
            parts.push(ext.join(", "));
        }
        if !t.requirements.is_empty() {
            parts.push(t.requirements.join(", "));
        }
        parts.join(" · ")
    }

    fn epic_of(&self, i: usize) -> String {
        let mut seen = BTreeSet::new();
        let mut cur = Some(self.t(i));
        while let Some(c) = cur {
            if !seen.insert(c.id.as_str()) {
                break;
            }
            if c.typ == ticket::EPIC {
                return c.id.clone();
            }
            if c.parent.is_empty() || !self.in_this.contains(c.parent.as_str()) {
                return String::new();
            }
            cur = self.by_id.get(c.parent.as_str()).map(|&j| self.t(j));
        }
        String::new()
    }

    fn dependencies(&self) -> String {
        if self.mine.is_empty() {
            return NONE.into();
        }
        let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for &i in &self.mine {
            groups.entry(self.epic_of(i)).or_default().push(i);
        }
        let mut parts = Vec::new();
        for (e, members) in &groups {
            if e.is_empty() {
                continue;
            }
            let title = self
                .by_id
                .get(e.as_str())
                .map(|&j| self.t(j).title.as_str())
                .unwrap_or("");
            parts.push(format!("### {e} · {title}\n\n{}", self.group(members)));
        }
        if let Some(g) = groups.get("").filter(|g| !g.is_empty()) {
            parts.push(format!("### No epic\n\n{}", self.group(g)));
        }
        parts.join("\n\n")
    }

    fn group(&self, members: &[usize]) -> String {
        let ids: BTreeSet<&str> = members.iter().map(|&i| self.t(i).id.as_str()).collect();
        let mut edges: Vec<(&str, &str)> = Vec::new();
        let mut external: BTreeSet<&str> = BTreeSet::new();
        for &i in members {
            let t = self.t(i);
            for d in &t.depends_on {
                edges.push((d, &t.id));
                if !ids.contains(d.as_str()) {
                    external.insert(d);
                }
            }
        }
        edges.sort_by(|a, b| a.1.cmp(b.1).then(a.0.cmp(b.0)));
        if members.len() + external.len() > MAX_NODES {
            let mut b = String::from("| Ticket | Depends on |\n|---|---|\n");
            for &i in members {
                let t = self.t(i);
                let deps: Vec<String> = t
                    .depends_on
                    .iter()
                    .map(|d| format!("{d}{}", self.other_change(d)))
                    .collect();
                b.push_str(&format!(
                    "| {} | {} |\n",
                    ticket_link(&t.id),
                    cell(&deps.join(", "))
                ));
            }
            return b.trim_end_matches('\n').to_string();
        }
        let mut b = String::from("```mermaid\nflowchart LR\n");
        for &i in members {
            let t = self.t(i);
            b.push_str(&format!(
                "  {}[\"{}\"]\n",
                node_id(&t.id),
                label(&t.id, &t.title)
            ));
        }
        for id in &external {
            let title = self
                .by_id
                .get(id)
                .map(|&j| self.t(j).title.as_str())
                .unwrap_or("not found");
            b.push_str(&format!(
                "  {}[\"{}{}\"]:::external\n",
                node_id(id),
                label(id, title),
                self.other_change(id)
            ));
        }
        for (from, to) in &edges {
            b.push_str(&format!("  {} --> {}\n", node_id(from), node_id(to)));
        }
        if !external.is_empty() {
            b.push_str("  classDef external stroke-dasharray: 5 5\n");
        }
        b.push_str("```");
        b
    }

    fn other_change(&self, id: &str) -> String {
        match self.by_id.get(id) {
            Some(&j) => {
                let c = ticket_change(self.t(j));
                if c != self.m.change {
                    format!(" ({c})")
                } else {
                    String::new()
                }
            }
            None => String::new(),
        }
    }

    fn repos(&self) -> String {
        let mut rows: BTreeMap<&str, (HashMap<&str, usize>, Vec<String>)> = BTreeMap::new();
        for &i in &self.mine {
            let t = self.t(i);
            if t.repo.is_empty() {
                continue;
            }
            let row = rows.entry(&t.repo).or_default();
            *row.0.entry(&t.status).or_default() += 1;
            row.1.push(ticket_link(&t.id));
        }
        if rows.is_empty() {
            return NONE.into();
        }
        let mut b =
            String::from("| Repo | draft | ready | published | Tickets |\n|---|---|---|---|---|\n");
        for (name, (counts, ids)) in &rows {
            let n = |s: &str| counts.get(s).copied().unwrap_or(0);
            b.push_str(&format!(
                "| `{name}` | {} | {} | {} | {} |\n",
                n(ticket::STATUS_DRAFT),
                n(ticket::STATUS_READY),
                n(ticket::STATUS_PUBLISHED),
                ids.join(", ")
            ));
        }
        b.trim_end_matches('\n').to_string()
    }

    fn builds(&self) -> String {
        if self.input.builds.is_empty() {
            return NONE.into();
        }
        let mut b = String::from("| Ticket | Repo | Build |\n|---|---|---|\n");
        for row in &self.input.builds {
            let id = if self.in_this.contains(row.id.as_str()) {
                ticket_link(&row.id)
            } else {
                cell(&row.id)
            };
            let repo = if row.repo.is_empty() {
                "—".to_string()
            } else {
                format!("`{}`", cell(&row.repo))
            };
            b.push_str(&format!("| {id} | {repo} | {} |\n", cell(&row.build())));
        }
        b.trim_end_matches('\n').to_string()
    }

    fn decisions(&self) -> String {
        let mut cited_by: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for &i in &self.mine {
            let t = self.t(i);
            for a in &t.adrs {
                let ids = cited_by.entry(a).or_default();
                if ids.last() != Some(&t.id.as_str()) {
                    ids.push(&t.id);
                }
            }
        }
        if cited_by.is_empty() {
            return NONE.into();
        }
        let mut b = String::from("| ADR | Title | Status | Cited by |\n|---|---|---|---|\n");
        for (a, ids) in &cited_by {
            let (name, title, status) = match self.m.adrs.get(*a) {
                Some(rec) => (
                    format!(
                        "[{a}]({TO_ROOT}{}/{})",
                        crate::gofmt::to_slash(self.input.adr_dir),
                        base(&rec.path)
                    ),
                    cell(&rec.title),
                    cell(&rec.status),
                ),
                None => (cell(a), "not found".to_string(), String::new()),
            };
            let by: Vec<String> = ids.iter().map(|id| ticket_link(id)).collect();
            b.push_str(&format!(
                "| {name} | {title} | {status} | {} |\n",
                by.join(", ")
            ));
        }
        b.trim_end_matches('\n').to_string()
    }

    fn traceability(&self) -> String {
        if !self.m.has_spec || self.m.requirements.is_empty() {
            return NONE.into();
        }
        let mut covered: HashMap<&str, Vec<&str>> = HashMap::new();
        for &i in &self.mine {
            let t = self.t(i);
            for id in &t.requirements {
                let ids = covered.entry(id).or_default();
                if ids.last() != Some(&t.id.as_str()) {
                    ids.push(&t.id);
                }
            }
        }
        let mut b = String::from(
            "From [functional-spec.md](functional-spec.md).\n\n| Req | Priority | Covered by |\n|---|---|---|\n",
        );
        for req in &self.m.requirements {
            let p = if req.priority.is_empty() {
                "—"
            } else {
                &req.priority
            };
            let links: Vec<String> = covered
                .get(req.id.as_str())
                .map(|ids| ids.iter().map(|id| ticket_link(id)).collect())
                .unwrap_or_default();
            let mut by = links.join(", ");
            if by.is_empty() && req.kind == 'R' && req.priority == "must" {
                by = "**uncovered**".into();
            }
            b.push_str(&format!("| {} | {p} | {by} |\n", req.id));
        }
        b.trim_end_matches('\n').to_string()
    }

    fn problem_id(id: &str) -> String {
        if id.ends_with(".md") {
            format!("tickets/{}", base(id))
        } else {
            id.to_string()
        }
    }

    fn problems(&self) -> String {
        let mut items: Vec<(&str, String, String)> = Vec::new();
        let all: Vec<&Ticket> = self.m.tickets.iter().collect();
        for p in ticket::validate_workspace(&all, &self.input.repos) {
            if self.in_this.contains(p.id.as_str()) || self.by_path.contains(p.id.as_str()) {
                items.push(("✗", Self::problem_id(&p.id), p.message));
            }
        }
        for p in &self.m.ticket_problems {
            let d = dir(&p.id);
            if base(d) == "tickets" && base(dir(d)) == self.m.change {
                items.push(("✗", Self::problem_id(&p.id), p.message.clone()));
            }
        }
        for f in problems(self.m) {
            if f.change != self.m.change {
                continue;
            }
            let mark = if f.severity == Severity::Warning {
                "!"
            } else {
                "✗"
            };
            items.push((mark, f.id, f.message));
        }
        if self.input.gates.get("architecture").map(String::as_str) == Some("approved")
            && self.input.narrative_empty
        {
            items.push(("!", "README".into(), "narrative is empty".into()));
        }
        if items.is_empty() {
            return NONE.into();
        }
        items.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.cmp(&b.2)));
        let mut b = String::new();
        for (mark, id, msg) in &items {
            let id = if self.in_this.contains(id.as_str()) {
                ticket_link(id)
            } else {
                id.clone()
            };
            b.push_str(&format!("- {mark} {id}: {msg}\n"));
        }
        b.trim_end_matches('\n').to_string()
    }
}
