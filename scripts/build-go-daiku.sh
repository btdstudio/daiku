#!/usr/bin/env bash
# Builds the Go daiku CLI that the golden fixtures describe, the same way everywhere.
#
#   scripts/build-go-daiku.sh            writes target/go-daiku/daiku
#
# It checks out btdstudio/daiku at a fixed commit in a temporary git worktree, so the
# state of any local clone doesn't matter, and builds with a pinned Go toolchain and
# without VCS stamping, so `daiku version` prints `dev`.
#
# DAIKU_GO_SRC may name an existing clone to take the commit from; otherwise the repo
# is cloned into target/go-daiku/src (once) and fetched from there.
set -euo pipefail

COMMIT=5aa7a78e64f79e6f9c06f1bcf74ee00437cc00cb
GO_VERSION=go1.26.5
REMOTE=https://github.com/btdstudio/daiku.git

root=$(cd "$(dirname "$0")/.." && pwd)
out="$root/target/go-daiku"

for tool in git go; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "build-go-daiku: $tool isn't on PATH" >&2
    exit 1
  fi
done

mkdir -p "$out"
src=${DAIKU_GO_SRC:-$out/src}
if [ ! -d "$src/.git" ] && [ ! -f "$src/HEAD" ]; then
  git clone --quiet --no-checkout "$REMOTE" "$src"
fi
if ! git -C "$src" cat-file -e "$COMMIT^{commit}" 2>/dev/null; then
  git -C "$src" fetch --quiet "$REMOTE" "$COMMIT"
fi

tree=$(mktemp -d "${TMPDIR:-/tmp}/go-daiku-XXXXXX")
cleanup() {
  git -C "$src" worktree remove --force "$tree/daiku" >/dev/null 2>&1 || true
  rm -rf "$tree"
}
trap cleanup EXIT
git -C "$src" worktree add --quiet --detach "$tree/daiku" "$COMMIT"

(
  cd "$tree/daiku"
  GOTOOLCHAIN=$GO_VERSION GOWORK=off GOFLAGS= CGO_ENABLED=0 \
    go build -trimpath -buildvcs=false -o "$out/daiku" ./cmd/daiku
)

echo "built $out/daiku ($("$out/daiku" version))"
