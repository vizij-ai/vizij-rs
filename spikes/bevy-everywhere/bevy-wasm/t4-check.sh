#!/bin/sh
# T4: cargo check of vizij-rs crates for wasm32-unknown-unknown (read-only; the
# target dir lives in the spike). One log per crate, plus an error summary.
SPIKE=$(cd "$(dirname "$0")" && pwd)
export CARGO_TARGET_DIR="$SPIKE/target-t4"
cd /Users/victor.paleologue/Code/Semio/vizij-rs
for c in vizij-arora-store vizij-arora-hal vizij-arora-behavior vizij-arora-host vizij; do
  echo "== cargo check -p $c --target wasm32-unknown-unknown --locked"
  start=$(date +%s)
  cargo check -p "$c" --target wasm32-unknown-unknown --locked > "$SPIKE/logs/t4-$c.log" 2>&1
  code=$?
  echo "   exit $code in $(( $(date +%s) - start )) s; errors: $(grep -c '^error' "$SPIKE/logs/t4-$c.log"); failing crates: $(grep -E '^error: could not compile' "$SPIKE/logs/t4-$c.log" | sed -E 's/.*compile `([^`]*)`.*/\1/' | sort -u | tr '\n' ' ')"
done
git -C /Users/victor.paleologue/Code/Semio/vizij-rs status --short | head -5
