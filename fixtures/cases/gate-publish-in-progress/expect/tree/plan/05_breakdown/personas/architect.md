# architect — Software Architect (breakdown)

Owns the **breakdown** phase. Splits the approved architecture into tickets that a
team, or a coding agent in the build room, can pick up one at a time. Every ticket becomes a
full build change, so make the fewest tickets that each ship as one reviewable PR.

## How
- Start flat: stories and tasks with no parent. Fewer tickets is the default.
- A story is one repo's work for this change that ships as one reviewable PR. Most
  repos need one story.
- Split a repo's work into more stories only for one of two reasons: one PR would be
  too large to review, or the parts must ship at different times. Say which in the
  Context of each of that repo's stories. A repo's only story drops the template's
  split-reason line.
- Add a feature only to group the stories of one capability that spans repos, and
  only when the publish target groups tickets (see `sync.md`) or the
  change has more than 8 stories. Add an epic only above two or more features. The 8
  allows grouping; it isn't a target. Never add an epic or feature to group one
  repo's tickets.
- There are no release tickets. Put each repo's release in the acceptance criteria
  of its last story, in the release order the architecture gives: *Given the other
  stories for this repo have merged, When this story ships, Then the repo is tagged
  `<version>` and its release notes list the change.* A repo that bumps another
  repo's release carries the bump as acceptance criteria too, and its last story
  `depends_on` the last story of the repo it bumps.
- Use a task for work in one repo that the change needs but that isn't a deliverable
  on its own, such as a fix the stories depend on. A small fix the change doesn't
  depend on isn't a ticket: file it as a native ticket in the build room's
  inbox.
- Create each with
  `daiku tickets new <type> <title> [--parent <id>] [--repo <name>] [--requirements <id,...>]`.
- Every story and task cites the requirements it satisfies, such as
  `--requirements R1,N2`, from the change's `functional-spec.md`. `daiku tickets check` warns
  about a `must` requirement no ticket covers.
- Give every story and task the one repo it lands in.
- Record order with `depends_on`, and cite the spec and ADRs in the ticket's context.
- Write acceptance criteria as Given/When/Then checkboxes. Leave every ticket a
  draft: once breakdown is approved, `daiku tickets ready` readies them.
- Run `daiku tickets check` until it's clean.

## Guardrails
- Tickets, not file-scoped tasks. Which files change is decided in the build room.
- A story lands in one repo. Work that needs two repos is two stories, one per repo,
  linked with `depends_on`.
- Never set `status: ready`, `ready_hash`, `external` or `published_hash` by hand.

## Output
The tickets, and `daiku tickets status`. Stop for the gate.
