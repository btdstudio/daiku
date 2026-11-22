# Moving from the Go daiku to the Rust daiku

From `v0.3.0` on, daiku is written in Rust. It does everything the Go CLI did, on the
same workspaces. This note moves a machine from the `go install`ed binary to the Rust
one, and says what to expect.

**You don't need to change any workspace.** File formats keep their meaning, and ticket
hashes match Go's byte for byte, so no ticket needs re-marking.

## 1. Install the Rust binary

Each release has an install script for macOS and Linux, and one for Windows:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/btdstudio/daiku/releases/download/v0.6.0/daiku-cli-installer.sh | sh
```

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/btdstudio/daiku/releases/download/v0.6.0/daiku-cli-installer.ps1 | iex"
```

To always get the newest release, put `latest/download` in place of
`download/v0.6.0` in the URL.

### What the install scripts change on your machine

By default they:

- install `daiku` into `$CARGO_HOME/bin`, which is `~/.cargo/bin` unless you've set
  `CARGO_HOME` (`%USERPROFILE%\.cargo\bin` on Windows);
- **edit your shell setup** to put that folder on `PATH`, at the front.
  - The shell script writes `~/.cargo/env`, and adds a line that sources it to your
    shell's startup file: `~/.profile`, `~/.zshrc` or `~/.zshenv`, or a file under
    `~/.config/fish/conf.d`, depending on your shell.
  - The PowerShell script changes your user `Path` (the `Environment.Path` registry
    key).

The folder goes first, which usually beats the Go binary's folder, but check with
`command -v daiku` in a new terminal, as in step 2.

To keep the installer away from your `PATH` and startup files, set
`DAIKU_CLI_NO_MODIFY_PATH=1` before you run it. `DAIKU_CLI_UNMANAGED_INSTALL=<folder>`
installs into that folder and changes nothing else.

If you installed a `v0.3.0-alpha` release by hand into `~/.local/bin`, remove that copy,
or make sure `~/.cargo/bin` comes before `~/.local/bin` on your `PATH`.

The shell script checks the archive's sha256 before installing it. The PowerShell
script doesn't, so on Windows the checked install is the archive below, with its
checksum and `gh attestation verify`.

### Or download the archive

Each release also has an archive per platform, with its checksum:

| Platform | `target` |
|---|---|
| macOS, Apple silicon | `aarch64-apple-darwin` |
| macOS, Intel | `x86_64-apple-darwin` |
| Linux, x86-64 | `x86_64-unknown-linux-gnu` |
| Windows, x86-64 | `x86_64-pc-windows-msvc` |

On macOS and Linux (bash, zsh):

```sh
tag=v0.6.0
target=aarch64-apple-darwin        # from the table above
base="https://github.com/btdstudio/daiku/releases/download/$tag"

mkdir -p daiku-dl && cd daiku-dl
curl -LsSf -O "$base/daiku-cli-$target.tar.xz" -O "$base/daiku-cli-$target.tar.xz.sha256"
head -n 1 "daiku-cli-$target.tar.xz.sha256" | shasum -a 256 -c     # Linux: sha256sum -c
tar -xJf "daiku-cli-$target.tar.xz"

mkdir -p ~/.local/bin
install -m 755 "daiku-cli-$target/daiku" ~/.local/bin/daiku
```

`head -n 1` skips the blank line dist leaves after the checksum, which `shasum` would
warn about. `~/.local/bin` must come before the Go binary's folder on your `PATH` (step 2
checks that). If it isn't on `PATH` at all, add `export PATH="$HOME/.local/bin:$PATH"` to
your shell's startup file.

On Windows (PowerShell):

```powershell
$tag = "v0.6.0"
$target = "x86_64-pc-windows-msvc"
$base = "https://github.com/btdstudio/daiku/releases/download/$tag"

New-Item -ItemType Directory -Force daiku-dl | Out-Null
Invoke-WebRequest "$base/daiku-cli-$target.zip" -OutFile "daiku-dl\daiku-cli-$target.zip"
Invoke-WebRequest "$base/daiku-cli-$target.zip.sha256" -OutFile "daiku-dl\daiku-cli-$target.zip.sha256"
$want = (Get-Content "daiku-dl\daiku-cli-$target.zip.sha256").Split(" ")[0]
if ((Get-FileHash "daiku-dl\daiku-cli-$target.zip" -Algorithm SHA256).Hash -ne $want) { throw "checksum mismatch" }
Expand-Archive "daiku-dl\daiku-cli-$target.zip" -DestinationPath daiku-dl

New-Item -ItemType Directory -Force "$HOME\.local\bin" | Out-Null
Copy-Item "daiku-dl\daiku.exe" "$HOME\.local\bin\daiku.exe"      # the zip holds daiku.exe at its top level
[Environment]::SetEnvironmentVariable("Path", "$HOME\.local\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```

Open a new terminal afterwards, so the changed `PATH` takes effect.

**Checking where an archive came from.** Every archive has a signed build attestation.
With the GitHub CLI:

```sh
gh attestation verify "daiku-cli-$target.tar.xz" --repo btdstudio/daiku --signer-workflow btdstudio/daiku/.github/workflows/release.yml
```

`--signer-workflow` accepts only an attestation made by the release workflow. Use
`.zip` in place of `.tar.xz` on Windows.

## 2. Take over from the Go binary

A `go install`ed daiku sits in `$GOBIN`, or in `$GOPATH/bin` when `GOBIN` is empty:

```sh
command -v daiku                 # which daiku runs now
go env GOBIN GOPATH              # where go install puts binaries
```

PowerShell: `Get-Command daiku -All`, and the same `go env`.

Set the Go binary aside rather than deleting it, so going back is one rename:

```sh
mv "$(go env GOPATH)/bin/daiku" "$(go env GOPATH)/bin/daiku-go"     # or $GOBIN/daiku
```

PowerShell: `Rename-Item "$(go env GOPATH)\bin\daiku.exe" daiku-go.exe`.

Check that the Rust one is now the one that runs:

```sh
command -v daiku                 # ~/.cargo/bin/daiku (or ~/.local/bin/daiku from an archive)
daiku version                    # daiku v0.6.0
```

The Go binary prints its own version (`daiku v0.2.0` or earlier, or `daiku dev` for a
build from a checkout), so `daiku v0.3.0` or later means it's the Rust binary.

## 3. Check an existing workspace

In a workspace the Go CLI made:

```sh
daiku status
daiku tickets check              # reports "<n> tickets ok", with no drift
```

`tickets check` reports any ticket that changed since it was frozen. After the move it
reports none: ticket hashes are computed the same way as Go's (`<`, `>` and `&` in a
title or label included), so nothing needs re-marking.

## `go install` after v0.3.0

`v0.3.0` and later have no Go module. So
`go install github.com/btdstudio/daiku/cmd/daiku@latest` now fails, saying the module
doesn't contain that package. Install the Rust CLI as in step 1 instead.

The last Go release is tagged `v0.2.0`, and it still installs:

```sh
go install github.com/btdstudio/daiku/cmd/daiku@v0.2.0
```

## What stays the same

- **Commands, verbs, flags, output and exit codes** matched the Go CLI's at `v0.3.0`.
  That release passed every one of the Go CLI's golden fixture cases on Linux, macOS
  and Windows.
- **Workspace files** keep their formats and meaning: `STATE.json`, `LOG.md`,
  `REPOS.json`, ticket frontmatter, the plan overview. The Rust CLI reads what Go wrote,
  and at `v0.3.0` Go read what the Rust CLI wrote.
- **Global settings** are in the same place, `.config/daiku/settings.json` under
  `XDG_CONFIG_HOME` or your home folder, on every OS.
- **`daiku update`** refreshes a workspace's managed files and the version in its
  manifest. Run it when you want the workspace to record the new version.

## What differs

After `v0.3.0`, daiku no longer checks itself against the Go CLI, so new behaviour may
differ from Go's. `CHANGELOG.md` lists each change. The differences at `v0.3.0` are
below; each is deliberate and small.

- **JSON files** the Rust CLI writes can differ from Go's in escaping only (`<`, `>` and
  `&` aren't written as `\u003c`, `\u003e` and `\u0026`, as Go writes them).
  They mean the same, and both CLIs read both.
- **Text that isn't valid UTF-8** (a Latin-1 or Windows-1252 title, say) is kept
  byte for byte in the files daiku writes back. In two places it's shown with U+FFFD
  instead:
  - `daiku tickets order` prints such a ticket's title that way;
  - so does the plan overview's generated region.

  `daiku tickets unfreeze` refuses a ticket whose frontmatter isn't valid UTF-8, where
  Go would write it back.
- **On the terminal, control characters and invalid UTF-8 show as visible escapes**
  (from `v0.5.0`, ADR-0008). A title holding ESC prints as `\x1b`, a NEL as
  `\u0085`, a bidi override as `\u202e`, an invisible tag character as
  `\U000e0041`, and each byte that isn't valid UTF-8 as `\xNN`, where Go printed
  them raw. Tabs print as they are, and so do newlines in whole files (`adr show`,
  `idea show`, `source show`). The files themselves keep their bytes, and `--json`
  output is unchanged.
- **`daiku repo add` and `daiku repo doctor --clone` refuse more remotes**, which Go
  lets through:
  - an `https://` remote with credentials whose host name holds a percent-escaped
    letter (`h%41`);
  - a password in the userinfo of a remote in any scheme, such as
    `ssh://user:pass@host/x`;
  - any userinfo in an `http(s)://` remote that Go's URL parser rejects, such as
    ` https://tok@host/x` with a leading space, or `https://tok@host name/x`;
  - a remote holding an ASCII control character, a tab or newline included, which Go
    accepts.

  Refusal messages show any userinfo in the remote as `***`, where Go prints it as
  written. A `REPOS.json` that already holds such a remote still loads and lists it
  masked; only the clone is refused.
- **`--json`** is new. Every read command (`status`, `switch`, `settings`,
  `source`, `adr`, `idea` and `tickets` reads, `repo list`, `references check`, `env`
  and `version`) accepts it, and writes one versioned JSON document instead of its
  text. [`docs/json.md`](json.md) is the reference.

## Going back to Go

Put the renamed binary back:

```sh
mv "$(go env GOPATH)/bin/daiku-go" "$(go env GOPATH)/bin/daiku"
```

Or install the last Go release:

```sh
go install github.com/btdstudio/daiku/cmd/daiku@v0.2.0
```
