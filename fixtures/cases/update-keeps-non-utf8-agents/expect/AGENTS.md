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
| `daiku status [<plan-change>]` | show the active change and every open change by room and repo, or a plan's tickets and their builds |
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

- Notes by hand, caf� �.

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
