#!/bin/zsh
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/tts-wasm
export CARGO_TARGET_DIR=/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/arora-wasm/target
DRIVER=$HOME/Library/Caches/.wasm-pack/chromedriver-f07dc4f5987c4c26/chromedriver
LOG=logs/wasm-pack-test-chrome-5.log
{
  echo "CMD: CARGO_TARGET_DIR=$CARGO_TARGET_DIR wasm-pack test --headless --chrome --chromedriver $DRIVER -- --test browser -- --nocapture"
  echo "webdriver.json: $(cat webdriver.json | tr -d '\n')"
  echo "chromedriver: $($DRIVER --version)"
  echo "chrome: $('/Users/victor.paleologue/Library/Caches/ms-playwright/chromium-1228/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing' --version)"
  echo "wasm-pack: $(wasm-pack --version)"
  echo "START $(date +%H:%M:%S)"
  S=$(date +%s)
  wasm-pack test --headless --chrome --chromedriver "$DRIVER" -- --test browser -- --nocapture 2>&1
  echo "EXIT $?"
  echo "END $(date +%H:%M:%S) duration=$(( $(date +%s) - S ))s"
} > $LOG 2>&1
