# `--json` output

Every read command accepts `--json` and then writes one JSON document to stdout,
instead of its text. This is the interface for scripts, mods and the app. It is
versioned and stable within a version (ADR-0004, amended 2026-11-21).

```sh
daiku status --json
daiku tickets check --json | jq '.data.errors'
```

`daiku help` doesn't mention `--json`; this page is the reference.

## The envelope

On success, the exit code is 0 (see [Exit codes](#exit-codes) for the one
exception), and stdout is:

```json
{
  "schema": 1,
  "data": { },
  "warnings": []
}
```

- **`schema`** is the contract's version, now `1`. A change that breaks a script
  bumps it. Adding a member doesn't.
- **`data`** is the value the command reads, with the same content its text form
  shows. Its shape is per command (see the table below), and field names are
  camelCase.
- **`warnings`** is always present. It holds what the text form prints to stderr
  as `warning: <message>`, without the prefix, in the same order. It is usually
  empty. `status`, for example, warns when a repo source's explore phase is
  skipped.

On failure, the exit code is 1, and stdout is:

```json
{
  "schema": 1,
  "error": {
    "kind": "SourceNotFound",
    "message": "daiku: source 9 not found at plan/00_intake/9.md"
  }
}
```

- **`kind`** is a stable name for the error. It is either the name of a daiku error
  (the full list is `error_kind` in `crates/daiku-core/src/json.rs`), or **`Usage`**
  when the arguments were wrong: a missing or extra argument, an id that doesn't
  parse, or a group with no verb.
- **`message`** is exactly what the command prints to stderr without `--json`,
  minus the trailing newline. For `Usage`, that's the usage line or block.

With `--json`, nothing is written to stderr, and a read never changes the
workspace.

## Commands

The schema files are generated from the code, and a test keeps them current.

| Command | `data` | Schema |
|---|---|---|
| `status` | the active change, and the open plans and builds | [`Success_Status.json`](json-schema/Success_Status.json) |
| `status <plan-change>` | the plan and its tickets' builds | [`Success_PlanStatus.json`](json-schema/Success_PlanStatus.json) |
| `switch`, `switch --all` | the changes listed: open ones, or all with `--all` | [`Success_KnownList.json`](json-schema/Success_KnownList.json) |
| `settings` | `global`, `project`, `effective` and `vcs` (see [Settings](#settings)) | [`Success_Loaded.json`](json-schema/Success_Loaded.json) |
| `source list` (or `source`) | the intake sources | [`Success_SourceList.json`](json-schema/Success_SourceList.json) |
| `source show <id>` | one source | [`Success_Source.json`](json-schema/Success_Source.json) |
| `adr list` (or `adr`) | the ADRs, with each file's text in `raw` | [`Success_AdrList.json`](json-schema/Success_AdrList.json) |
| `adr show <number>` | one ADR | [`Success_Adr.json`](json-schema/Success_Adr.json) |
| `idea list` | the ideas | [`Success_IdeaList.json`](json-schema/Success_IdeaList.json) |
| `idea show <id>` | `{"summary": <the summary file's text>}` | [`Success_IdeaSummary.json`](json-schema/Success_IdeaSummary.json) |
| `tickets check` | `errors`, `warnings` and `count` | [`Success_TicketsCheck.json`](json-schema/Success_TicketsCheck.json) |
| `tickets status` | the buckets, and the tickets that couldn't be read | [`Success_TicketsStatus.json`](json-schema/Success_TicketsStatus.json) |
| `tickets order` | the tickets in publish order | [`Success_TicketsOrder.json`](json-schema/Success_TicketsOrder.json) |
| `repo list` | the registered repos and their local paths | [`Success_RepoList.json`](json-schema/Success_RepoList.json) |
| `references check` | what the check found | [`Success_ReferenceCheck.json`](json-schema/Success_ReferenceCheck.json) |
| `env` | `{"flags": <the harness flags>}` | [`Success_EnvFlags.json`](json-schema/Success_EnvFlags.json) |
| `version` (or `--version`, `-v`) | `{"version": "v<version>"}` for a release, or `"dev"` for a build from source | [`Success_Version.json`](json-schema/Success_Version.json) |
| any failure | — | [`Failure.json`](json-schema/Failure.json) |

`--json` can go anywhere among the arguments. Text that isn't valid UTF-8 in your
files comes through as U+FFFD, since JSON strings can't hold raw bytes.

### Which invocations are reads

- A read group named with no verb is a read: `tickets`, `idea`, `repo` and
  `references` with `--json` return a `Usage` error holding the group's usage
  block. `source` and `adr` with no verb list.
- On any other command, `--json` is an extra argument, and the output is text:
  - Commands that change files reject it as an unknown flag or a usage error, on
    stderr. `daiku tickets new … --json` writes nothing.
  - `switch <change> --json` switches, and ignores the extra argument.
  - An unknown verb, such as `tickets frob --json`, prints the group's usage on
    stderr.
- `--help` wins over `--json`: `daiku status --json --help` prints help as text.

## Exit codes

| Result | Exit |
|---|---|
| success | 0 |
| any error, including `Usage` | 1 |
| `tickets check` whose report has errors | 1, with the report in `data` |

`tickets check` returns its report as `data` even when the report has errors,
because the findings are what a script wants. The exit code still says whether
the tickets passed, as the text form's does.

## Two kinds of warnings

- **The envelope's `warnings`:** problems the operation noticed beside its
  result, which text prints to stderr.
- **Warnings inside `data`:** part of the result itself. `tickets check`'s
  `data.warnings` are the ticket findings that text prints to stdout as `!` lines.
  They don't fail the check.

## Settings

`settings --json` gives the merged settings in `effective`, and each settings file
in `global` and `project`, **unchanged**: its keys as written, including any daiku
doesn't use. An object is empty when its file doesn't exist. Don't keep anything in
a daiku settings file that you wouldn't want printed.

## Stability

- Within `schema: 1`, members may be added, but none is removed, renamed or
  retyped.
- A breaking change bumps `schema`, and shows in the diff of `docs/json-schema/`.
- Each release's notes (`CHANGELOG.md`) say what changed. `0.3.0-alpha.3` fixed
  alpha.2's first version of the contract.
