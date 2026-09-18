#!/bin/sh
# Builds the spike for wasm32 (reusing the bevy-wasm spike's target dir) and
# runs wasm-bindgen; prints timings and sizes. `./build.sh picking` enables the
# picking feature and writes to pkg-picking/.
set -e
SPIKE=$(cd "$(dirname "$0")" && pwd)
BW="$SPIKE/../bevy-wasm"
export CARGO_TARGET_DIR="$BW/target"
export PATH="$HOME/.cargo/bin:$PATH"
set -o pipefail
cd "$SPIKE"
FEATURES=""; OUT=pkg
if [ "$1" = "picking" ]; then FEATURES="--features picking"; OUT=pkg-picking; fi
echo "start $(date)"
echo "== cargo build (release, wasm32-unknown-unknown) $FEATURES"
time cargo build --release --target wasm32-unknown-unknown --lib $FEATURES 2>&1 | grep -v "^\s*Compiling" | tail -60
WASM="$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/challenge_wasm.wasm"
ls -l "$WASM"
echo "== wasm-bindgen --target web -> $OUT"
time "$BW/tools/bin/wasm-bindgen" --target web --out-dir "$OUT" --no-typescript "$WASM"
ls -l "$OUT"/
gzip -9 -k -f "$OUT"/challenge_wasm_bg.wasm && ls -l "$OUT"/challenge_wasm_bg.wasm.gz
echo "end $(date)"
