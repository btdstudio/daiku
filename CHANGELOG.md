# Changelog

Releases before 0.3.0 were the Go CLI, in `btdstudio/daiku`.

## 0.3.0-alpha.2

Full parity with the Go CLI plus `--json` on every read command, built from
`btdstudio/daiku-next` before it merges into `btdstudio/daiku`.

- **Every Go command is ported** and matches the Go CLI's behaviour. The 568 golden
  fixture cases were captured from the Go CLI, and all of them pass against it (Linux,
  macOS) and against the Rust CLI (Linux, macOS and Windows).
- **`--json` on every read command in spec R5.** `status` (and `status <plan-change>`),
  `switch` (listing), `settings`, `source list`/`show`, `adr list`/`show`,
  `idea list`/`show`, `tickets check`/`status`/`order`, `repo list`,
  `references check`, `env`, and `version` accept `--json` and emit one versioned
  envelope to stdout: `{"schema":1,"data":...}` on success, or
  `{"schema":1,"error":{"kind","message"}}` with exit 1 on failure. `data` is the
  serialised core read value (ADR-0004, R6).
- **Published schemas.** View and contract schemas are committed under
  `docs/json-schema/` and kept in step by a drift test (AC4).
- **Workspaces made by the Go CLI work unchanged.** File formats keep their meaning, and
  ticket hashes match Go's byte for byte, so no ticket needs re-marking.
- **Moving from the Go binary:** see
  [`docs/migration.md`](https://github.com/btdstudio/daiku-next/blob/v0.3.0-alpha.2/docs/migration.md).

**Installing from this release.** The repository is private until it merges, and the
install scripts below download without logging in, so use the archive download for now.
Download the archive for your platform with the GitHub CLI:

```sh
gh release download v0.3.0-alpha.2 -R btdstudio/daiku-next -p 'daiku-cli-<target>.*'
```

## 0.3.0-alpha.1

The first Rust prerelease of daiku, built from `btdstudio/daiku-next` before it merges
into `btdstudio/daiku`.

- **Every Go command is ported** and matches the Go CLI's behaviour. The 568 golden
  fixture cases were captured from the Go CLI, and all of them pass against it (Linux,
  macOS) and against the Rust CLI (Linux, macOS and Windows).
- **Workspaces made by the Go CLI work unchanged.** File formats keep their meaning, and
  ticket hashes match Go's byte for byte, so no ticket needs re-marking.
- **`--json` is not in this release.** It comes in a later alpha.
- **Moving from the Go binary:** see
  [`docs/migration.md`](https://github.com/btdstudio/daiku-next/blob/v0.3.0-alpha.1/docs/migration.md).

**Installing from this release.** The repository is private until it merges, and the
install scripts below download without logging in, so use the archive download for now.
Download the archive for your platform with the GitHub CLI:

```sh
gh release download v0.3.0-alpha.1 -R btdstudio/daiku-next -p 'daiku-cli-<target>.*'
```
