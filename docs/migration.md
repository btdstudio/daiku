# Moving from the Go daiku to the Rust daiku

The Rust CLI does everything the Go CLI does, on the same workspaces. This note moves a
machine from the `go install`ed binary to the Rust one, and says what to expect.

**You don't need to change any workspace.** File formats keep their meaning, and ticket
hashes match Go's byte for byte, so no ticket needs re-marking.

## 1. Get the Rust binary

Until the code merges into the public `btdstudio/daiku`, this repository is private. The
install scripts attached to each release (`curl … | sh`, `irm … | iex`) download without
logging in, so the one-liners don't work yet. (dist's installers can send a GitHub token
read from `DAIKU_CLI_GITHUB_TOKEN` for a private repository; that hasn't been tried against
this one.) What has been tried is to download the archive for your platform with the GitHub
CLI (`gh auth status` should show you logged in with access to `btdstudio/daiku-next`).

| Platform | `target` |
|---|---|
| macOS, Apple silicon | `aarch64-apple-darwin` |
| macOS, Intel | `x86_64-apple-darwin` |
| Linux, x86-64 | `x86_64-unknown-linux-gnu` |
| Windows, x86-64 | `x86_64-pc-windows-msvc` |

### macOS and Linux (bash, zsh)

```sh
tag=v0.3.0-alpha.1
target=aarch64-apple-darwin        # from the table above

gh release download "$tag" -R btdstudio/daiku-next -D daiku-dl -p "daiku-cli-$target.tar.xz*"
(cd daiku-dl && head -n 1 "daiku-cli-$target.tar.xz.sha256" | shasum -a 256 -c)   # Linux: sha256sum -c
tar -xJf "daiku-dl/daiku-cli-$target.tar.xz" -C daiku-dl

mkdir -p ~/.local/bin
install -m 755 "daiku-dl/daiku-cli-$target/daiku" ~/.local/bin/daiku
```

`head -n 1` skips the blank line dist leaves after the checksum, which `shasum` would
warn about. `~/.local/bin` must come before the Go binary's folder on your `PATH` (step 2
checks that). If it isn't on `PATH` at all, add `export PATH="$HOME/.local/bin:$PATH"` to
your shell's startup file.

### Windows (PowerShell)

```powershell
$tag = "v0.3.0-alpha.1"
$target = "x86_64-pc-windows-msvc"

gh release download $tag -R btdstudio/daiku-next -D daiku-dl -p "daiku-cli-$target.zip*"
$want = (Get-Content "daiku-dl\daiku-cli-$target.zip.sha256").Split(" ")[0]
if ((Get-FileHash "daiku-dl\daiku-cli-$target.zip" -Algorithm SHA256).Hash -ne $want) { throw "checksum mismatch" }
Expand-Archive "daiku-dl\daiku-cli-$target.zip" -DestinationPath daiku-dl

New-Item -ItemType Directory -Force "$HOME\.local\bin" | Out-Null
Copy-Item "daiku-dl\daiku-cli-$target\daiku.exe" "$HOME\.local\bin\daiku.exe"
[Environment]::SetEnvironmentVariable("Path", "$HOME\.local\bin;" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")
```

Open a new terminal afterwards so the changed `PATH` takes effect.

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

Check the Rust one is now the one that runs:

```sh
command -v daiku                 # ~/.local/bin/daiku
daiku version                    # daiku v0.3.0-alpha.1
```

The Go binary prints its own version (`daiku v0.1.0`, or `daiku dev` for a build from a
checkout), so a `daiku v0.3.0-alpha.1` means it's the Rust binary.

## 3. Check an existing workspace

In a workspace the Go CLI made:

```sh
daiku status
daiku tickets check              # reports "<n> tickets ok", with no drift
```

`tickets check` reports a ticket that changed since it was frozen. After the move it
reports none: ticket hashes are computed the same way as Go's, `<`, `>` and `&` in a
title or label included, so nothing needs re-marking.

## What stays the same

- **Commands, verbs, flags, output and exit codes** match the Go CLI's. The Rust binary
  passes all 568 of the Go CLI's golden fixture cases on Linux, macOS and Windows.
- **Workspace files** keep their formats and meaning: `STATE.json`, `LOG.md`,
  `REPOS.json`, ticket frontmatter, the plan overview. The Rust CLI reads what Go wrote
  and Go reads what the Rust CLI writes.
- **Global settings** are in the same place, `.config/daiku/settings.json` under
  `XDG_CONFIG_HOME` or your home folder, on every OS.
- **`daiku update`** refreshes a workspace's managed files and the version in its
  manifest. Run it when you want the workspace to record the new version.

## What differs

Everything below is deliberate and small.

- **JSON files** the Rust CLI writes can differ from Go's in escaping only (`<`, `>` and
  `&` aren't written as `<`…). They mean the same, and both CLIs read both.
- **Text that isn't valid UTF-8** (a Latin-1 or Windows-1252 title, say) is kept
  byte for byte in the files daiku writes back. In two places it's shown with U+FFFD
  instead: `daiku tickets order` prints such a ticket's title that way, and the plan
  overview's generated region does. `daiku tickets unfreeze` refuses a ticket whose
  frontmatter isn't valid UTF-8, where Go would write it back.
- **`daiku repo add`** refuses an `https://` remote that carries credentials even when
  its host name holds a percent-escaped letter (`h%41`), which Go lets through.
- **`--json`** isn't there yet. It comes in a later alpha.

## Going back to Go

Put the renamed binary back:

```sh
mv "$(go env GOPATH)/bin/daiku-go" "$(go env GOPATH)/bin/daiku"
```

Or, once the last Go commit has been tagged (it's tagged just before the Rust code
replaces it in `btdstudio/daiku`):

```sh
go install github.com/btdstudio/daiku/cmd/daiku@v0.2.0
```

## After the merge: the install scripts

Once the code lives in the public `btdstudio/daiku`, each release's install scripts work,
and replace the manual download in step 1. Use the release's own tag in the URL, as the
release notes do. (GitHub's `releases/latest` skips prereleases, so it doesn't find an
`-alpha` release.)

```sh
tag=v0.3.0-alpha.1
curl --proto '=https' --tlsv1.2 -LsSf "https://github.com/btdstudio/daiku/releases/download/$tag/daiku-cli-installer.sh" | sh
```

```powershell
$tag = "v0.3.0-alpha.1"
powershell -ExecutionPolicy Bypass -c "irm https://github.com/btdstudio/daiku/releases/download/$tag/daiku-cli-installer.ps1 | iex"
```

### What the install scripts change on your machine

By default they:

- install `daiku` into `$CARGO_HOME/bin`, which is `~/.cargo/bin` unless you've set
  `CARGO_HOME` (`%USERPROFILE%\.cargo\bin` on Windows);
- **edit your shell setup** to put that folder on `PATH`, at the front. The shell script
  writes `~/.cargo/env` and adds a line that sources it to your shell's startup file
  (`~/.profile`, `~/.zshrc` or `~/.zshenv`, or a file under `~/.config/fish/conf.d`,
  depending on your shell). The PowerShell script changes your user `Path` (the
  `Environment.Path` registry key).

The folder goes first, which usually beats the Go binary's folder, but check with
`command -v daiku` in a new terminal, as in step 2. To keep the installer away from your
`PATH` and startup files, set `DAIKU_CLI_NO_MODIFY_PATH=1` before you run it.
`DAIKU_CLI_UNMANAGED_INSTALL=<folder>` installs into that folder and changes nothing else.

Steps 2 and 3 are the same afterwards.
