# daiku — agent guide

This file applies to agents working on the daiku repo itself. daiku replaces
zumen (planning), tomo (building) and their shared core, kiso, with one tool.

## Non-negotiable rules

- Go >= 1.22, standard library only — no third-party runtime or build deps.
  `go.mod` has no `require` block. `go test ./...` must stay green from a clean
  checkout.
- `internal/core` holds no daiku command wiring. Its packages take the tool name
  and their configuration from the caller, as kiso's did; `repos` still ships
  its reusable `repo` and `env` handlers. The one daiku-specific
  layout it owns is `state.PlanLoop` and `state.BuildLoop`, so the folder rules
  live in one place.
- `state` must not import `ticket`, directly or through another package (a test
  enforces this). Pass `ticket.IDRe` to `state.BuildLoop` instead.
- daiku's `STATE.json` starts at schema 1 with no migrations. zumen and tomo
  workspaces are refused, never converted.

## Layout

```
embed.go              package daiku: embeds scaffold/ (RootFS, PlanFS, AgentsFS)
scaffold/
  root/               what `daiku init` writes: CLAUDE.md, README.md, ideas/, references/
  plan/               the plan room, written under plan/ on first `change new`
  agents/             AGENTS.md fragments: root.md plus one per room
cmd/daiku/            the daiku binary: version, init, update, check, then internal/cli
internal/
  cli/          every command; verbs act on the active change's loop
  state/        the store: plan and build loops, zumen's wrappers
  settings/     daiku's settings (defaults under plan/)
  icm/          scaffold engine config, seeds, AGENTS.md writing, EnsurePlan
  agents/       AGENTS.md assembly from fragments, and the section merge
  plan/         the plan loop's artifact checks
    layout/ adr/ source/ overview/ idea/ reference/
  core/
    state/        phase machines, one STATE.json store with a loop per change
    ticket/       ticket schema v1, planned/native/pulled (X-) ids
    scaffold/     embedded-tree scaffold engine: root plus rooms in one manifest
    repos/        logical repo registry + local paths
    frontmatter/  controlled YAML-subset parser/writer
    placeholder/  template placeholder detection in markdown, outside code
    settings/     defaults ← global ← project merge
    workspace/    the local .daiku/ directory, active pointer, legacy and symlink checks
    changelog/    append-only log writer
    version/      version reporting (build info)
testdata/             overview fixture and golden files
```

Plan-only paths come from `internal/plan/layout`; no package hard-codes
`plan/specs` or a room folder. Every workspace write goes through
`workspace.WriteFile`, `OpenFile` or `MkdirAll` (or, for scaffold files,
`workspace.ThroughSymlink`), so a committed symlink can't send a file outside the
workspace. Path settings must be local (`filepath.IsLocal`).

## Development

- `go test ./...` — run all tests.
- `go vet ./...` — run static checks.
- `go test ./internal/icm -update` — regenerate the scaffold golden after
  editing `scaffold/`; `go test ./internal/cli -run TestOverviewGolden
  -update-overview` for the overview golden.
- In a checkout under a `go.work` that doesn't list daiku, prefix commands
  with `GOWORK=off`.
- CI runs both on Go 1.22 and 1.24 (`.github/workflows/test.yml`).
- Conventional commit messages.
