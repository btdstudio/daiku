# <project> — daiku workspace

This workspace follows the Interpretable Context Methodology (ICM): software work
moves through gated phases, each owned by a persona, with a human approving every
gate. Start with [AGENTS.md](./AGENTS.md).

Planning and building each have a room, created the first time it's used:

- `ideas/` — rough ideas, explored in conversation before they become a change
- `references/` — material the workspace keeps for reference
- `plan/` — planning: research, spec, architecture and tickets, from
  `daiku change new`. The loop is `intake → [research] → [explore] →
  [synthesis] → spec → architecture → breakdown → publish`.
- `build/` — building one ticket at a time, from `daiku build new`

`STATE.json` and `LOG.md` hold every change's progress, whichever room it's in.
Driven by the `daiku` CLI.
