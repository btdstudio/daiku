# plan/05_breakdown — Architect room (breakdown)

Owns the **breakdown** phase: turning the architecture into tickets.

## Persona
- `personas/architect.md` — stories and tasks; features and epics only where grouping helps

## Input (read)
- `plan/specs/<n>-<slug>/functional-spec.md` and `architecture.md` — approved
- `plan/04_architecture/adr/` — decisions tickets should cite
- `templates/ticket.md` — the ticket format

## Output (write)
- `plan/specs/<n>-<slug>/tickets/<ID>.md` — created with `daiku tickets new`

## Gate
If the breakdown changed the solution's shape, refresh the narrative in
`plan/specs/<n>-<slug>/README.md` first. Run `daiku tickets check`, then stop and hand
control back for approval, with the tickets still drafts. A `!` warning
(`ready, but breakdown for … is "…"`) means breakdown was reopened after those
tickets were readied. Revise them as needed. Once breakdown is approved again,
each one is re-readied by id with `daiku tickets ready <id>`, because `--all`
only readies drafts.
