# `--json` cases

Fixture cases for `--json` on the read commands in spec R5 (ADR-0004). They have
the same format as `fixtures/cases/` and use the same shared workspaces, but they
run only against the Rust daiku, because the Go CLI has no `--json`:

```sh
daiku-fixtures check   --bin target/debug/daiku --cases fixtures/json-cases
daiku-fixtures capture --bin target/debug/daiku --cases fixtures/json-cases [CASE...]
```

Because a capture records what the Rust binary prints, read every expectation it
writes before you commit it.

Each `json-<name>` case uses the inputs of the Go case `fixtures/cases/<name>`,
plus `--json`, so its `data` can be compared with the text output Go was checked
against (AC3). A few cases have no Go case of the same name; each of these takes
another Go case's starting files and changes only its `args`.

`crates/daiku-fixtures/tests/json_cases.rs` checks every case here. Each case's
expected output must be one envelope with nothing on stderr, with the exit code the
contract gives. Every read form must have a data case and an error case. A new read
form needs an entry in that test's `FORMS`, and both kinds of case.

`json-status-repo-explore-skipped` covers the envelope's `warnings`. Its Go twin is
`fixtures/cases/status-repo-explore-skipped`, which prints the same warning to stderr.

A stray `--json` on a command that isn't a read form must behave as it does in Go.
Those cases are Go cases, in `fixtures/cases/` (`*-json-flag`).
