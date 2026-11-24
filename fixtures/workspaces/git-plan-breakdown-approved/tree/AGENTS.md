# <project> — daiku workspace

Layer 1 root map (Interpretable Context Methodology). Every agent reads this
file first, then navigates by the routing below. `CLAUDE.md` imports this
file; it is harness-agnostic and the single source of truth.

Edits inside daiku's `##` sections are overwritten by `daiku update`. Keep your own
notes in `## Tech stack & standards`, `## Model routing`, or in sections of your own.

## How to start

This workspace has a room for each kind of work. A room appears the first time
it's used, and its sections are added to this file then, so a room you haven't
used yet has no sections here.

- **Think an idea through** — `daiku idea new "Title"`, then work it with the
  facilitator (`ideas/personas/facilitator.md`). When it's ready,
  `daiku idea promote <id> --title "Title" --repo <name>` turns it into a plan
  change, with the idea's summary as its intake.
- **Plan an initiative** — `daiku change new "Title"` (add `--repo <name>` for
  each repo in scope) creates the `plan/` room and a plan change: research,
  spec, architecture and tickets, ending in tickets published to a PM tool.
- **Build a ticket** — `daiku build new <ticket-id>` builds one ticket in one
  repo, in the `build/` room: requirements, design, tasks, code, review and a
  pull request. The ticket can be planned, created here, or pulled from a PM tool.

## Commands (the `daiku` CLI)

| Command | Purpose |
|---|---|
| `daiku init [dir] [--no-git] [--force]` | scaffold a daiku workspace, root only (`--no-git`: a local workspace, vcs: none; `--force`: overwrite existing files) |
| `daiku update` | refresh managed files to the installed version |
| `daiku status [<plan-change>]` | show the active change and every open change by room and repo, or a plan's tickets and their builds |
| `daiku artifacts <change>` | list a change's files by phase, for a plan or a build |
| `daiku settings` | show/merge settings |
| `daiku switch [--all] [<change>]` | list changes (`--all` includes closed ones), or switch the active change |
| `daiku change close <change> --reason <r>` | close a change |
| `daiku gate <phase> <status> [--reason <r>] [--force]` | advance the active change's gate (earlier phases must be approved or skipped); `skipped` needs a reason |
| `daiku reopen <phase> --reason <r> [--force]` | move back to a phase and clear later approvals; for a build, `--force` overrides the repo lock |
| `daiku idea new <title>` | create an idea in `ideas/`, from the summary template |
| `daiku idea list` | list ideas with their status, and the reason for a parked or dropped one |
| `daiku idea show <id>` | print an idea's summary and status |
| `daiku idea park <id> --reason <r>` | set an open idea aside, with a reason |
| `daiku idea drop <id> --reason <r>` | drop an open idea, with a reason; its folder stays |
| `daiku idea reopen <id> --reason <r>` | reopen a parked or dropped idea, with a reason |
| `daiku idea promote <id> --title <t> [--repo <name\|path>]... [--rev <r>]` | start a plan change from an idea: its intake is the idea's summary, and an idea can be promoted more than once |
| `daiku references check` | warn about unindexed files in `references/` and index entries that lack a file or a field |
| `daiku repo add <name> <remote> [--path <p>] [--branch <b>]` | register a repo this workspace works with |
| `daiku repo list` | list repos and their local paths |
| `daiku repo path <name> <path>` | set a repo's local path |
| `daiku repo archive <name> [--reason <r>]` | mark a repo archived: kept for its history, but not cloned and given no new builds |
| `daiku repo unarchive <name>` | make an archived repo live again |
| `daiku repo doctor [--clone] [--into <dir>]` | check repos; `--clone` fetches missing ones |
| `daiku env` | print harness flags (`--add-dir <path>`) for the active change's repos |
| `daiku check` | validate the embedded scaffold |
| `daiku version` | print the version |
| `daiku help [<command> [<verb>]]` | usage; `daiku <command> --help` (or `-h`) prints one command's usage |

`status`, `switch`, `gate`, `reopen`, `change close` and `env` act on the active
change, whichever room it's in. The folders/personas describe *what* to do;
`daiku` is a plain CLI — run it like `git` or `gh`. Prefer `daiku <command>` over
hand-editing `STATE.json` when the binary is on your PATH.

## Routing

| Task / Intent | Directory | Persona | Model | Read | Skip |
|---|---|---|---|---|---|
| Work an idea | `ideas/` | facilitator | `deep` | `ideas/context.md`, `ideas/<n>-<slug>/*`, `references/index.md` | `plan/specs/*`, the repos |

Each room adds its own routing below once it exists.

## Version control

`vcs` in `.daiku/settings.json` says whether this workspace lives in git: `git`
(the default) or `none`. Under `vcs: git`, daiku commits workspace state as
described below. Under `vcs: none` the workspace is plain local files: daiku
writes the same files but never runs git. It keeps `.gitignore`, so
`.daiku/active` and `.daiku/paths.json` stay out of any enclosing or later git
repo, but writes no `.gitattributes` or `.gitkeep`.

## Workflow rules

- Each phase in order: a phase can't be gated until the earlier ones are approved,
  skipped or complete.
- Each phase stops at a gate for human approval — never proceed past a gate on
  your own. Record transitions with `daiku gate <phase> <status>`. `--force`
  overrides the order check and is logged; only use it when a human says so.
- Run each task on its routing row's model tier, mapped in `## Model routing`.
  For a `fast` or `standard` task, when the session isn't on that tier's model,
  hand the task's work to a subagent on that model, and give it the row's
  persona and its `Read` and `Skip` lists. The subagent does the work and
  returns; a question it can't settle, or a step that needs the human's
  approval, comes back to the session, which asks the human. A subagent never
  runs `daiku gate`, `daiku reopen`, `daiku switch`, `daiku change close`,
  `daiku commit` or `daiku pr`: it proposes, and the session runs them once the
  human approves. A `deep` task runs in the session: if the session isn't on
  the `deep` model, say so before starting. If a tier has no model mapped, or
  the harness can't set a subagent's model, do the work in the session and say
  so.
- Read only the files the routing names for your task; skip the rest. Any
  phase may also open a file listed in `references/index.md`: reference material such
  as notes, designs and exported chats. `references/context.md` has the index format.
- Skip a phase with `daiku gate <phase> skipped --reason <r>`. Use `daiku reopen
  <phase> --reason <r>` to go back: it clears the approvals after that phase,
  because they no longer hold.
- `gate`, `reopen` and `change close` write `STATE.json` and `LOG.md`. Under
  `vcs: git` they also commit them for you, leaving anything else you have staged
  alone, and `gate` commits the change's folders, so write the artifact before
  you gate it. They don't push.
- Cite `file:line` when grounding design or findings in a real codebase.

## Tech stack & standards

- <FILL IN: the target systems' stacks, or "greenfield — to be decided in architecture">

## Model routing

Each routing row names a model tier. Map each tier to a model your harness
offers; this section is yours, and `daiku update` keeps it. The workflow rules
hand `fast` and `standard` tasks to a subagent on their tier's model, and run
`deep` tasks in the session. So put the session on the `standard` model, and
switch it to the `deep` one for `deep` tasks. Reading this file
doesn't switch the current session's model.

| Tier | Use for | Model |
|---|---|---|
| `deep` | judgment: ideas, synthesis, specs, architecture, breakdown, design, review, security | <FILL IN> |
| `standard` | most work: research, exploring repos, settling tickets, requirements, code | <FILL IN> |
| `fast` | mechanical steps: intake, publishing, running tests, shipping | <FILL IN> |

For example, in Claude Code: `opus`, `sonnet`, `haiku`.

## Directory map

```
├── CLAUDE.md
├── AGENTS.md
├── README.md
├── sync.md               # how an agent talks to your PM tool: publish and fetch
├── sync-examples/        # starters for sync.md
├── .gitignore            # keeps .daiku/active and paths.json out of git
├── .gitattributes        # vcs: git only
├── STATE.json            # every change's loop, phase, gates, sources (committed under vcs: git)
├── LOG.md                # gate history (committed under vcs: git)
├── REPOS.json            # logical repos and remotes (committed under vcs: git)
├── .daiku/               # settings + manifest (committed under vcs: git), active change + repo paths (local)
├── references/           # reference files + your own index.md (daiku never edits it)
├── ideas/                # ideas, one <n>-<slug>/ folder each (summary.md, log.md)
├── plan/                 # the plan room, once `daiku change new` has run
└── build/                # the build room, once `daiku build new` has run
```

## Plan loop

A plan change is an initiative, often across many repos, and ends in tickets. It
flows through eight phases, each owned by a persona. Research, explore and
synthesis are skippable, with a reason. `daiku change new` skips explore when
there's no repo, and synthesis when there are fewer than two. `daiku status`
warns if a repo's explore is skipped. `daiku status <plan-change>` lists the
plan's tickets and how far each one's build has got, as the overview's Builds
section does.

`intake → [research] → [explore] → [synthesis] → spec → architecture → breakdown → publish`

- **intake** — the goal, constraints, stakeholders and repos in scope: a prose
  issue/idea, or a pointer to an existing codebase.
- **research** (researcher) — external investigation: docs, vendors, prior art,
  subject-matter experts.
- **explore** (explorer) — one findings report per repo (system map, entry points,
  module responsibilities, data flow, cited `file:line`, debt/risk observations).
  Observation only; no prescription.
- **synthesis** (architect) — what the findings add up to across repos: common
  patterns, divergences, constraints.
- **spec** (analyst) — unambiguous, verifiable system-level requirements with
  Given/When/Then acceptance criteria.
- **architecture** (architect) — the smallest solution: decisions with rationale and
  rejected alternatives, component map, interfaces, data flow. Record durable
  decisions as ADRs under `plan/04_architecture/adr/`.
- **breakdown** (architect) — stories and tasks as ticket files, each one repo's work
  in one reviewable PR; features and epics only where grouping helps.
- **publish** (publisher) — sync tickets to a PM tool, then freeze them. A run phase.

Each phase writes its artifact and stops at a gate for human approval.

To start one:

1. **Create a change** — `daiku change new "Title"` (add `--repo <name>` for
   each registered repo, or `--repo <path>` for a single unregistered one). This
   writes the intake (`plan/00_intake/<id>.md`), makes a `plan/specs/<n>-<slug>/`
   folder, seeds `STATE.json`, and switches to it.
2. **Kick off** with a prompt like: *"Start the loop on `plan/00_intake/1.md`."*

Under `vcs: git`, a plan gate also commits the change's folders under
`plan/01_research/` and `plan/02_explore/`. Commit ADRs yourself; they aren't per
change.

## Plan commands

| Command | Purpose |
|---|---|
| `daiku source new --title <t> [--repo <path>] [--rev <r>] [--body <b>]` | add an intake source to `plan/00_intake/` |
| `daiku source list` | list intake sources |
| `daiku source show <id>` | show an intake source |
| `daiku adr new --title <t> [--deciders <d>] [--context <c>] [--decision <d>] [--consequences <c>] [--file <path>]` | create an ADR in `plan/04_architecture/adr/` (`--file` reads the body from a file) |
| `daiku adr list` | list ADRs |
| `daiku adr show <number>` | show an ADR |
| `daiku change new <title> [--repo <name\|path>]... [--rev <r>] [--body <b>]` | create a plan change + intake + switch; creates `plan/` the first time |
| `daiku tickets new <type> <title> [--parent <id>] [--repo <name>] [--depends <id,id>] [--requirements <id,id>] [--inbox]` | create a ticket in the active change, citing the spec's requirements; with `--inbox`, or no open plan change, an `N-` task in `build/tickets/` |
| `daiku tickets check` | validate every ticket: schema, parents, cycles, repos, readiness, drift. Errors (`✗`) exit 1; warnings (`!`) don't |
| `daiku tickets status` | list tickets by draft, ready, published and drifted |
| `daiku tickets order` | list tickets in publish order |
| `daiku tickets ready <id>... \| --all [--force]` | ready draft tickets once breakdown is approved, all or none |
| `daiku tickets mark-published <id> --external <tool>=<item-id> [--force]` | record a PM tool's id and freeze the ticket; needs publish in-progress (`--force` overrides only that), a valid ready mark, and published predecessors |
| `daiku tickets unfreeze <id> --reason <r>` | clear a ticket's freeze, so the next sync updates it |
| `daiku overview [<change>] [--check] [--stdout]` | write or check `plan/specs/<change>/README.md`: the narrative region is yours, the generated region is rewritten; `--check` exits 1 if it's out of date |

Prefer `daiku <command>` over hand-editing `plan/00_intake/`.

## Plan routing

| Task / Intent | Directory | Persona | Model | Read | Skip |
|---|---|---|---|---|---|
| Define the base source | `plan/00_intake/` | — | `fast` | `plan/00_intake/context.md`, `references/index.md` | `plan/specs/*` |
| External research | `plan/01_research/` | researcher | `standard` | `plan/01_research/context.md`, `plan/00_intake/<id>.md`, `references/index.md` | `plan/specs/*`, the repos |
| Explore the repos | `plan/02_explore/` | explorer | `standard` | `plan/02_explore/context.md`, the repos (`daiku repo list`), `references/index.md` | `plan/specs/*` |
| Synthesize findings | `plan/02_explore/` | architect | `deep` | `plan/02_explore/context.md`, `plan/02_explore/<n>-<slug>/*`, `references/index.md` | `plan/specs/*` |
| Write the spec | `plan/03_spec/` | analyst | `deep` | `plan/03_spec/context.md`, `plan/00_intake/*`, `plan/01_research/<n>-<slug>/*`, `plan/02_explore/<n>-<slug>/*`, `references/index.md` | the repos |
| Design the architecture | `plan/04_architecture/` | architect | `deep` | `plan/04_architecture/context.md`, `plan/specs/<n>-<slug>/functional-spec.md`, `plan/02_explore/<n>-<slug>/*`, `references/index.md` | the repos |
| Break down into tickets | `plan/05_breakdown/` | architect | `deep` | `plan/05_breakdown/context.md`, `plan/specs/<n>-<slug>/functional-spec.md`, `plan/specs/<n>-<slug>/architecture.md`, `references/index.md` | the repos |
| Publish tickets | `plan/06_publish/` | publisher | `fast` | `plan/06_publish/context.md`, `sync.md`, `plan/specs/<n>-<slug>/tickets/*`, `references/index.md` | the repos, `plan/01_research/*`, `plan/02_explore/*` |

## Plan naming

- Specs: `plan/specs/<n>-<slug>/functional-spec.md|architecture.md`
- Tickets: `plan/specs/<n>-<slug>/tickets/<ID>.md`, with ids like `S-0014` from one
  workspace-wide sequence
- Intake: `plan/00_intake/<id>.md`
- Ideas: `ideas/<n>-<slug>/summary.md|log.md`
- Research: `plan/01_research/<n>-<slug>/<topic>.md`
- Findings: `plan/02_explore/<n>-<slug>/<repo>.md`, `plan/02_explore/<n>-<slug>/synthesis.md`
- ADRs: `plan/04_architecture/adr/NNNN-<slug>.md`
- Sync instructions: `sync.md` at the workspace root, with starters in `sync-examples/`
- Personas: `plan/<stage>/personas/<name>.md`
- Templates: `plan/<stage>/templates/<artifact>.md`

## Plan directory map

```
plan/
├── 00_intake/            # the base source (<id>.md)
├── 01_research/          # researcher (skippable)
├── 02_explore/           # explorer + synthesis (skippable)
├── 03_spec/              # analyst
├── 04_architecture/      # architect + adr/
├── 05_breakdown/         # architect
├── 06_publish/           # publisher
└── specs/                # per-change artifacts and tickets (<n>-<slug>/)
```

## Plan handoff

Tickets are the only interface out of the plan room. They're local markdown until
published; then the PM tool owns them, and the file is a frozen snapshot. A coding
agent picks one up from there, in the build room or any other harness.
