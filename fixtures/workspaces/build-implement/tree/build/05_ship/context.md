# build/05_ship — Ship room

Owns the **ship** phase: a branch, micro-commits and a pull request.

## Input (read)
- `build/<repo>/changes/<ticket-id>/tasks.json` — what was built
- the build's ticket (`daiku status`)
- `build/<repo>/changes/<ticket-id>/test-results.md`, `code-review.md` and
  `security-review.md` — what to report in the pull request

## Output (write)
- a branch named as `daiku status` suggests
- micro-commits in the build's repo, made with `daiku commit`
- a pull request, opened with `daiku pr`

## The pull request template

`daiku pr` starts the body from the first template it finds:

1. the build repo's own `pull_request_template.md` (any case), in `.github/`, then
   the repo root, then `docs/`
2. this workspace's `build/05_ship/templates/pull-request.md`
3. daiku's built-in copy, used when that file is missing

A repo template is used as written. `{{links}}` marks where the ticket's links go;
a template without it gets the links appended.

## Fill the template

1. `daiku pr --template > <scratch file>` prints the chosen template with the links
   in place, and names the template it used on stderr.
2. Fill each section from `tasks.json` and the test, review and security reports.
   Keep the `<!-- daiku:links -->` block as it is.
3. `daiku pr --body-file <scratch file>` opens the pull request with that text. The
   links block is refreshed, or added if it's gone.

## Customise

Edit `build/05_ship/templates/pull-request.md` to change the template for every
repo in this workspace; `daiku update` keeps your edits. A repo's own template wins
over it.

## Gate
Ship is a run phase: mark it `in-progress` when you start and `complete` once the
pull request is open. Do not merge it.
