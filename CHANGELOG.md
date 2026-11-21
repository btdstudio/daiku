# Changelog

Releases before 0.3.0 were the Go CLI, the last being `v0.2.0`; their notes are on
this repo's GitHub releases. The `0.3.0-alpha` prereleases were built in
`btdstudio/daiku-next` before it merged here.

## 0.4.0

Model tiers in a workspace's AGENTS.md, and the Rust CLI as its own reference.

- **Each routing row names a model tier.** The routing tables in AGENTS.md gain a
  `Model` column, giving every persona and activity a tier: `deep`, `standard` or
  `fast`. A new `## Model routing` section, after `## Tech stack & standards`, maps
  each tier to a model for your harness. It starts with `<FILL IN>` placeholders and
  an example line for Claude Code; it's yours to edit, and `daiku update` keeps it. A
  tier applies when an agent hands a phase to a subagent, or when you pick the model
  at the start of a phase. Reading AGENTS.md doesn't switch a session's model.
- **`daiku update` adds a missing section of yours.** An existing workspace gets
  `## Model routing` with its defaults on the next `update`, along with the new column.
  A section of yours that's present is still kept byte for byte. If you deleted
  `## Tech stack & standards`, `update` now puts its placeholder back; to opt out of a
  section, keep its heading and clear its text.
- **daiku no longer checks itself against the Go CLI.** CI checks the Rust CLI against
  its own fixture cases, and from this release on its behaviour may differ from the Go
  CLI's `v0.2.0`. Workspaces made by the Go CLI still work. See
  [`docs/migration.md`](https://github.com/btdstudio/daiku/blob/v0.4.0/docs/migration.md).
- **Docs match how releases ship:** the supported versions in `SECURITY.md`, a note
  that the PowerShell installer doesn't check checksums (with the checked route for
  Windows), and attestation checks pinned to the release workflow.

## 0.3.0

daiku is now written in Rust. This release replaces the Go CLI in this repository with
the Rust workspace, and brings in its history from `btdstudio/daiku-next`.

- **The same daiku.** Every Go command, verb, flag, output and exit code is ported. The
  Go CLI's golden fixture cases all pass against it on Linux, macOS and Windows.
  Workspaces made by the Go CLI work unchanged, and ticket hashes match Go's, so no
  ticket needs re-marking.
- **Install with one command.** Each release has a shell installer for macOS and Linux,
  and a PowerShell one for Windows, as well as an archive per platform with its
  checksum. See [`docs/migration.md`](https://github.com/btdstudio/daiku/blob/v0.3.0/docs/migration.md),
  which also covers moving from the Go binary.
- **Signed builds.** Every archive has a build attestation:
  `gh attestation verify <archive> --repo btdstudio/daiku`.
- **`--json` on every read command.** It writes one versioned document,
  `{"schema":1,"data":...,"warnings":[...]}`, or an error with exit 1. See
  [`docs/json.md`](https://github.com/btdstudio/daiku/blob/v0.3.0/docs/json.md). Since
  `0.3.0-alpha.3`:
  - `tickets`, `idea` and `repo` with `--json` and no verb return a `Usage` error;
  - `switch`'s listing comes from the core library;
  - a test checks each `--json` case against the Go CLI's case on the same inputs.
- **`go install` stops at `v0.2.0`.** This release has no Go module, so
  `go install github.com/btdstudio/daiku/cmd/daiku@latest` fails.
  `go install github.com/btdstudio/daiku/cmd/daiku@v0.2.0` still installs the last Go
  release.

See [`CHANGELOG.md`](https://github.com/btdstudio/daiku/blob/v0.3.0/CHANGELOG.md) for the alpha releases' details of the port.

## 0.3.0-alpha.3

Fixes to the `--json` contract that `0.3.0-alpha.2` shipped, before anything is built
on it. The contract stays `schema: 1`. It gains one member, and some field names and
one error kind are different, so a script written against alpha.2 may need updating.

- **Warnings are in the envelope.** A success is now
  `{"schema":1,"data":...,"warnings":[...]}`. `warnings` is always present, and holds
  what the text form prints to stderr as `warning: ...`. In alpha.2, `status --json`
  dropped the warning for a repo source whose explore phase is skipped. The error
  envelope is unchanged (ADR-0004, amended 2026-11-21).
- **A stray `--json` behaves as it does in Go.** In alpha.2, `--json` was removed from
  the arguments of every `tickets` and `repo` verb. So `daiku repo add --json …` and
  `daiku tickets new … --json` wrote files where Go stops with a usage error, and
  `switch <change> --json` failed where Go switches. Now only the read forms in spec
  R5 treat `--json` as the flag. Every other command sees its arguments as Go would.
  `source show <id> <extra>` and `adr show <n> <extra>` work again, as in Go.
- **Every failing read under `--json` writes the error envelope.** A usage error, or
  an id that doesn't parse, used to print text to stderr and leave stdout empty. It's
  now `{"schema":1,"error":{"kind":"Usage","message":...}}`, with the text the
  command prints without `--json` as `message`. `Usage` replaces alpha.2's
  `InvalidArgs`. Arguments the text form ignores, `--json` now ignores too, so
  `settings x --json` and `tickets check x --json` succeed as their text forms do.
- **Field names are camelCase everywhere,** as in Go's files. `noChanges`, and in
  `settings`' `effective`: `baseSource`, `sourceDir`, `adrDir` and `syncFile`. alpha.2
  wrote these in snake_case. The schemas in `docs/json-schema/` are regenerated.
- **Tested.** `fixtures/json-cases/` has 56 cases: a data case and an error case for
  every read form, all run in CI on Linux, macOS and Windows. A test checks that each
  case writes one envelope with the right exit code and nothing on stderr. Six new Go
  cases pin the stray-`--json` behaviour and `status`'s warning against the Go CLI.

**Installing from this release:** as for 0.3.0-alpha.2 below, with the tag
`v0.3.0-alpha.3`.

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
