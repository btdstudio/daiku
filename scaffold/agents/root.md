# <project> — daiku workspace

Layer 1 root map (Interpretable Context Methodology). Every agent reads this
file first, then navigates by the routing below. `CLAUDE.md` just points
here; this file is harness-agnostic and the single source of truth.

Edits inside daiku's `##` sections are overwritten by `daiku update`. Keep your own
notes in `## Tech stack & standards` or in sections of your own.

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
| `daiku status` | show the active change's phase/status |
| `daiku settings` | show/merge settings |
| `daiku switch [--all] [<change>]` | list changes (`--all` includes closed ones), or switch the active change |
| `daiku change close <change> --reason <r>` | close a change |
| `daiku gate <phase> <status> [--reason <r>] [--force]` | advance the active change's gate (earlier phases must be approved or skipped); `skipped` needs a reason |
| `daiku reopen <phase> --reason <r>` | move back to a phase and clear later approvals |
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

| Task / Intent | Directory | Persona | Read | Skip |
|---|---|---|---|---|
| Work an idea | `ideas/` | facilitator | `ideas/context.md`, `ideas/<n>-<slug>/*`, `references/index.md` | `plan/specs/*`, the repos |

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
