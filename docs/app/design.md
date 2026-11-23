# daiku-app design

`daiku-app` is a desktop app over a daiku workspace. This page is its technical
design for the first phase, a read-only viewer: the UI stack, how the app is put
together, what it shows, and what it needs from `daiku-core`. No app code exists
yet.

The filesystem stays the source of truth. The app is a view over files that agents
and the CLI are already changing, it keeps no workspace state of its own, and daiku
works the same without it.

Screens, below, gives the layout and the look of every view.

## Phases

1. **A read-only viewer** with a ticket board and live updates. This page covers
   this phase.
2. Editing: markdown, moving tickets and switching changes, written back through
   core.
3. Embedded terminals running the agent CLIs people already use.
4. Whiteboards.
5. Chat through the Agent Client Protocol, only if terminals aren't enough.

## Stack

The app is built with **Tauri 2** and a **React + TypeScript** frontend
(ADR-0007).

- **Core in-process.** Tauri's backend is Rust, so it links `daiku-core` directly,
  with no sidecar and no subprocess.
- **Later phases.** The webview reaches the components phases 2–4 need: xterm.js
  for terminals, CodeMirror for markdown, and tldraw or Excalidraw for whiteboards.
  tldraw and Excalidraw are React components, which is why the frontend is React.
- **Components.** shadcn/ui on Tailwind v4: its source is copied into the app, with
  the design system's tokens as its CSS variables (ADR-0009). The source is ours to
  style, and there is no runtime CSS-in-JS. HeroUI is out because its resizable panes
  and segmented control are paid. Ant Design is out because of its strong dashboard
  look and styles generated at runtime.
- **Packaging.** Tauri bundles for macOS, Windows and Linux.

Rejected:

| Option | Why not |
|---|---|
| GPUI | Native and fast, but terminals, rich markdown editing and a whiteboard would all be built by hand. Its docs are thin, its API moves with Zed's, and its support outside macOS is younger. |
| Wails v3 | Its backend is Go, and core is Rust. (The desktop idea preferred Wails when core was Go.) |
| Iced, egui, Slint | The same gaps as GPUI, with weaker text and editor widgets. |
| Electron | A Node backend reaches core only by running `daiku --json`, and it ships its own browser engine. |
| Dioxus desktop | The same system webview as Tauri, but the UI is written in Rust, so React components can't be used directly. |

The decision is worth reopening if GPUI gains reusable terminal and editor
components, or if WebKitGTK proves unusable on Linux for phases 3–4.

## Architecture

### Where the code lives

```
crates/daiku-app/        Cargo workspace member, not in default-members
  src/main.rs            Tauri setup: registers the commands, owns the watcher
  src/commands.rs        one Tauri command per read
  src/watch.rs           the file watcher (see Live updates)
  ui/                    the React + TypeScript frontend (Vite)
```

`daiku-app` is a member of the Cargo workspace but not of `default-members`. So
`cargo test` and the release keep building only core, the CLI and the fixture
runner, and Tauri's dependencies (WebKitGTK on Linux) stay off their CI jobs. The
app gets its own CI job when it's built. Core's dependency allowlist (ADR-0005)
covers core and the CLI, not the app.

### How the UI gets data

Each Tauri command opens a `daiku_core::Workspace` and calls one core function.
It returns what `--json` would print: a `daiku_core::json::Success<T>` built with
`Success::from_outcome`, or `daiku_core::json::failure(&err)` (see
[`--json` output](../json.md)). The app doesn't run the `daiku` binary.

The frontend's TypeScript types are generated from the published schemas in
[`docs/json-schema/`](../json-schema/) as part of the app's build, for example with
`json-schema-to-typescript`. Core gains no dependency for this, and a script or a
mod that reads `--json` can use the same types.

### Threads

Core is synchronous and has no async runtime (ADR-0003). Each command runs its
core call through `tauri::async_runtime::spawn_blocking`, so a slow disk never
blocks the UI thread. `Env` holds boxed traits with no `Send` bound, so a
`Workspace` can't cross threads: each command builds its `Env` and opens its
`Workspace` inside the closure.

The app's `daiku_core::Env` uses the real clock and process runner and an `Events`
that ignores events: phase one runs no git and no clone. Those implementations
(`SystemClock`, `ChildProcess`, `NoEvents`) live in the CLI binary, in
`crates/daiku-cli/src/env.rs`, which the app can't depend on, so the app carries its
own copies. Whether to share them instead is left to the app's build ticket.

### Errors

A `Failure` carries a `kind` and a message (`daiku_core::json::error_kind`). The
view that asked shows it inline and keeps its last good data. Core's `Warning`s
come back in the envelope's `warnings` and are shown with the data.

### Workspace text

Titles, names, reasons and file bodies are shown through `Text::display`
(ADR-0008), the rule the CLI applies to its output, so a control character or an
invalid byte in a workspace file shows as an escape in both clients and can't act
on the view.

### View state

The app keeps only view state: recent workspaces, the last workspace and view
opened, and window size. It's stored in Tauri's app config folder. The app writes
nothing under a workspace and never caches workspace data on disk.

## What phase one shows

| View | Core call | Shows |
|---|---|---|
| Workspaces | none (view state) | recent workspace roots; open one |
| Overview | `Workspace::status()` | the active change; open plans with phase, status and gates; builds by repo, with the build holding each repo's lock |
| Board | `Workspace::tickets_list()` and `status()` | every ticket in one of the five columns below; filter by plan change and repo |
| Card | a `TicketView` | id, title, type, repo, `depends_on`; badges for drifted, published and frozen; the build's phase and status if one started |
| Ticket | a `TicketView` | its frontmatter fields, and its body as read-only markdown |
| Change | `status()`, and `status_plan(name)` for a plan | each phase in `Change::machine()` order with its gate and any skip reason; for a plan, its `BuildRow`s |
| Ideas, ADRs | `Workspace::idea_list()`, `adr_list()` | each idea's id, title and status; each ADR's number, title and status |

### Board columns

A ticket's own `status` field isn't updated when its build ships, so the column
comes from build state first. For each ticket, from every plan change and the
inbox:

| Column | Rule |
|---|---|
| Closed | a build change with the ticket's id exists and is closed |
| Done | a build exists and its `ship` gate is `complete` |
| In build | a build exists, and neither of the above applies |
| Ready | no build, and `status` is `ready` or `published` |
| Draft | no build, and `status` is `draft` |

The rules are checked in that order, so every ticket lands in exactly one column.
Drifted, published and frozen are badges on the card, not columns. The rule is a
view rule, computed in the app from `TicketView` and `Status`, not in core.

## Screens

The window is quiet, dark and dense, a calm frame around the work. This section
gives the frame, the two tabs, the nudge, how the window folds, how a workspace
opens, and the look. The values are the starting design system. They become CSS
variables when the app is built.
The layout comes from the owner's wireframes and mockups, and the component kit from
a comparison of shadcn/ui, HeroUI and Ant Design. Those working files live outside
this repo.

### The frame

One frame serves both tabs. Left to right: the workspaces rail, the terminal, the
viewer and the list. The terminal and the viewer share the width and run full height.

| Part | Size | What it holds |
|---|---|---|
| Title bar | 44px | The app name, the Plan/Build switch (centred), and the focus switch: Split, Terminal, Viewer, Board |
| Workspaces rail | 52px | One chip per open workspace, and a "+" to open another |
| Terminal | shares the width | The terminal pane |
| Viewer | shares the width | The open file, ticket or plan artifact |
| List | 340px, on the right | Tickets on Build; plans, ideas, ADRs and references on Plan |
| Pane header | 36px | The pane's name or the open file's path, and its fill-the-window button |
| Status bar | 30px | Where you are, and the nudge |

- **Focus.** The focus switch picks Split (all panes), Terminal, Viewer or Board. Each
  pane's arrow fills this window. It doesn't open a new one.
- **The terminal arrives with terminals** (phase 3 in Phases above). In phase one the
  column is hidden and the viewer takes its width. This section doesn't change the
  phase order.
- **No create actions.** Phase one has no "+" to create tickets, plans or ideas,
  because creating is a write (phase 2). The rail's "+" opens a workspace and is the
  exception.

### Build

The list holds tickets in four groups, in this order. Each header has a status mark.

| Group | Mark |
|---|---|
| In build | half-filled circle |
| Ready | circle |
| Draft | dashed circle |
| Done | circle with a check |

The next ticket is marked: its row sits on a distinct surface with a 3px amber edge on
the left. Long groups fold to "N more". The list leaves out Closed, which isn't work.
Board focus opens the full board, with all five columns of Board columns.

### Plan

The list holds four groups, in this order. Each header has an icon.

| Group | Icon |
|---|---|
| Plans | bulleted list |
| Ideas | bulb |
| ADRs | scale |
| References | book |

The viewer shows a phase strip over the selected plan's artifact. A skipped phase is
marked with its reason. Board focus lays plans out by phase: one card per plan, in its
current phase.

References show in the Plan list, but they aren't a phase-one view, because listing
them needs a core call that doesn't exist yet. That is a follow-up.

### Where each view lives

Every view in "What phase one shows" has a place in the frame.

| View | Where it appears |
|---|---|
| Workspaces | The rail, and the launch screen when none is open |
| Overview | The status bar, and the Plan list; builds and their repo locks in the Build list's In build group |
| Board | Board focus |
| Card | A row in the list, and a card on the board |
| Ticket | The viewer |
| Change | The viewer; for a plan, the phase strip |
| Ideas, ADRs | Groups in the Plan list |

### The nudge

The status bar carries the nudge. It starts "Next: …". For example:

> Next: N-0020 is a draft. Start its build when you're ready.

In a narrow window it shortens:

> Next: N-0020 · draft

Amber marks only what needs the person: the nudge, the Next pill and edge, the count
that holds the next item, and draft status labels. Nothing decorative is amber.

Which item counts as next, and where that rule lives (core or the app), is a
follow-up.

### Narrow windows

- From 1440px: four columns.
- Below 1440px: the list folds into a rail (52px) with counts.
- Below 1000px: one pane, with Terminal, Viewer and Tickets tabs. The Terminal tab
  arrives with terminals.

The workspaces rail, the Plan/Build switch (centred at every width) and a shortened
status bar always stay.

### Opening a workspace

- Several workspaces open in one window, one chip each in the rail.
- The rail's "+" opens the system folder picker.
- On launch the app reopens the last workspace.
- With none open (first launch, or the last one gone), a launch screen shows recent
  workspaces, "Open workspace…" and a drop zone.
- A folder with no `.daiku/` shows "This folder isn't a daiku workspace", with the
  `daiku init` command to copy. Phase one doesn't write.
- Dropping a folder on the window or the Dock icon is a nice-to-have.
- Opening from the terminal (`daiku app`) is out for now.

### The look

Principles:

- Dark only for now.
- Quiet and native, not a web dashboard.
- One accent, amber, meaning "needs you".
- Blue is only for the terminal prompt and links.
- Dense, and laptop first: every screen works at 1440x900.
- Nothing the folders don't say. Every label comes from the CLI's `--json`.
- The terminal gets out of the way. With focus, every keystroke goes to it.

Colours:

| Role | Token | Hex | Use |
|---|---|---|---|
| Window | `surface-window` | #1a1a1c | The viewer, the board, empty space |
| Chrome | `surface-chrome` | #1d1d20 | Title bar, rails, list and status bar |
| Terminal | `surface-terminal` | #141416 | The terminal pane only |
| Raised | `surface-raised` | #26262a | Cards, the switch track, row hover |
| Selected | `surface-selected` | #3a3a40 | The active segment of a switch |
| Next | `surface-next` | #2b2a26 | Behind the Next row or card |
| Line | `line` | #2e2e33 | Pane borders and header hairlines |
| Ink | `ink` | #ececef | Primary text |
| Ink secondary | `ink-secondary` | #c9c9cf | Body text, list titles |
| Ink muted | `ink-muted` | #a4a4ab | Meta text, status bar text |
| Ink faint | `ink-faint` | #9a9aa2 | Group labels, counts, empty states |
| Attention | `attention` | #f0b45a | The one accent: something needs the person |
| Attention ink | `attention-ink` | #f0c27a | Attention as text, such as the nudge |
| On attention | `on-attention` | #1a1a1c | Text on an amber fill |
| Prompt | `prompt` | #7aa7ff | The terminal prompt and links |

The design system also has tokens for group bands (`surface-band`), the active
workspace chip (`surface-workspace`), more line weights (`line-band`, `line-control`,
`line-hover`), terminal text (`ink-terminal`), the outline and fill for a card or chip
marked Next or draft (`attention-line`, `attention-surface`) and link hover
(`prompt-hover`).

Type uses two families: Inter (sans) and JetBrains Mono (mono). Ids, paths and
commands are always mono.

| Style | Family | Size | Weight |
|---|---|---|---|
| Title | sans | 20px | 600 |
| Body | sans | 13px | 400 |
| Lists and secondary | sans | 12px | 400 |
| Label | sans | 11px | 600, uppercase, 0.06em |
| Id | mono | 12px | 500 |
| Terminal | mono | 12px | 400 |

The spacing scale is 4, 8, 12, 16 and 24px. Radii are 4, 6 and 8px, 12px for the
window, and a pill. There are no shadows. Icons are thin outlines with a 1.8px stroke.

## What the app needs from core

| Need | In `daiku-core` |
|---|---|
| Open a workspace | `Workspace::open` (`workspace.rs`) |
| The active change, open plans, builds by repo | `Workspace::status()` → `changes::Status` (`changes.rs`) |
| A plan and its builds | `Workspace::status_plan()` → `changes::PlanStatus`, `plan::overview::BuildRow` |
| A change's phases and gates | `state::Change`, `Change::machine()` (`state/mod.rs`) |
| Ideas | `Workspace::idea_list()` (`room.rs`) |
| ADRs | `Workspace::adr_list()` (`room.rs`) |
| The envelope and errors | `json::Success`, `json::failure`, `json::error_kind` (`json.rs`) |
| Every ticket's fields and body | **missing:** `Workspace::tickets_list()` and `TicketView`, in N-0013 |
| What to watch | **missing:** `Workspace::watch_paths()`, in N-0012 |
| What a file event means | **missing:** `classify()` and `Changed`, in N-0012 |

Phase one needs no core events. `daiku_core::Event` reports progress from
operations such as `repo doctor --clone`, and what changed on disk comes from the
watcher. The paths are under `crates/daiku-core/src/`.

## Live updates

The watcher lives in the app, never in core or the CLI. Core stays free of watcher
and async crates, and gives the app two functions instead: `watch_paths` says what
to watch, and `classify` says what an event means.

1. When a workspace opens, the app shows the frame (see Screens), and starts a
   watcher (the `notify` crate) on `watch_paths()`.
2. An agent or the CLI changes files. For example, gating a build rewrites
   `STATE.json` and `LOG.md`. The watcher collects events until 150 ms pass with
   none, maps each path through `classify`, and drops `None`.
3. The app emits one `workspace-changed` event to the UI with the set of `Changed`
   values. The UI re-queries only the views they affect:
   - `State` → Overview, Board, Change;
   - `Ticket` → Board, and that ticket if it's open;
   - `Idea` → Ideas;
   - `Adr` → ADRs;
   - `Repos` or `Settings` → Overview.
   - the rest (`PlanArtifact`, `BuildArtifact`, `Reference`, `Scaffold`) refresh no
     phase-one view, and are there for later phases.
4. Core writes files in place, so a read can see a half-written file. A read that
   fails keeps the view's last data, shows the error, and is retried on the next
   event.

```mermaid
sequenceDiagram
    participant Agent
    participant FS as Workspace files
    participant W as daiku-app watcher
    participant Core as daiku-core
    participant UI
    Agent->>FS: write STATE.json and a ticket
    FS-->>W: file events
    W->>Core: classify(path) per event (pure)
    Core-->>W: State, Ticket{id}
    W-->>UI: workspace-changed {State, Ticket}
    UI->>W: invoke status, tickets_list
    W->>Core: status(), tickets_list() on the blocking pool
    Core-->>UI: Success envelope (or Failure)
```

## Interfaces

These are the core additions the app needs. They're the target for N-0012 and
N-0013, not code that exists yet. All of them add no dependency, print nothing
and stay synchronous.

### `watch_paths` and `classify` (N-0012)

```rust
// crates/daiku-core/src/watch.rs, re-exported from the crate root.
pub struct WatchPath { pub path: PathBuf, pub recursive: bool }

impl Workspace {
    /// What a client watches: the root non-recursively (STATE.json, REPOS.json,
    /// LOG.md, AGENTS.md), and `.daiku`, `plan`, `build`, `ideas` and
    /// `references` recursively. A missing folder is still listed, so the client
    /// can watch for it. No I/O.
    pub fn watch_paths(&self) -> Vec<WatchPath>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[non_exhaustive]
pub enum Changed {
    State,                                          // STATE.json, .daiku/active, LOG.md
    Ticket { id: String, change: Option<String> },  // plan/specs/<c>/tickets/<id>.md, build/tickets/<id>.md
    PlanArtifact { change: String },                // other files under plan/specs/<c>/, plan/<room>/<c>/…,
                                                    // and plan/00_intake/<n>.md (see below)
    BuildArtifact { repo: String, change: String }, // build/<repo>/changes/<c>/…
    Idea { id: i64 },                               // ideas/<n>-<slug>/…
    Adr,                                            // plan/04_architecture/adr/…
    Repos,                                          // REPOS.json, .daiku/paths.json
    Settings,                                       // .daiku/settings.json
    Reference,                                      // references/…
    Scaffold,                                       // AGENTS.md, room context, persona and template files
}

/// What a change at `rel` (relative to the root, either separator) means.
/// No I/O. `None` for paths daiku doesn't read (`.git`, `.DS_Store`, editor
/// swap files).
pub fn classify(rel: &Path) -> Option<Changed>;
```

`classify` depends on the path alone, so it's cheap to call on every event and
can be tested with a table of paths. A plan change's files live in more than one
room: its spec, architecture and tickets under `plan/specs/<change>/`, its explore
notes under `plan/02_explore/<change>/`, and its intake at `plan/00_intake/<n>.md`.
All of them are `PlanArtifact`, except tickets, which are `Ticket`. An intake's path
has only the change's number, so `classify` returns that number as `change`, and
the client matches it to the plan change whose name starts with `<n>-`. Room files
that belong to no change (`context.md`, `personas/`, `templates/`) are `Scaffold`.

### `tickets_list` and `TicketView` (N-0013)

```rust
// crates/daiku-core/src/tickets.rs
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TicketView {
    pub id: String,
    #[serde(rename = "type")]
    pub typ: String,
    pub title: String,
    pub repo: String,
    pub change: String, // "" for an inbox ticket
    pub inbox: bool,
    pub path: String,
    pub status: String,
    pub depends_on: Vec<String>,
    pub parent: String,
    pub requirements: Vec<String>,
    pub labels: Vec<String>,
    pub external: BTreeMap<String, String>,
    pub drifted: bool,
    pub frozen: bool,
    pub body: crate::text::Text, // keeps non-UTF-8 bytes, as idea summaries do
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TicketList {
    pub tickets: Vec<TicketView>,
    pub unreadable: Vec<Problem>,
}

impl Workspace {
    /// Every plan change's tickets and the inbox's, sorted by id, and the files
    /// that couldn't be read, as `tickets status` reports them.
    pub fn tickets_list(&self) -> Result<Outcome<TicketList>, Error>;
}
```

Whether the CLI also gains `daiku tickets list --json` is decided in N-0013.

## A Herdr mod

[Herdr](https://herdr.dev/app)'s desktop app runs JavaScript mods that can run
commands and watch folders. A daiku mod could read `daiku … --json` and use
`watchFolder` for live updates, sharing the TypeScript types generated from
`docs/json-schema/`. It would be a separate client from `daiku-app`, and it isn't
designed here.

## Later phases

Choices here that later phases rely on:

- Writes (phase 2) go through core, never straight to files from the frontend.
- Terminals (phase 3) use xterm.js with a PTY, or herdr-core.
- Whiteboard files (phase 4) live in the workspace, so agents can read them.

Open questions, left for those phases:

- herdr-core or a PTY the app owns, behind an adapter.
- The whiteboard file format (Excalidraw or tldraw JSON) and its folder.
- Whether starting an agent from a ticket creates a git worktree per ticket, and
  how that works across repos in `REPOS.json`.
- How early to test WebKitGTK on Linux for xterm.js and canvas performance.
- Whether core should write files atomically, so a watcher never sees half a file.
