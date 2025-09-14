#!/usr/bin/env bash
set -euo pipefail

# Run from the root of vizij-rs
REPO_ROOT=$(git rev-parse --show-toplevel)
cd "$REPO_ROOT"

# 1. Ensure the workspace builds and tests
cargo build --workspace
cargo test --workspace

# 2. Build WASM artifacts (assumes wasm-pack or equivalent is installed)
echo "Building WASM packages…"
node scripts/build-animation-wasm.mjs
node scripts/build-graph-wasm.mjs

# 3. Dry‑run publishing of crates in dependency order
CRATES=(
  vizij-animation-core
  vizij-graph-core
  bevy_vizij_animation
  bevy_vizij_graph
  vizij-animation-wasm
  vizij-graph-wasm
)

for crate in "${CRATES[@]}"; do
  echo "\n=== Dry running $crate ==="
  cargo publish --dry-run -p "$crate"
done

# 4. Dry‑run publishing of npm packages in vizij-rs
for pkg in animation-wasm node-graph-wasm; do
  echo "\n=== Dry running @vizij/$pkg ==="
  pushd npm/@vizij/$pkg > /dev/null
  npm i
  npm run build
  npm pack --dry-run
  popd > /dev/null
done

# 5. Dry‑run publishing of npm packages in vizij-web
cd ../vizij-web
for workspace in packages/@vizij/animation-react packages/@vizij/node-graph-react packages/render packages/utils; do
  pkg_name=$(jq -r .name < $workspace/package.json)
  echo "\n=== Dry running $pkg_name ==="
  npm i
  npm run --workspace $pkg_name build || npm run build
  npm pack --dry-run --workspace $pkg_name
done

echo "\nDry run complete.  All artifacts built successfully."
