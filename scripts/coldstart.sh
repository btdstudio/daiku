#!/bin/sh
# coldstart.sh — the optional cold-start check, run by hand before tagging a
# release.
#
# It checks that an agent with no prior context can find its way in a fresh
# daiku workspace from each of two opening prompts. It works with any agent:
# it sets up each workspace, prints the prompt, waits while you run it, then
# checks the result on disk.
#
#   sh scripts/coldstart.sh
#
# DAIKU=/path/to/daiku  use that binary instead of building ./cmd/daiku
# COLDSTART_KEEP=1      keep the temp workspaces afterwards
#
# It writes only under one temp folder, runs no agent, and never pushes, tags
# or publishes. Exit 0 means both prompts passed.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
base=${TMPDIR:-/tmp}
tmp=$(mktemp -d "${base%/}/daiku-coldstart.XXXXXX")
cleanup() {
	if [ "${COLDSTART_KEEP:-}" = 1 ]; then
		echo "kept: $tmp"
	else
		rm -rf "$tmp"
	fi
}
trap cleanup EXIT
trap 'exit 130' INT TERM

# The daiku under test always lives in $bin, so there's one folder to put on
# the agent's PATH: the agent must run this binary, not an older install.
bin=$tmp/bin
mkdir -p "$bin"
daiku=$bin/daiku
if [ -n "${DAIKU:-}" ]; then
	cp "$DAIKU" "$daiku"
else
	(cd "$here" && GOWORK=off go build -o "$daiku" ./cmd/daiku)
fi
echo "under test: $("$daiku" version) ($daiku)"
if [ "$(command -v daiku 2>/dev/null || true)" != "$daiku" ]; then
	echo "note: the daiku on your PATH isn't this one — put it first in the agent's shell, as shown below"
fi
# Keep the person's own daiku settings out of the fresh workspaces.
XDG_CONFIG_HOME=$tmp/config
export XDG_CONFIG_HOME

failed=0

# ask prints the prompt to give the agent and waits for Enter.
ask() {
	scenario=$1 ws=$2 prompt=$3
	echo
	echo "=== $scenario ==="
	echo "Start an agent in this folder, with no other context:"
	echo
	echo "workspace: $ws"
	echo
	echo "In the agent's shell, first put the daiku under test on PATH:"
	echo
	echo "    export PATH=\"$bin:\$PATH\""
	echo
	echo "and give it this prompt, word for word:"
	echo
	echo "    $prompt"
	echo
	echo "Let it work until it stops, then press Enter here."
	echo "==="
	read -r _ || true
}

pass() { echo "PASS $1"; }
fail() { echo "FAIL $1: $2"; failed=1; }

# 1. build: a repo and a ticket in the inbox; the agent should start a build.
ws=$tmp/build-ws
"$daiku" init "$ws" >/dev/null
git init -q "$tmp/demo"
(
	cd "$ws"
	"$daiku" repo add demo https://github.com/example/demo --path "$tmp/demo" >/dev/null
	"$daiku" tickets new task "Add a /health endpoint" --repo demo --inbox >/dev/null
)
ask build "$ws" "I have a ticket, help me build it"
if [ -d "$ws/build/demo/changes/N-0001" ] && grep -q '"N-0001"' "$ws/STATE.json" 2>/dev/null; then
	pass build
else
	fail build "no build change for ticket N-0001 (expected build/demo/changes/N-0001/ and a STATE.json record)"
fi

# 2. plan: an empty workspace; the agent should start a plan change.
ws=$tmp/plan-ws
"$daiku" init "$ws" >/dev/null
ask plan "$ws" "Help me plan a password reset flow"
if grep -q '"loop": "plan"' "$ws/STATE.json" 2>/dev/null; then
	pass plan
else
	fail plan "no plan change (expected one in STATE.json)"
fi

echo
if [ "$failed" = 0 ]; then
	echo "cold start: both prompts passed"
else
	echo "cold start: failed"
fi
exit "$failed"
