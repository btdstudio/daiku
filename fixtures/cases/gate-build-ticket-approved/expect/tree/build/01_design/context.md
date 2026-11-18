# build/01_design — Product Manager + Architect room

Owns the **ticket → requirements → design → tasks** phases of a build.

## Personas
- `personas/analyst.md` — ticket and requirements (what + how we know it's done)
- `personas/architect.md` — design (smallest solution) + tasks (`tasks.json`)

## Input (read)
- the build's ticket: `daiku status` shows it, in `build/tickets/<id>.md` or, for a
  planned ticket, `plan/specs/<n>-<slug>/tickets/<id>.md`
- `templates/requirements.md`, `templates/design.md` — skeletons

## Output (write)
- `build/<repo>/changes/<ticket-id>/requirements.md`
- `build/<repo>/changes/<ticket-id>/design.md`
- `build/<repo>/changes/<ticket-id>/tasks.json`

## Gate
Stop after each artifact and hand control back for approval. Never start the
next phase until the current one is approved (see `daiku status`).
