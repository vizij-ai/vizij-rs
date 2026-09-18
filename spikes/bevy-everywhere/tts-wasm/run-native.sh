#!/bin/zsh
export PATH="$HOME/.cargo/bin:$PATH"
cd /private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/tts-wasm
export CARGO_TARGET_DIR=/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/arora-wasm/target-host
LOG=logs/test-native-5.log
{
  echo "CMD: CARGO_TARGET_DIR=$CARGO_TARGET_DIR cargo test -- --nocapture"
  echo "START $(date +%H:%M:%S)"
  S=$(date +%s)
  cargo test -- --nocapture 2>&1
  echo "EXIT $?"
  echo "END $(date +%H:%M:%S) duration=$(( $(date +%s) - S ))s"
} > $LOG 2>&1
