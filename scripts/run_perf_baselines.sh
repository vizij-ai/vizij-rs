#!/usr/bin/env bash
set -euo pipefail

# Runs Stage 1 native Criterion benches and appends a summary row per scenario
# to vizij_docs/current_documentation/perf_baselines.md.
# Parsing is best-effort: it extracts the median estimate and unit from Criterion output.

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${ROOT}/../vizij_docs/current_documentation/perf_baselines.md"

ensure_table() {
  if [[ ! -f "$OUT" ]]; then
    cat >"$OUT" <<'EOF'
# Performance Baselines
<!-- tags: status=tracking; topics=performance,benchmarking -->

Run the Stage 1 native benches and append results here for quick comparison across revisions. The helper script below appends rows automatically; otherwise copy/paste the bench output into the table.

## How to run
```bash
# from repo root
bash vizij-rs/scripts/run_perf_baselines.sh
```

## Results (append-only)
| date | crate | bench | scenario | est | unit |
| --- | --- | --- | --- | --- | --- |
EOF
  fi
}

append_rows() {
  local log_file="$1"
  local crate="$2"
  local bench="$3"

  python3 - "$log_file" "$crate" "$bench" "$OUT" <<'PY'
import sys, re, datetime, pathlib
log_path, crate, bench, out_path = sys.argv[1:]
lines = pathlib.Path(log_path).read_text().splitlines()

rows = []
scenario = None
# Accept units like ns, us, µs, ms, s
time_re = re.compile(r"time:\s*\[\s*([\d\.]+)\s+([^\s]+)\s+([\d\.]+)\s+[^\s]+\s+([\d\.]+)\s+[^\s]+\s*\]")

def shorten(name: str) -> str:
    parts = name.split('/')
    if len(parts) >= 2:
        return '/'.join(parts[-2:])
    return name

def to_micros(val: float, unit: str) -> float:
    u = unit.replace("µ", "u")  # normalize micro symbol
    if u == "ns":
        return val / 1_000.0
    if u == "us":
        return val
    if u == "ms":
        return val * 1_000.0
    if u == "s":
        return val * 1_000_000.0
    return val

for line in lines:
    stripped = line.strip()
    if stripped and "time:" not in stripped and not stripped.startswith("Found"):
        scenario = stripped
    m = time_re.search(stripped)
    if m and scenario:
        est = float(m.group(3))
        unit = m.group(2)
        micros = to_micros(est, unit)
        rows.append((shorten(scenario), micros))
        scenario = None

if not rows:
    sys.exit(0)

dt = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")
out = pathlib.Path(out_path)
with out.open("a") as f:
    for scenario, micros in rows:
        f.write(f"| {dt} | {crate} | {bench} | {scenario} | {micros:,.3f} | µs |\n")
PY
}

run_bench() {
  local crate="$1"
  local bench="$2"
  local log
  log="$(mktemp)"
  echo "==> Running ${crate} :: ${bench}"
  (cd "$ROOT" && cargo bench -p "$crate" --bench "$bench" -- --noplot --quiet | tee "$log")
  append_rows "$log" "$crate" "$bench"
  rm -f "$log"
}

ensure_table

run_bench vizij-graph-core graph_eval
run_bench vizij-animation-core animation_step
run_bench vizij-orchestrator-core orchestrator_tick

echo "Done. Results appended to $OUT"
