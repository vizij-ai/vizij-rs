#!/bin/sh
# Builds the spike for wasm32 and runs wasm-bindgen; prints the timings and sizes.
set -e
SPIKE=$(cd "$(dirname "$0")" && pwd)
export CARGO_TARGET_DIR="$SPIKE/target"
cd "$SPIKE"
echo "== cargo build (release, wasm32-unknown-unknown)"
time cargo build --release --target wasm32-unknown-unknown --lib 2>&1 | tail -5
WASM="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/bevy_wasm_spike.wasm"
ls -l "$WASM"
echo "== wasm-bindgen --target web"
time ./tools/bin/wasm-bindgen --target web --out-dir pkg --no-typescript "$WASM"
ls -l pkg/
gzip -9 -k -f pkg/bevy_wasm_spike_bg.wasm && ls -l pkg/bevy_wasm_spike_bg.wasm.gz
