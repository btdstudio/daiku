# daiku-next

The Rust rewrite of [daiku](https://github.com/btdstudio/daiku), the folder-driven
planning workspace CLI. Once it matches the Go CLI command for command, it replaces
the Go code in `btdstudio/daiku`.

## Crates

| Crate | What it is |
|---|---|
| `crates/daiku-core` | The library: workspaces, changes, gates, tickets. It returns data and never prints, so any client (the CLI, a desktop app) can use it. |
| `crates/daiku-cli` | The `daiku` binary: argument parsing, calling core and printing, for every Go command. |
| `crates/daiku-fixtures` | A test tool, not released. It runs any `daiku` binary against golden fixture cases captured from the Go CLI. |

## Install

Prereleases are published on this repository's Releases page, built by
[dist](https://github.com/axodotdev/cargo-dist) when a `v*` tag is pushed. The
repository is private until it merges into `btdstudio/daiku`, so download an archive with
`gh release download`. [`docs/migration.md`](docs/migration.md) walks through it, and
through moving from the Go binary.

## Build and test

The toolchain is pinned in `rust-toolchain.toml`; rustup installs it on first use.

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
```

Fixture cases live in `fixtures/cases/`. See `AGENTS.md` for how to check, capture
and measure coverage.

## Licence

MIT. See `LICENSE`.
