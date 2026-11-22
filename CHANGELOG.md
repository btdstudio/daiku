# Changelog

Releases before 0.3.0 were the Go CLI, the last being `v0.2.0`; their notes are on
this repo's GitHub releases. The `0.3.0-alpha` prereleases were built in
`btdstudio/daiku-next` before it merged here.

## 0.7.1

Remotes that can't leak their credentials.

- **`daiku repo add` refuses a password in a remote of any scheme**, not only
  `https://`. An `ssh://`, `git://` or `ftp://` remote with a password in its user
  part is refused, so the password never reaches the committed `REPOS.json`. A user
  with no password (`ssh://git@host/x`) and scp-style remotes (`git@host:org/x.git`)
  are accepted as before.
- **An `http(s)` remote with any user is refused, even when the URL is malformed.**
  A bare token in the user field, such as ` https://tok@host/x` (a leading space) or
  `https://tok@host name/x`, used to slip through because the URL didn't parse.
  `repo add` now refuses every `http(s)` remote that `repo list` would mask.
- **A remote holding an ASCII control character is refused**, tab, newline and CR
  included. A leading control character could hide a remote's scheme, and so its
  credentials, from both the check and the mask. A pasted remote with a trailing
  newline is refused too; git can't use one either.
- **Refusal messages mask credentials.** `invalid remote` and `remote helpers aren't
  allowed` show any userinfo after `://` as `***`, so a dash-led `ssh://` remote with
  a password prints as `-ssh://***@host/x`. That holds on the terminal, in `repo doctor --clone`'s
  report, and in the error `daiku-core` hands to other clients.
- **`repo list` masks more.** A stored remote whose scheme is hidden behind a `-`, a
  `helper::` prefix or a control character now shows its userinfo as `***`.
- **`repo doctor --clone` checks a remote before git runs.** A `REPOS.json` that
  arrives through a pull still loads, and its remotes list masked. But a remote that
  `repo add` would refuse is not cloned, and git never sees it.

`docs/migration.md` lists these as differences from the Go CLI, which accepts all of
these remotes.

## 0.7.0

Tagged, but published without binaries: the release notes held an example that
GitHub Actions took for a credential, so the build jobs never ran. Its changes ship in
0.7.1, unchanged.

## 0.6.0

Model routing that acts, and a `CLAUDE.md` that imports AGENTS.md.

- **`CLAUDE.md` imports AGENTS.md.** A new workspace's `CLAUDE.md` is the single line
  `@AGENTS.md`, so Claude Code loads AGENTS.md at the start of every session instead
  of being asked to read it. That loads the whole file, about 21 KB with every room
  present, each time. `daiku update` replaces an unedited `CLAUDE.md` and skips one
  you edited. A harness that doesn't expand `@` imports sees only that line.
- **A workflow rule routes model tiers to subagents.** AGENTS.md's `## Workflow
  rules` gains a rule: for a `fast` or `standard` task, when the session isn't on that
  tier's model, hand the work to a subagent on that model, briefed with the routing
  row's persona and `Read` and `Skip` lists. A subagent never runs `daiku gate`,
  `reopen`, `switch`, `change close`, `commit` or `pr`; it proposes, and the session
  runs them once you approve. `deep` tasks stay in the session, and the agent says so
  if the session isn't on the `deep` model. If a tier has no model mapped, or the
  harness can't set a subagent's model, the work stays in the session. `daiku update`
  adds the rule to an existing workspace.
- **New workspaces get a clearer `## Model routing` paragraph** and an intro that says
  `CLAUDE.md` imports AGENTS.md. Both are text you own, so `daiku update` leaves an
  existing workspace's as it is; edit yours by hand to match. To save usage, keep the
  session on your `standard` model and switch to the `deep` one for `deep` tasks.

## 0.5.0

Safer output from shared files, archived repos, and stricter checks on what a
workspace's files can make daiku do.

- **Printed text can't act on your terminal.** Text from workspace files (ticket
  titles, idea reasons, repo remotes and branches, paths, whole ADR, idea and source
  files) now shows control characters as visible escapes: ESC as `\x1b`, a carriage
  return as `\r`, NEL as `\u0085`, a bidi override as `\u202e`, and invisible tag
  characters as `\U000e0041`. Each byte that isn't valid UTF-8 shows as `\xNN`, where
  it used to be printed raw. Tabs, emoji and every script print as they are, and so do
  newlines in whole files. The files themselves keep their bytes, and `--json` output
  is unchanged. See
  [`docs/migration.md`](https://github.com/btdstudio/daiku/blob/v0.5.0/docs/migration.md#what-differs).
- **Archive a repo.** `daiku repo archive <name> [--reason <r>]` marks a repo in
  `REPOS.json` as archived, and `daiku repo unarchive <name>` makes it live again. An
  archived repo keeps its entry for its builds' history, but it's never cloned, and
  `build new`, `change new --repo` and `idea promote --repo` refuse new work on it.
  Existing builds on it still read and close. `repo list` shows the state, in text and
  in `--json` (`archived`, `archivedReason`). `repo doctor` skips what's expected of a
  retired repo, but still checks a checkout that's on disk for a changed remote. Older
  CLIs ignore the new fields.
- **Credentials in remotes are masked.** `repo doctor`'s remote-mismatch line and
  `repo list` replace the userinfo of an `http(s)://` remote with `***`, and do the
  same for other `scheme://` remotes that carry a password, so a token stored in a
  checkout's origin no longer reaches the terminal or CI logs. Comparison and cloning
  still use the real value.
- **Repo names in `REPOS.json` are checked when it's read.** A hand-edited name like
  `../outside` used to let `repo doctor --clone --into <dir>` clone outside `<dir>`.
  Now every command that reads the registry fails first, with
  `daiku: REPOS.json: invalid repo name "../outside" (…)`.
- **`--reason` stays on one line.** A reason with line breaks, given to `gate`,
  `reopen`, `change close` or `tickets unfreeze`, is folded to one line before it's
  stored, so it can't add a forged entry to `LOG.md` or the commit message. A reason
  that's only line breaks counts as missing.
- **A failed `repo` command no longer leaves `REPOS.json` half-written** when
  `.daiku/paths.json` is a symlink: both files are checked before either is written.
- **The desktop app's design** is in
  [`docs/app/design.md`](https://github.com/btdstudio/daiku/blob/v0.5.0/docs/app/design.md): Tauri 2 with a React frontend, a
  read-only viewer first.

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
