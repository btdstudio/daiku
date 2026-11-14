# daiku (大工) 🪚 — plan and build in one workspace

daiku scaffolds and maintains an Interpretable Context Methodology (ICM)
workspace where software work moves through gated phases, each owned by a
persona, with a human approving every gate. A workspace has a room for each
kind of work, created the first time it's used:

- **plan** — initiatives that take weeks, often span many repos, and need
  research and architecture before anyone writes code. They end in tickets.
- **build** — one ticket in one repo, from requirements to a pull request
  (coming in a later version).

The filesystem is the architecture; no framework or runtime is required after
scaffold. daiku replaces [zumen](https://github.com/btdstudio/zumen) (planning),
[tomo](https://github.com/btdstudio/tomo) (building) and their shared core,
[kiso](https://github.com/btdstudio/kiso). It doesn't read their workspaces.

## Attribution

The Interpretable Context Methodology (ICM) that daiku implements is inspired
by:

- **Jake Van Clief**, **David McDermott** — ["Interpretable Context Methodology:
  Folder Structure as Agentic Architecture"](https://arxiv.org/abs/2603.16021)

## Install

```sh
go install github.com/btdstudio/daiku/cmd/daiku@latest
```

## Getting started

```sh
daiku init my-workspace        # root only: no room yet
cd my-workspace
daiku change new "Gateway"     # creates the plan room and a plan change
```

Or build straight away, without a plan:

```sh
daiku repo add api https://github.com/org/api --path ~/src/api
daiku tickets new task "Fix login" --repo api --inbox   # build/tickets/N-0001.md
daiku build new N-0001         # creates the build room and a build change
```

`AGENTS.md` names every way in: `daiku idea new`, `daiku change new` and
`daiku build new`. It gains a room's sections once that room exists.

## Commands

| Command | Purpose |
|---|---|
| `daiku init [dir] [--no-git] [--force]` | scaffold a daiku workspace, root only (`--no-git`: a local workspace, vcs: none; `--force`: overwrite existing files) |
| `daiku update` | refresh managed files to the installed version |
| `daiku status [<plan-change>]` | show the active change and every open change by room and repo, or a plan's tickets and their builds |
| `daiku settings` | show/merge settings |
| `daiku switch [--all] [<change>]` | list changes (`--all` includes closed ones), or switch the active change |
| `daiku source new --title <t> [--repo <path>] [--rev <r>] [--body <b>]` | add an intake source to `plan/00_intake/` |
| `daiku source list` | list intake sources |
| `daiku source show <id>` | show an intake source |
| `daiku adr new --title <t> [--deciders <d>] [--context <c>] [--decision <d>] [--consequences <c>] [--file <path>]` | create an ADR in `plan/04_architecture/adr/` (`--file` reads the body from a file) |
| `daiku adr list` | list ADRs |
| `daiku adr show <number>` | show an ADR |
| `daiku build new <ticket-id>` | create a build change for a ticket, in its repo; creates `build/` the first time |
| `daiku change new <title> [--repo <name\|path>]... [--rev <r>] [--body <b>]` | create a plan change + intake + switch; creates `plan/` the first time |
| `daiku change close <change> --reason <r>` | close a change |
| `daiku gate <phase> <status> [--reason <r>] [--force]` | advance the active change's gate (earlier phases must be approved or skipped); `skipped` needs a reason |
| `daiku reopen <phase> --reason <r> [--force]` | move back to a phase and clear later approvals; for a build, `--force` overrides the repo lock |
| `daiku tickets new <type> <title> [--parent <id>] [--repo <name>] [--depends <id,id>] [--requirements <id,id>] [--inbox]` | create a ticket in the active change, citing the spec's requirements; with `--inbox`, or no open plan change, an `N-` task in `build/tickets/` |
| `daiku tickets pull <file>` | record a ticket an agent fetched from a PM tool (through `sync.md`) as `build/tickets/X-<pm-id>.md`; pulling the same item again replaces it |
| `daiku tickets check` | validate every ticket: schema, parents, cycles, repos, readiness, drift. Errors (`✗`) exit 1; warnings (`!`) don't |
| `daiku tickets status` | list tickets by draft, ready, published and drifted |
| `daiku tickets order` | list tickets in publish order |
| `daiku tickets ready <id>... \| --all [--force]` | ready draft tickets once breakdown is approved, all or none |
| `daiku tickets mark-published <id> --external <tool>=<item-id> [--force]` | record a PM tool's id and freeze the ticket; needs publish in-progress (`--force` overrides only that), a valid ready mark, and published predecessors |
| `daiku tickets unfreeze <id> --reason <r>` | clear a ticket's freeze, so the next sync updates it |
| `daiku overview [<change>] [--check] [--stdout]` | write or check `plan/specs/<change>/README.md`: the narrative region is yours, the generated region is rewritten; `--check` exits 1 if it's out of date |
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
| `daiku commit [git commit args]` | commit what's staged in the active build's repo, one reviewed group at a time |
| `daiku pr [title]` | open a pull request from the active build's repo, linking its ticket |
| `daiku check` | validate the embedded scaffold |
| `daiku version` | print the version |
| `daiku help [<command> [<verb>]]` | usage; `daiku <command> --help` (or `-h`) prints one command's usage |

`status`, `switch`, `gate`, `reopen`, `change close` and `env` act on the active
change, whichever room it's in. `commit` and `pr` act on the active build.

## The workspace

```
├── AGENTS.md          root map, assembled from the root and each room's sections
├── STATE.json         every change's loop, phase, gates, sources (committed under vcs: git)
├── LOG.md             gate history (committed under vcs: git)
├── REPOS.json         logical repos and remotes (committed under vcs: git)
├── .daiku/            settings + manifest (committed under vcs: git), active change + repo paths (local)
├── sync.md            how an agent talks to your PM tool: publish and fetch
├── sync-examples/     starters for sync.md
├── references/        reference files + your own index.md (daiku never edits it)
├── ideas/             ideas, one <n>-<slug>/ folder each (summary.md, log.md)
├── plan/              the plan room, once `daiku change new` has run
│   ├── 00_intake/ … 06_publish/
│   └── specs/         per-change artifacts and tickets (<n>-<slug>/)
└── build/             the build room, once `daiku build new` has run
    ├── tickets/       the inbox: N- and X- tickets
    ├── 01_design/ … 04_refactoring/
    └── <repo>/changes/<ticket-id>/   one folder per build
```

`daiku references check` warns about unindexed files in `references/`.

## The plan loop

`intake → [research] → [explore] → [synthesis] → spec → architecture → breakdown → publish`

Research, explore and synthesis are skippable, with a reason. Breakdown writes
stories and tasks as ticket files, each one repo's work in one reviewable PR;
features and epics only where grouping helps. Publishing is agent work:
`sync.md`, at the workspace root, tells an agent how to talk to your PM tool, and
the CLI never calls one. Each change has a one-page overview,
`plan/specs/<change>/README.md`, that `daiku overview` keeps current. Its
generated **Builds** section lists the plan's tickets with their repo and build
phase; a build's gate, reopen and close rewrite it, and commit it with the
transition, when the plan is in the workspace.

## The build loop

`ticket → requirements → design → tasks → implement → test → review → security → ship`

A build takes one ticket into one repo and ends in a pull request. `daiku build new
<ticket-id>` names the build by its ticket and reads the ticket where it is: a ready
or published ticket in `plan/specs/<change>/tickets/`, or one in the inbox,
`build/tickets/`. The inbox holds native tickets (`N-`, from `daiku tickets new …
--inbox`) and tickets pulled from a PM tool (`X-<pm-id>`, from `daiku tickets pull`
after an agent fetches the item through `sync.md`). Security is skippable, with a
reason; implement and ship are run phases.

Only one build per repo is in implement, test, review, security or ship at a time:
gating another into implement fails, naming the holder, until it's closed, its ship
is complete, or it's reopened to tasks. `--force` overrides and is logged. `daiku
commit` and `daiku pr` run in the build's repo, and the pull request links the
ticket and its external items.

## Where things stand

Bare `daiku status` shows the active change (its room, repo, phase, gates,
initiative and code path), then every open plan, then every open build grouped
by repo, with the active one marked `*` and each repo's lock holder named. A
build that has shipped but isn't closed shows as done and holds nothing.
`daiku status <plan-change>` lists that plan's tickets with their repo and
build phase, or "not started". Both only read.

## Ideas

Before a change, you can work an idea through in `ideas/`. `daiku idea new "Title"`
makes a folder with a summary and a log, a facilitator persona works it with you in a
fixed order, and `daiku idea promote <id> --title <t>` starts a plan change whose
intake is the idea's summary. An idea can be promoted more than once to split it, and
it can be parked or dropped, never deleted. It's optional, and ideas have no gates.

## Version control

The `vcs` setting in `.daiku/settings.json` (or `~/.config/daiku/settings.json`)
says whether a workspace lives in git: `git` (the default) commits state on every
transition; `none` never runs git. `daiku init --no-git` makes a `vcs: none`
workspace.

## Development

```sh
GOWORK=off go build -o daiku ./cmd/daiku   # build
GOWORK=off go test ./...                    # all tests
GOWORK=off go test ./internal/icm -update  # regenerate the scaffold golden
```

Runtime: Go >= 1.22, standard library only.
