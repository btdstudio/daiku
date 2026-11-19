# build/03_testing — QA room

Owns the **test** phase after implementation. (A dedicated QA pass: the coder
writes tests with the change, and the reviewer judges their adequacy.)

## Persona
- `personas/qa.md` — writes/runs tests and checks them against requirements

## Input (read)
- the build repo's code (the code under test; see `daiku repo list`)
- `build/<repo>/changes/<ticket-id>/requirements.md` (acceptance criteria)

## Output (write)
- tests alongside the app in the build repo (see `daiku repo list`)
- `build/<repo>/changes/<ticket-id>/test-results.md` — run results & coverage

## Gate
Report pass/fail per acceptance criterion, then stop for approval. Failures loop
back to the coder with `daiku reopen implement --reason <r>`.
