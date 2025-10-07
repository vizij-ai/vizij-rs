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
node scripts/build-orchestrator-wasm.mjs

# 3. Dry‑run publishing of crates in dependency order
CRATES=(
  vizij-animation-core
  vizij-graph-core
  vizij-animation-wasm
  vizij-graph-wasm
  vizij-orchestrator-wasm
  bevy_vizij_animation
  bevy_vizij_graph
)

for crate in "${CRATES[@]}"; do
  printf "\n=== Dry running %s ===\n" "$crate"
  cargo publish --dry-run -p "$crate"
done

# 4. Dry‑run publishing of npm packages in vizij-rs (supporting + wasm wrappers)
NPM_PACKAGES=(
  value-json
  wasm-loader
  animation-wasm
  node-graph-wasm
  orchestrator-wasm
)

for pkg in "${NPM_PACKAGES[@]}"; do
  printf "\n=== Dry running @vizij/%s ===\n" "$pkg"
  pushd npm/@vizij/$pkg > /dev/null
  if [[ -f package-lock.json ]]; then
    npm ci
  else
    npm install --no-save
  fi
  npm run build
  npm pack --dry-run
  popd > /dev/null
done

# 5. Dry‑run publishing of npm packages in vizij-web
cd ../vizij-web
for workspace in \
  packages/@vizij/animation-react \
  packages/@vizij/node-graph-react \
  packages/@vizij/orchestrator-react \
  packages/@vizij/config \
  packages/@vizij/rig \
  packages/render \
  packages/utils; do
  pkg_name=$(jq -r .name < $workspace/package.json)
  printf "\n=== Dry running %s ===\n" "$pkg_name"
  npm i
  npm run --workspace $pkg_name build || npm run build
  npm pack --dry-run --workspace $pkg_name
done

echo "\nDry run complete.  All artifacts built successfully."
