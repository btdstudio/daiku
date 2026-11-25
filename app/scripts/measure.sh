#!/usr/bin/env bash
# Measures the app's cold start, bundle size and the watcher delay (S-0017, AC9).
#
#   npm run measure -- <path to a daiku workspace>
#
# Reads the workspace; every `daiku gate` runs on a temporary copy of it.
# Needs `daiku` on PATH (or DAIKU_BIN). Takes a few minutes: it builds the app twice.
set -euo pipefail

if [ $# -ne 1 ] || [ ! -d "$1/.daiku" ]; then
  echo "usage: measure.sh <path to a daiku workspace>" >&2
  exit 2
fi
workspace=$(cd "$1" && pwd)
daiku_bin=${DAIKU_BIN:-daiku}
app_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
bundle=$app_dir/src-tauri/target/release/bundle/macos/daiku.app
binary=$bundle/Contents/MacOS/daiku-app
cold_runs=5
gate_runs=10

now_ms() { perl -MTime::HiRes=time -e 'printf "%d\n", time*1000'; }

median() {
  sort -n | awk '{ a[NR] = $1 } END { if (NR % 2) print a[(NR + 1) / 2]; else print int((a[NR / 2] + a[NR / 2 + 1]) / 2) }'
}

cd "$app_dir"

echo "== build: --features measure =="
npm run tauri build -- --bundles app --features measure >/dev/null
size_measure_kb=$(du -sk "$bundle" | cut -f1)

echo "== cold start ($cold_runs runs) =="
cold=()
for i in $(seq "$cold_runs"); do
  t0=$(now_ms)
  line=$(DAIKU_APP_MEASURE_ROOT="$workspace" perl -e 'alarm 60; exec @ARGV' "$binary" | grep -m1 '^ready ')
  t1=${line#ready }
  cold+=("$((t1 - t0))")
  echo "run $i: $((t1 - t0)) ms"
  sleep 1
done
cold_median=$(printf '%s\n' "${cold[@]}" | median)

# Built last, so the default build is what stays in target/release/bundle/macos/.
echo "== build: default =="
npm run tauri build -- --bundles app >/dev/null
size_default_kb=$(du -sk "$bundle" | cut -f1)

echo "== watcher delay ($gate_runs gates, on a copy) =="
copy=$(mktemp -d)
probe_pid=
cleanup() {
  exec 9>&- 2>/dev/null || true # closing stdin ends the probe
  [ -n "$probe_pid" ] && kill "$probe_pid" 2>/dev/null || true
  rm -rf "$copy" "${probe_log:-}" "${probe_fifo:-}"
}
trap cleanup EXIT
cp -R "$workspace/." "$copy/"
cd "$copy"
# A plan change that only has an intake: gate it and reopen it, alternately.
change=$("$daiku_bin" switch --all | awk '$2 == "plan" && $3 == "intake" && $4 == "draft" { print $1; exit }' || true)
[ -n "$change" ] || change=2-workspace-setup-wizard
"$daiku_bin" switch "$change" >/dev/null

cd "$app_dir/src-tauri"
cargo build --release --example watch_probe 2>/dev/null
probe_log=$(mktemp)
# Hold the probe's stdin open through fd 9 until cleanup.
probe_fifo=$(mktemp -u)
mkfifo "$probe_fifo"
exec 9<>"$probe_fifo"
./target/release/examples/watch_probe "$copy" <&9 >"$probe_log" &
probe_pid=$!
for _ in $(seq 50); do grep -q '^ready ' "$probe_log" && break; sleep 0.2; done
sleep 1

cd "$copy"
delays=()
for i in $(seq "$gate_runs"); do
  before=$(grep -c '^event ' "$probe_log" || true)
  t0=$(now_ms)
  if [ $((i % 2)) -eq 1 ]; then
    "$daiku_bin" gate intake approved >/dev/null
  else
    "$daiku_bin" reopen intake --reason probe >/dev/null
  fi
  line=
  for _ in $(seq 100); do
    n=$(grep -c '^event ' "$probe_log" || true)
    if [ "$n" -gt "$before" ]; then
      line=$(grep '^event ' "$probe_log" | sed -n "$((before + 1))p")
      break
    fi
    sleep 0.05
  done
  [ -n "$line" ] || { echo "run $i: no event within 5 s" >&2; exit 1; }
  delay=$((${line#event } - t0))
  delays+=("$delay")
  echo "run $i: $delay ms"
  sleep 2
done
watch_median=$(printf '%s\n' "${delays[@]}" | median)

echo
echo "| What | Result | Target |"
echo "|---|---|---|"
echo "| Cold start, median of $cold_runs | ${cold_median} ms | 2000 ms |"
echo "| Watcher delay (from command start), median of $gate_runs | ${watch_median} ms | 1000 ms |"
echo "| .app size, measure build | $((size_measure_kb / 1024)) MB (${size_measure_kb} KB) | none |"
echo "| .app size, default build | $((size_default_kb / 1024)) MB (${size_default_kb} KB) | none |"
echo "cold runs (ms): ${cold[*]}"
echo "watcher runs (ms): ${delays[*]}"
