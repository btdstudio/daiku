# AGENTS.md — daiku-next

The Rust rewrite of the daiku CLI. Read this before changing code.

## Layout

- `crates/daiku-core` — library. All daiku behaviour lives here.
- `crates/daiku-cli` — the `daiku` binary: parses arguments, calls core, prints.
- `crates/daiku-fixtures` — the golden fixture runner (`publish = false`).
- `fixtures/cases/` — fixture cases captured from the Go CLI at `5aa7a78`.
- `fixtures/workspaces/` — shared starting workspaces the cases build on.
- `fixtures/ERROR-PATHS.md` — which error classes each command has, and the case for
  each. A self-test keeps it in step with `fixtures/cases/`.
- `fixtures/usage.txt` — the Go CLI's `daiku help` output, the command list that
  coverage measures against.
- `scripts/build-go-daiku.sh` — builds that Go CLI into `target/go-daiku/daiku`.

## Rules for daiku-core

- **Core prints nothing.** It returns serde-serializable data. Clippy denies
  `print_stdout` and `print_stderr` in this crate; don't allow them.
- **Errors** are one `thiserror` enum whose `Display` is the Go CLI's message text.
  Operations return a value plus warnings; they don't print warnings.
- **No UI, terminal or async crates.** Time, processes, events and directories come
  through the `Env` the caller passes in, so tests and other clients can replace them.
  Core never reads the process environment or the clock, and never spawns a process
  itself; `daiku_core::fake` has test versions of each.
- **Every write goes through `daiku_core::write`** (`write_file`, `create_dir_all`,
  `open_file`), which refuses paths outside the workspace or through a symlink.
- **Error text lives in `Error`'s `Display`.** The CLI prints `{err}` and adds
  nothing, so a new message is a new variant, worded as Go words it. Use
  `daiku_core::gofmt` for Go's quoting (`%q`) and paths (`path.Join`, `IsLocal`),
  and `daiku_core::gojson::parse` to read JSON with Go's error wording. To decode
  a file Go reads into a struct (`STATE.json`, `REPOS.json`), use
  `gojson::decode`'s helpers, which keep `json.Unmarshal`'s rules and type
  errors, and pin the wording with a probe like `scripts/state-messages.go`.
- **Ports follow the Go code** at `5aa7a78` function by function, with Go's tests
  translated alongside.
- **Transitions go through `state::Store`**, which saves `STATE.json`, appends
  `LOG.md`, refreshes the plan overview, and commits through `Env.process` in a
  git workspace (never under `vcs: none`). Rules that decide whether a transition
  may happen (order, artifacts, the repo lock, the `gates` policy) live in
  `changes.rs`, not the CLI.
- **Dependency allowlist (ADR-0005).** Core may depend on `serde`, `serde_json`,
  `thiserror`, `sha2`, `regex`, `jiff`, `include_dir` and `schemars` only. The CLI adds
  no runtime dependency beyond core; argument parsing is a port of Go's parsers, not
  clap. Adding a runtime dependency to core or the CLI needs an ADR.
- `daiku-fixtures` may use test-only crates; nothing it uses reaches a released binary.

## The scaffold

`crates/daiku-core/scaffold/` is the workspace scaffold `daiku init` writes, embedded
in the binary with `include_dir`. It came from Go daiku at `5aa7a78` byte for byte,
and its manifest hashes must keep matching Go's golden manifest
(`crates/daiku-core/tests/data/golden-manifest.json`) until the scaffold is
deliberately changed.

- `.gitattributes` marks it `-text`, so no checkout changes its line endings.
- Its own `root/.gitignore` and `root/.gitattributes` only apply under
  `scaffold/root/`, which holds none of the files they name; they hide nothing in
  this repo.
- Changing a scaffold file changes its manifest hash: update the golden manifest and
  say why in the change.

## Fixture cases

A case is a folder in `fixtures/cases/<name>/` holding `case.toml`: the inputs you
write, then the `[expect]` table that `capture` writes.

```toml
args = ["gate", "intake", "approved"]   # required
workspace = "plan-intake-draft"         # optional: a shared starting workspace
# env = { KEY = "{ROOT}/x" }            # extra variables (see the rules below)
# unordered_stdout = false              # compare stdout lines sorted
# git = false                           # run in a git repo, record git_log
# skip_on = ["windows"]                 # macos, linux, windows
# tools = ["git"]                       # PATH holds only these (Unix only)
# deviations = ""                       # recorded differences from Go

[expect]
exit = 0
stdout = "1-auth-flow: intake = approved\n"
changes = [
  "~ f LOG.md",
  "~ f STATE.json",
  "+ f plan/specs/1-auth-flow/README.md",
]
```

`[expect]` holds `exit` and, when not empty, `stdout`, `stderr`, `changes` and
`git_log` (git cases). Output that isn't valid UTF-8 is stored as `stdout_bytes` or
`stderr_bytes` instead: the text as it is, with `\` written `\\` and each byte that
isn't part of valid UTF-8 written `\xNN`. A case sets one form or the other, never
both. Normalisation (`{ROOT}`, `{TODAY}` and the rest) applies to the valid UTF-8 runs
of output and files alike, and files that aren't valid UTF-8 compare byte for byte. It must be the last table: `capture` replaces everything from
its header on, keeps the lines above it exactly as written, and refuses if the
result wouldn't read back the same.

`changes` lists what the run did to the workspace, compared with its starting files:
`+ f path` added, `- f path` removed, `~ f path` changed (also `d` folders and
`l path -> target` links). Files the run didn't touch aren't stored, but they're still
checked: a change the expectation doesn't list fails the case.

Beside `case.toml`, only what a case needs:

```
before/     the starting workspace, or files added to or replacing the shared one;
            an empty `.fixture-keep` keeps an empty folder
config/     files for the run's global config folder (XDG_CONFIG_HOME), e.g.
            config/daiku/settings.json
stage/      git cases only: files added after the base commit and staged
            (`git add -A`), so the binary starts with staged changes
expect/     the files the run added or changed (`+ f` and `~ f` in `changes`), at
            their paths; absent when the run wrote no file
```

Fixture data never holds a file named `.gitignore` or `.gitattributes`, because git
would apply it to this repo (the daiku scaffold's `.gitignore` would hide
`.daiku/active`). Store them as `.gitignore.fixture` and `.gitattributes.fixture`; the
runner renames them on the way in and out, and refuses the real names.

The starting files are copied in this order: the workspace's `tree/` and `config/`,
then the case's `before/` and `config/`. A case file may replace a workspace file, and
a link in `before/` may replace a workspace file, but nothing may replace a folder.
`{ROOT}`, `{HOME}` and `{CONFIG}` expand in every UTF-8 starting file, which is how
`.daiku/paths.json` names the sandbox.

The case folder, `case.toml`, `before`, `config`, `stage` and `expect`, and a workspace's
folder and `tree`, may not be symlinks; nothing under `expect/` may be one; and a
symlink inside a starting folder must be relative, resolve to something that exists,
and stay inside that folder (also when followed through other links). The runner
refuses anything else, so a committed case can't pull files from outside the repo
into a run or into `expect/`.

Case data may not reach git's configuration, which can name commands for git to run:
every process in a run gets an empty global git config (`GIT_CONFIG_GLOBAL`) and
`GIT_CONFIG_NOSYSTEM=1`; a `config/` folder may not contain `git/`; no starting file
may be named `.git`; and a case's `env` may not set `PATH`, `HOME`, `TZ`, `LANG`,
`LC_ALL`, `USERPROFILE`, `SystemRoot`, or anything starting with `XDG_`, `GIT_`, `LD_`
or `DYLD_`.

A case that writes through a symlink, or uses `tools`, carries `skip_on = ["windows"]`.
`daiku pr` cases that get as far as running `gh` use `tools = ["git"]`, so `gh` is
never found and nothing reaches the network.

Each run gets fresh temp folders for the workspace (the cwd), `HOME` and
`XDG_CONFIG_HOME`; `TZ=UTC`, `LANG=C`, `LC_ALL=C`; and no other variable from your
environment except `PATH` (plus `USERPROFILE` and `SystemRoot` on Windows); git
reads no global or system config. Git cases
start from a commit of `before/` with a fixed author and date; `git-log` lists only the
commits the binary made.

Before comparing, run-specific values become tokens: `{TIMESTAMP}`, `{TODAY}`,
`{ROOT}`, `{HOME}`, `{CONFIG}` and `{COMMIT}`. `{ROOT}`, `{HOME}` and `{CONFIG}` also
expand in `args` and `env`. `.json` files compare by value; other files and outputs compare as
text with LF line endings.

```sh
# check a binary against every case (or name some)
cargo run -p daiku-fixtures -- check --bin target/debug/daiku [CASE...]
# write expect/ from a binary's run (the Go CLI, for new cases)
cargo run -p daiku-fixtures -- capture --bin path/to/go/daiku CASE...
# list Go commands and verbs no case exercises
cargo run -p daiku-fixtures -- coverage
```

`capture` rewrites a case's `[expect]` and `expect/`. Review the result before
committing: it is the contract. It warns (`warning: <case>: <file> contains today's
date <date>`) when a starting file holds the run's date, which `{TODAY}` would turn
into a token; use a fixed date in January 2026 instead.

### The Go CLI

The cases describe Go daiku at `5aa7a78`, built by `scripts/build-go-daiku.sh` from a
clean worktree with Go 1.26.5, `GOWORK=off` and `-buildvcs=false`, so `daiku version`
prints `dev`. It clones `btdstudio/daiku` into `target/go-daiku/src` the first time,
or takes the commit from a clone named by `DAIKU_GO_SRC`. CI's `fixtures` job builds
it the same way, checks every case against it on Linux and macOS, runs `coverage`,
and checks that `fixtures/usage.txt` equals its `daiku help`.

```sh
scripts/build-go-daiku.sh
cargo run -p daiku-fixtures -- check --bin target/go-daiku/daiku
```

### Shared workspaces

`fixtures/workspaces/<name>/recipe.toml` says how a workspace is made; `tree/` and
`config/` beside it are the result, written by the `workspaces` command. Don't edit
them by hand: change the recipe and rebuild.

```toml
from = "plan-intake-draft"        # optional parent, built first
git = false                       # run the steps in a git repo
steps = [["gate", "intake", "approved"]]   # daiku argument lists; each must exit 0
[[files]]                         # written after the steps
path = "plan/specs/1-auth-flow/functional-spec.md"   # under config/ → config folder
text = "..."
```

```sh
# rebuild every workspace (or name some; missing parents are built too)
cargo run -p daiku-fixtures -- workspaces --bin target/go-daiku/daiku [NAME...]
```

Stored workspaces hold fixed values, not the build's: timestamps become
`2026-01-15T00:00:00.000Z`, dates `2026-01-15`, and sandbox paths `{ROOT}`, `{HOME}`
and `{CONFIG}`. `.git/` is dropped; a git case gets a fresh repo from `git = true`.
Rebuilding gives the same bytes, so after a rebuild `git status` shows only what the
recipe change meant to change.

Names say the state: `init`, `plan-intake-draft` through `plan-published`,
`build-ticket-draft`, `build-implement`, `build-ship`, a `git-` twin of each, and a few
for one purpose (`ideas`, `adrs`, `plan-closed-change`, `build-two-in-repo`). Cases are
named `<command>[-<verb>]-<situation>`; add each new case to `fixtures/ERROR-PATHS.md`.

## Verify

Before handing work back, run:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test --workspace
```

CI runs the same four on macOS, Linux and Windows, and the `fixtures` job above on
Linux and macOS. When you add or recapture cases, also run the Go check.

### Porting a command

`fixtures/ported.txt` lists the cases the Rust `daiku` must pass; CI's
`rust-fixtures` job checks them on all three OSes:

```sh
cargo build -p daiku-cli
cargo run -p daiku-fixtures -- check --bin target/debug/daiku --list fixtures/ported.txt
```

When a story ports a command, it adds that command's cases to the list (all of them,
error paths included) and they must pass before the story ships. Commands not ported
yet print `daiku: <command> isn't implemented yet`.
