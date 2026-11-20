# Changelog

Releases before 0.3.0 were the Go CLI, in `btdstudio/daiku`.

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
