# daiku — agent guide

This file applies to agents working on the daiku repo itself. daiku replaces
zumen (planning), tomo (building) and their shared core, kiso, with one tool.

## Non-negotiable rules

- Go >= 1.22, standard library only — no third-party runtime or build deps.
  `go.mod` has no `require` block. `go test ./...` must stay green from a clean
  checkout.
- `internal/core` holds no command behaviour. Its packages take the tool name and
  their configuration from the caller, as kiso's did. The one daiku-specific
  layout it owns is `state.PlanLoop` and `state.BuildLoop`, so the folder rules
  live in one place.
- `state` must not import `ticket`, directly or through another package (a test
  enforces this). Pass `ticket.IDRe` to `state.BuildLoop` instead.
- daiku's `STATE.json` starts at schema 1 with no migrations. zumen and tomo
  workspaces are refused, never converted.

## Layout

```
cmd/daiku/            the daiku binary (only `version` so far)
internal/core/
  state/        phase machines, one STATE.json store with a loop per change
  ticket/       ticket schema v1, planned/native/pulled (X-) ids
  scaffold/     embedded-tree scaffold engine: root plus rooms in one manifest
  repos/        logical repo registry + local paths
  frontmatter/  controlled YAML-subset parser/writer
  placeholder/  template placeholder detection in markdown, outside code
  settings/     defaults ← global ← project merge
  workspace/    the local .daiku/ directory, active pointer, legacy check
  changelog/    append-only log writer
  version/      version reporting (build info)
```

## Development

- `go test ./...` — run all tests.
- `go vet ./...` — run static checks.
- In a checkout under a `go.work` that doesn't list daiku, prefix commands
  with `GOWORK=off`.
- CI runs both on Go 1.22 and 1.24 (`.github/workflows/test.yml`).
- Conventional commit messages.
