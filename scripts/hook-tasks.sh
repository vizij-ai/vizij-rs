#!/usr/bin/env bash
# Shared task runner for git hooks and npm scripts.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

TASK_LABEL="tasks"

set_label() {
  TASK_LABEL="$1"
}

log_step() {
  echo "[${TASK_LABEL}] $1"
}

run_cmd() {
  local description="$1"
  shift
  log_step "$description"
  "$@"
}

rust_fmt() {
  run_cmd "cargo fmt --all" cargo fmt --all
}

rust_fmt_check() {
  run_cmd "cargo fmt --all -- --check" cargo fmt --all -- --check
}

rust_clippy() {
  # Skip benches in automated flows; still cover libs/bins/examples/tests
  run_cmd "cargo clippy --workspace --all-features --bins --examples --tests -- -D warnings" \
    cargo clippy --workspace --all-features --bins --examples --tests -- -D warnings
}

rust_build() {
  run_cmd "cargo build --all-features --all-targets" cargo build --all-features --all-targets
}

rust_test() {
  # Avoid compiling/running benches to keep hook/CI fast
  run_cmd "cargo test --workspace --all-features" cargo test --workspace --all-features
}

rust_clean() {
  run_cmd "cargo clean" cargo clean
}

npm_lint() {
  run_cmd "pnpm -r --if-present run lint" pnpm -r --if-present run lint
}

npm_clean() {
  run_cmd "pnpm run clean" pnpm run clean
}

npm_build() {
  run_cmd "pnpm run build" pnpm run build
}

npm_test() {
  run_cmd "pnpm run test" pnpm run test
}

run_wasm_checks() {
  run_cmd "node scripts/build-animation-wasm.mjs" node scripts/build-animation-wasm.mjs
  run_cmd "node scripts/build-graph-wasm.mjs" node scripts/build-graph-wasm.mjs
  run_cmd "node scripts/build-orchestrator-wasm.mjs" node scripts/build-orchestrator-wasm.mjs

  (
    cd npm/@vizij/animation-wasm
    run_cmd "npm/@vizij/animation-wasm: npm ci" npm ci
    run_cmd "npm/@vizij/animation-wasm: npm run build" npm run build
    run_cmd "npm/@vizij/animation-wasm: npm pack --dry-run" npm pack --dry-run
  )

  (
    cd npm/@vizij/node-graph-wasm
    run_cmd "npm/@vizij/node-graph-wasm: npm ci" npm ci
    run_cmd "npm/@vizij/node-graph-wasm: npm run build" npm run build
    run_cmd "npm/@vizij/node-graph-wasm: npm pack --dry-run" npm pack --dry-run
  )

  (
    cd npm/@vizij/orchestrator-wasm
    run_cmd "npm/@vizij/orchestrator-wasm: npm ci" npm ci
    run_cmd "npm/@vizij/orchestrator-wasm: npm run build" npm run build
    run_cmd "npm/@vizij/orchestrator-wasm: npm pack --dry-run" npm pack --dry-run
  )
}

cmd_fmt_rust() {
  set_label "fmt:rust"
  rust_fmt
}

cmd_fmt_rust_check() {
  set_label "fmt:rust:check"
  rust_fmt_check
}

cmd_lint_rust() {
  set_label "lint:rust"
  rust_clippy
}

cmd_build_rust() {
  set_label "build:rust"
  rust_build
}

cmd_test_rust() {
  set_label "test:rust"
  rust_test
}

cmd_clean_rust() {
  set_label "clean:rust"
  rust_clean
}

cmd_lint_npm() {
  set_label "lint:npm"
  npm_lint
}

cmd_clean_npm() {
  set_label "clean:npm"
  npm_clean
}

cmd_build_npm() {
  set_label "build:npm"
  npm_build
}

cmd_test_npm() {
  set_label "test:npm"
  npm_test
}

cmd_format() {
  set_label "format"
  rust_fmt
  npm_lint
}

cmd_check_rust() {
  set_label "check:rust"
  rust_fmt_check
  rust_clippy
  rust_build
  rust_test
}

cmd_check_npm() {
  set_label "check:npm"
  npm_clean
  npm_build
  npm_test
}

cmd_check() {
  set_label "check"
  rust_clean
  rust_fmt_check
  rust_clippy
  rust_build
  rust_test
  npm_clean
  npm_build
  npm_test

  if [[ "${HOOK_RUN_WASM:-0}" == "1" ]]; then
    run_wasm_checks
  fi
}

cmd_pre_commit() {
  set_label "pre-commit"
  rust_fmt
  rust_clippy
  log_step "OK"
}

cmd_pre_push() {
  set_label "pre-push"

  if [[ "${SKIP_GIT_HOOKS:-0}" == "1" ]]; then
    log_step "SKIP_GIT_HOOKS=1 -> skipping checks"
    return 0
  fi

  rust_fmt_check
  rust_clippy
  rust_test
  run_cmd "verify node registry" pnpm --filter vizij-rs verify:registry

  if [[ "${HOOK_RUN_WASM:-0}" == "1" ]]; then
    run_wasm_checks
  fi

  log_step "OK"
}

usage() {
  cat <<'EOF'
Usage: scripts/hook-tasks.sh <command>

Commands:
  pre-commit        Run the pre-commit checks (fmt + clippy)
  pre-push          Run the pre-push checks (fmt --check, clippy, tests, optional WASM)
  fmt-rust          Format the Rust workspace
  fmt-rust-check    Check Rust formatting without writing changes
  lint-rust         Run clippy with warnings as errors
  build-rust        Build all Rust targets with --all-features
  test-rust         Run the Rust test suite with --all-features
  clean-rust        Clean Rust build artifacts
  lint-npm          Run lint across JS/TS packages (if lint scripts exist)
  clean-npm         Run pnpm clean scripts
  build-npm         Run pnpm build scripts
  test-npm          Run pnpm test scripts
  format            Run combined format routine (Rust fmt + npm lint)
  check-rust        Run the full Rust check pipeline
  check-npm         Run the full npm check pipeline
  check             Run the full project check (clean, fmt, lint, build, test)
  help              Show this help message
EOF
}

main() {
  if [[ $# -eq 0 ]]; then
    usage
    exit 1
  fi

  case "$1" in
    pre-commit) cmd_pre_commit ;;
    pre-push) cmd_pre_push ;;
    fmt-rust) cmd_fmt_rust ;;
    fmt-rust-check) cmd_fmt_rust_check ;;
    lint-rust) cmd_lint_rust ;;
    build-rust) cmd_build_rust ;;
    test-rust) cmd_test_rust ;;
    clean-rust) cmd_clean_rust ;;
    lint-npm) cmd_lint_npm ;;
    clean-npm) cmd_clean_npm ;;
    build-npm) cmd_build_npm ;;
    test-npm) cmd_test_npm ;;
    format) cmd_format ;;
    check-rust) cmd_check_rust ;;
    check-npm) cmd_check_npm ;;
    check) cmd_check ;;
    help|-h|--help) usage ;;
    *)
      echo "Unknown command: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
}

main "$@"
