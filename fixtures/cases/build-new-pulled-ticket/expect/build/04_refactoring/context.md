# build/04_refactoring — Tech Lead / Code Review + Security room

Owns the **review** and **security** passes after implementation.

## Personas
- `personas/reviewer.md` — correctness, scope vs design, test adequacy
- `personas/security.md` — vulnerability classes and security debt

## Input (read)
- the build repo's code (the change and its seams; see `daiku repo list`)
- `build/<repo>/changes/<ticket-id>/*` (the design the code claims to implement)

## Output (write)
- `build/<repo>/changes/<ticket-id>/code-review.md` — numbered findings
- `build/<repo>/changes/<ticket-id>/security-review.md` — vulnerability findings

## Gate
Only **critical** / **high** findings loop back to the coder, with `daiku reopen
implement --reason <r>`. Skip security with `daiku gate security skipped --reason
<r>` when the change has no security surface. Read-only by default — report, don't
fix.
