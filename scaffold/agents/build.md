## Build loop

A build change takes one ticket into one repo and ends in a pull request. It's
named by the ticket's id, and flows through nine phases, each owned by a persona.
Implement and ship are run phases (`in-progress`, then `complete`); the rest are
gated, and security can be skipped with a reason.

`ticket → requirements → design → tasks → implement → test → review → security → ship`

- **ticket** (analyst) — settle the ticket: what's asked and why. For a planned or
  pulled ticket with `external`, read the live item through `sync.md` (**fetch**).
- **requirements** (analyst) — turn the ticket into an unambiguous spec.
- **design** (architect) — the smallest solution that satisfies the requirements.
- **tasks** (architect) — break the design into dependency-ordered tasks.
- **implement** (coder) — execute tasks, one at a time, with tests.
- **test** (qa) — report pass/fail per acceptance criterion.
- **review** (reviewer) — correctness, scope and test adequacy. Read-only.
- **security** (security) — vulnerabilities and security debt. Read-only. Skippable,
  with a reason.
- **ship** — a branch named as `daiku status` suggests, micro-commits, and a pull
  request linking the ticket, its external ids, and its spec and ADRs.

A test failure, or a critical or high finding, loops back with
`daiku reopen implement --reason <r>`. That clears test and review, which then run
again against the fixed code.

**One build per repo in implement.** Gating a build into implement fails while
another open build on the same repo is in implement, test, review, security or
ship. Closing that build, finishing its ship, or reopening it to tasks frees the
repo; `--force` overrides and is logged.

## Build routing

| Task / Intent | Directory | Persona | Read | Skip |
|---|---|---|---|---|
| Settle the ticket | `build/01_design/` | analyst | the build's ticket (`daiku status`), `sync.md` for a ticket with `external` | the repo, `build/02_implementation/*` |
| Write requirements | `build/01_design/` | analyst | `build/01_design/context.md`, the build's ticket | the repo, `build/03_testing/*` |
| Design solution | `build/01_design/` | architect | `build/01_design/context.md`, `build/<repo>/changes/<ticket-id>/requirements.md` | — |
| Break down into tasks | `build/01_design/` | architect | `build/<repo>/changes/<ticket-id>/design.md` | — |
| Implement code | `build/02_implementation/` | coder | `build/02_implementation/context.md`, `build/<repo>/changes/<ticket-id>/*`, the repo (`daiku repo list`) and its `AGENTS.md` | `build/04_refactoring/*` |
| Test | `build/03_testing/` | qa | `build/03_testing/context.md`, the repo and its `AGENTS.md`, `build/<repo>/changes/<ticket-id>/requirements.md` | `build/tickets/*` |
| Review | `build/04_refactoring/` | reviewer | `build/04_refactoring/context.md`, the repo and its `AGENTS.md`, `build/<repo>/changes/<ticket-id>/*` | `build/tickets/*` |
| Security pass | `build/04_refactoring/` | security | `build/04_refactoring/context.md`, the repo and its `AGENTS.md` | `build/tickets/*` |
| Ship | the repo | — | `build/<repo>/changes/<ticket-id>/tasks.json`, the build's ticket | `build/02_implementation/*` |

## Build naming

- Build changes: `build/<repo>/changes/<ticket-id>/requirements.md|design.md|tasks.json|test-results.md|code-review.md|security-review.md`
- The inbox: `build/tickets/<ID>.md` — `N-0001` when created here, `X-<pm-id>` when
  pulled from a PM tool. Planned tickets stay in `plan/specs/<n>-<slug>/tickets/`
  and are read in place.
- Personas: `build/<stage>/personas/<name>.md`
- Templates: `build/01_design/templates/<artifact>.md`

## Build directory map

```
build/
├── tickets/              # the inbox: N- and X- tickets
├── 01_design/            # analyst + architect
├── 02_implementation/    # coder
├── 03_testing/           # qa
├── 04_refactoring/       # reviewer + security
└── <repo>/changes/       # one folder per build, named by its ticket id
```
