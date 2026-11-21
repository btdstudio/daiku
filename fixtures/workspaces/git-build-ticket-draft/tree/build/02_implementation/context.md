# build/02_implementation — Software Engineer room

Owns the **implement** phase.

## Persona
- `personas/coder.md` — implements approved tasks, one at a time, with tests

## Input (read)
- `build/<repo>/changes/<ticket-id>/tasks.json` — the approved tasks
- `build/<repo>/changes/<ticket-id>/design.md` — the approved design
- relevant files under the build's repo (see `daiku repo list`)

## Output (write)
- code under the build's repo (see `daiku repo list`), touching exactly the files each task names
- tests alongside the change

## Gate
Report task-by-task results (files changed, tests run). Do not commit or push;
version control is handled by a later review/commit step.
