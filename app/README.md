# daiku app

> **Work in progress.** Phase one isn't usable yet, and nothing is released: the
> daiku releases ship only the CLI. Build from source only to work on the app.

The daiku app is a desktop window onto a daiku workspace. It reads the same folders
the CLI does, through `daiku-core`, and shows what the CLI's `--json` shows. This
folder holds the first scaffold: one placeholder screen that opens a folder and shows
its `status`. See [docs/app/design.md](../docs/app/design.md) for what the app is
meant to become.

The app is a Tauri 2 shell around a Vite, React and TypeScript frontend. Its Rust
crate, `src-tauri/`, is its own Cargo workspace, so the root `cargo` commands, CI and
releases don't build it.

## Prerequisites

- macOS 13.3 or later. Only macOS is supported for now.
- Command Line Tools (`xcode-select --install`). Xcode itself isn't needed.
- node 26 and npm 11, as `.nvmrc` says. Vite needs node 20.19 or 22.12 at least; an
  older node fails at `npm run dev` with an error about `styleText`. With nvm, run
  `nvm use` here (or `nvm install` first).
- Rust, at the version the repo's `rust-toolchain.toml` pins. `rustup` installs it.

## Build and run

From this folder:

```sh
npm ci
npm run tauri dev                          # run with hot reload
npm run tauri build -- --bundles app       # build an unsigned daiku.app
```

The `.app` lands in `src-tauri/target/release/bundle/macos/daiku.app`.

Other scripts: `npm run gen:types` writes `src/generated/` from
`docs/json-schema/`, `npm run lint`, `npm test` and `npm run build`. In `src-tauri/`,
use the usual `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
`cargo test`.

## First launch

The app isn't signed. The first time you open it, macOS blocks it. Open System
Settings, go to Privacy & Security, and choose Open Anyway next to daiku. Then click
"Open workspace…" and pick a workspace folder.

## macOS floor

`bundle.macOS.minimumSystemVersion` in `src-tauri/tauri.conf.json` is `13.3`. Tailwind
v4 needs Safari 16.4 or later ([Tailwind's compatibility notes](https://tailwindcss.com/docs/compatibility)).
[Tauri's webview versions page](https://v2.tauri.app/reference/webview-versions/) lists
macOS 13.3 with WebKit 615.1.26.11.22, which is Safari 16.4. So 13.3 meets the floor
and stays. Checked on 2026-11-23.

## Measurements

Targets come from the architecture: a cold start of 2 s and a watcher delay of 1 s.
A median worse than twice its target is called out here, and raised before the
production watcher (S-0019) is built.

| What | Method | Machine | Date | Result | Target |
|---|---|---|---|---|---|
| Cold start (median of 5) | Release `.app` built with `--features measure`; time from just before launching the binary to the app printing `ready` once the first `status` data is rendered, with the workspace named by `DAIKU_APP_MEASURE_ROOT`. Runs: 698, 341, 385, 393, 366 ms | macOS 27.2.1, Apple M4 | 2026-11-23 | 385 ms | 2 s |
| Watcher delay (median of 10) | `src-tauri/examples/watch_probe.rs` (`notify`, recursive) on a copy of the workspace; `daiku gate` and `daiku reopen` alternate, 2 s apart; time from the command's exit to the probe's `event` line. Runs: -18, -7, -7, -7, -7, -8, -6, -8, -5, -8 ms (re-run for the review) | macOS 27.2.1, Apple M4 | 2026-11-23 | -7 ms | 1 s |
| `.app` size | `du -sk` of `daiku.app`: default build 13964 KB, measure build 13968 KB | macOS 27.2.1, Apple M4 | 2026-11-23 | 13.6 MB | none |

Neither median is over twice its target. The watcher figure is negative because the
probe sees the write a few milliseconds before the `daiku` command exits: the delay
from write to event is under about 20 ms here. This is the raw watcher; the
debounced watcher of S-0019 will add its own delay. The bundle includes every
subset of the Inter and JetBrains Mono fonts.

To measure again: `npm run measure -- <path to a daiku workspace>`. It reads that
workspace and changes only a copy of it.
