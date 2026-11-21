# Changelog

Releases before 0.3.0 were the Go CLI, in `btdstudio/daiku`.

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
