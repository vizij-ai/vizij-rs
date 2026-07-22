#!/usr/bin/env bash
set -euo pipefail

# Orchestrate testing suite:
#  1) Core native tests
#  2) Ensure wasm32 target is installed
#  3) WASM tests (Node) via wasm-bindgen-test
#
# Usage:
#   bash ./animation/run-testing-suite.sh
#
# Prereqs for WASM:
#   rustup target add wasm32-unknown-unknown
#   wasm-pack installed (cargo install wasm-pack) or available via npx

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[1/3] Running vizij-animation-core tests (native)..."
cargo test -p vizij-animation-core

echo "[2/3] Ensuring wasm32 target is installed..."
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
  rustup target add wasm32-unknown-unknown
fi

echo "[3/3] Running vizij-animation-wasm tests (Node via wasm-pack)..."
cd "$ROOT_DIR/animation/vizij-animation-wasm"
if command -v wasm-pack >/dev/null 2>&1; then
  wasm-pack test --node
else
  npx --yes wasm-pack test --node
fi

echo "Testing suite completed successfully."
