# plan/00_intake — Base source

Holds the base source for each change: the goal, constraints, stakeholders and
repos in scope. It can be a prose issue/idea, or a pointer to an existing codebase
to plan against.

## Files
- `<id>.md` — one per source (created by `daiku source new`)

## Front matter
```yaml
---
source: issue          # or: repo
repo: <path/url>       # only when source: repo
rev: <branch/sha>      # optional pin
---
```

## When `source: repo`
The loop runs `explore` before `spec`. `daiku change new --repo` sets this up. If a
change started without a repo, bring explore back with
`daiku reopen explore --reason <r>`.

## Gate
Stop after defining the source and hand control back for approval.
