# AGENTS.md — daiku-next

The Rust rewrite of the daiku CLI. Read this before changing code.

## Layout

- `crates/daiku-core` — library. All daiku behaviour lives here.
- `crates/daiku-cli` — the `daiku` binary: parses arguments, calls core, prints.
- `crates/daiku-fixtures` — the golden fixture runner (`publish = false`).
- `fixtures/cases/` — fixture cases captured from the Go CLI.
- `fixtures/usage.txt` — the Go CLI's `daiku help` output, the command list that
  coverage measures against.

## Rules for daiku-core

- **Core prints nothing.** It returns serde-serializable data. Clippy denies
  `print_stdout` and `print_stderr` in this crate; don't allow them.
- **Errors** are one `thiserror` enum whose `Display` is the Go CLI's message text.
  Operations return a value plus warnings; they don't print warnings.
- **No UI, terminal or async crates.** Time, processes, events and directories come
  through the `Env` the caller passes in, so tests and other clients can replace them.
- **Dependency allowlist (ADR-0005).** Core may depend on `serde`, `serde_json`,
  `thiserror`, `sha2`, `regex`, `jiff`, `include_dir` and `schemars` only. The CLI adds
  no runtime dependency beyond core; argument parsing is a port of Go's parsers, not
  clap. Adding a runtime dependency to core or the CLI needs an ADR.
- `daiku-fixtures` may use test-only crates; nothing it uses reaches a released binary.

## Fixture cases

A case is a folder in `fixtures/cases/<name>/`:

```
case.toml   args = ["gate", "spec", "approved"]   # required
            env = { KEY = "{ROOT}/x" }            # optional
            unordered_stdout = false              # compare stdout lines sorted
            git = false                           # run in a git repo, record git-log
            skip_on = ["windows"]                 # macos, linux, windows
            deviations = ""                       # recorded differences from Go
before/     the starting workspace; an empty `.fixture-keep` keeps an empty folder
expect/     stdout, stderr, exit, tree.txt (d/f/l manifest), tree/ (file contents),
            git-log (git cases only)
```

The case folder, `case.toml`, `before` and `expect` may not be symlinks, nothing under
`expect/` may be one, and a symlink inside `before/` must be relative, resolve to
something that exists, and stay inside `before/` (also when followed through other
links). The runner refuses anything else, so a committed case can't pull files from
outside the repo into a run or into `expect/`.

Each run gets fresh temp folders for the workspace (the cwd), `HOME` and
`XDG_CONFIG_HOME`; `TZ=UTC`, `LANG=C`, `LC_ALL=C`; and no other variable from your
environment except `PATH` (plus `USERPROFILE` and `SystemRoot` on Windows). Git cases
start from a commit of `before/` with a fixed author and date; `git-log` lists only the
commits the binary made.

Before comparing, run-specific values become tokens: `{TIMESTAMP}`, `{TODAY}`,
`{ROOT}`, `{HOME}`, `{CONFIG}` and `{COMMIT}`. `{ROOT}`, `{HOME}` and `{CONFIG}` also
expand in `args` and `env`. `.json` files compare by value; other files compare as
text with LF line endings.

```sh
# check a binary against every case (or name some)
cargo run -p daiku-fixtures -- check --bin target/debug/daiku [CASE...]
# write expect/ from a binary's run (the Go CLI, for new cases)
cargo run -p daiku-fixtures -- capture --bin path/to/go/daiku CASE...
# list Go commands and verbs no case exercises
cargo run -p daiku-fixtures -- coverage
```

`capture` deletes a case's `expect/` before writing it. Review the result before
committing: it is the contract.

## Verify

Before handing work back, run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
```

CI runs the same four on macOS, Linux and Windows.
