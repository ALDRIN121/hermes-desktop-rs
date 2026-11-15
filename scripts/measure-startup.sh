#!/usr/bin/env bash
# Cold/warm startup and steady-state memory for the Rust shell.
#
# Measures the real bundled app, not a mock: launch → first paint (the window
# server reporting a non-empty root) → settled. RSS is sampled from the process
# tree (WebContent included), since the WebView's renderer is a separate pid on
# macOS and a shell-only number would understate the footprint.
#
#   scripts/measure-startup.sh [runs] [route]
#
# Writes docs/implementation/perf-baseline.json.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNS="${1:-3}"
ROUTE="${2:-/}"
APP="$ROOT/target/fixtures/Hermes UI Fixture.app"
BIN="$APP/Contents/MacOS/hermes-desktop-rust"
OUT="$ROOT/docs/implementation/perf-baseline.json"

if [[ ! -x "$BIN" ]]; then
  echo "measure: $BIN missing — bundle the fixture app first" >&2
  exit 1
fi

home_for() {
  local base
  base="$(mktemp -d "${TMPDIR:-/tmp}/hermes-perf-XXXXXX")"
  mkdir -p "$base/logs"
  printf '{"revision":0,"values":{}}' > "$base/desktop-ui.json"
  echo "$base"
}

# Every process the app owns: the shell and its WebKit helper processes.
tree_rss_kb() {
  local pid="$1" total=0 child
  for child in $(pgrep -P "$pid" 2>/dev/null || true) "$pid"; do
    local rss
    rss="$(ps -o rss= -p "$child" 2>/dev/null | tr -d ' ')"
    [[ -n "$rss" ]] && total=$((total + rss))
    for grandchild in $(pgrep -P "$child" 2>/dev/null || true); do
      rss="$(ps -o rss= -p "$grandchild" 2>/dev/null | tr -d ' ')"
      [[ -n "$rss" ]] && total=$((total + rss))
    done
  done
  echo "$total"
}

run_once() {
  local label="$1" home pid started first_paint settled rss
  home="$(home_for)"
  started="$(python3 -c 'import time; print(int(time.time()*1000))')"
  rm -f "${TMPDIR:-/tmp}/hermes-rust-first-paint.json"
  HERMES_MEASURE_STARTUP=1 HERMES_HOME="$home" HERMES_SHELL_TEST_INSTANCE="perf-$label" \
    "$BIN" --route "$ROUTE" >/dev/null 2>&1 &
  pid=$!

  # The shell stamps its own first paint; polling an outside accessibility
  # probe would need permissions and would time the wrong thing anyway.
  first_paint=""
  for _ in $(seq 1 300); do
    sleep 0.05
    stamp="${TMPDIR:-/tmp}/hermes-rust-first-paint.json"
    if [[ -s "$stamp" ]]; then
      first_paint="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["firstPaintMs"])' "$stamp" 2>/dev/null || true)"
      [[ -n "$first_paint" ]] && break
    fi
    kill -0 "$pid" 2>/dev/null || break
  done
  sleep 4
  settled="$(python3 -c 'import time; print(int(time.time()*1000))')"
  rss="$(tree_rss_kb "$pid")"

  kill "$pid" 2>/dev/null || true
  sleep 1
  rm -rf "$home"

  python3 - "$label" "$started" "${first_paint:-0}" "$settled" "$rss" <<'PY'
import json, sys
label, started, first, settled, rss = sys.argv[1:6]
print(json.dumps({
    "run": label,
    "firstPaintMs": int(first) if int(first) else None,
    "settledMs": int(settled) - int(started),
    "rssMb": round(int(rss) / 1024, 1),
}))
PY
}

echo "measure: $RUNS cold runs on route $ROUTE"
rows=()
for i in $(seq 1 "$RUNS"); do
  row="$(run_once "$i")"
  rows+=("$row")
  echo "  run $i: $row"
done

python3 - "$OUT" "${rows[@]}" <<'PY'
import json, sys
out, rows = sys.argv[1], [json.loads(r) for r in sys.argv[2:]]
paints = [r["firstPaintMs"] for r in rows if r["firstPaintMs"]]
rss = [r["rssMb"] for r in rows]
report = {
    "measured": "bundled fixture app, debug profile, isolated HERMES_HOME",
    "runs": rows,
    "summary": {
        "firstPaintMsMedian": sorted(paints)[len(paints) // 2] if paints else None,
        "rssMbMax": max(rss) if rss else None,
        "rssMbMedian": sorted(rss)[len(rss) // 2] if rss else None,
    },
}
with open(out, "w") as handle:
    json.dump(report, handle, indent=2)
    handle.write("\n")
print(json.dumps(report["summary"], indent=2))
PY
echo "measure: wrote $OUT"
