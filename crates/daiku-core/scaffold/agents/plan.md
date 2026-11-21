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
