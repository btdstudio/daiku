# plan/02_explore — Explorer and synthesis room

Owns the **explore** phase (one findings report per repo) and the **synthesis**
phase (what those reports add up to).

`daiku change new` skips explore when there's no repo, and synthesis when there are
fewer than two. Bring either back with `daiku reopen <phase> --reason <r>`.

## Personas
- `personas/explorer.md` — reads one repo and writes its findings
- `../04_architecture/personas/architect.md` — writes the synthesis

## Input (read)
- `plan/00_intake/<id>.md` — the repos in scope
- each repo, reached through `daiku env`

## Output (write)
- `plan/02_explore/<n>-<slug>/<repo>.md` — from `templates/findings.md`
- `plan/02_explore/<n>-<slug>/synthesis.md` — from `templates/synthesis.md`

## Gate
Stop after each phase's output and hand control back for approval.
