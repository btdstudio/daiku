# plan/04_architecture — Architect room

Owns the **architecture** phase and the ADR store. Also writes the **synthesis** in
`plan/02_explore/`.

ADRs live here at the workspace level, not per change, because decisions outlive
initiatives and tickets cite them.

## Persona
- `personas/architect.md` — the smallest solution + durable decisions

## Input (read)
- `plan/specs/<n>-<slug>/functional-spec.md` — the approved spec
- `plan/02_explore/<n>-<slug>/` — findings and synthesis (if they ran)
- `templates/architecture.md` — skeleton

## Output (write)
- `plan/specs/<n>-<slug>/architecture.md`
- `adr/NNNN-<slug>.md` — architecture decision records
- `plan/specs/<n>-<slug>/README.md` — the narrative region only

## Narrative
After `architecture.md`, write the README's narrative, between
`<!-- daiku:narrative -->` and `<!-- /daiku:narrative -->`. It's about 250 words:
the problem, the shape of the solution, who's affected, and what's out of scope.
If the README doesn't exist, run `daiku overview` first. daiku rewrites the rest
of the file.

## Gate
Stop after the architecture (and any ADRs) and hand control back for approval. ADRs
sit outside the spec folder, so under `vcs: git` commit them yourself before gating.
