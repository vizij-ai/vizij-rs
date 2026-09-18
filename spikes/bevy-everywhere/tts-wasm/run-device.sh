#!/bin/zsh
export PATH="$HOME/.cargo/bin:$PATH"
cd /Users/victor.paleologue/Code/Semio/vizij-rs
export CARGO_TARGET_DIR=/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/tts-wasm/target
LOG=/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/tts-wasm/logs/test-vizij-device-native-3.log
{
  echo "CMD: (in vizij-rs $(git rev-parse --short HEAD), $(git status --short | wc -l | tr -d ' ') dirty paths) CARGO_TARGET_DIR=$CARGO_TARGET_DIR cargo test --locked -p vizij --bin vizij -- device::tests::a_play_viseme_run_plays_the_shape_through_its_envelope device::tests::a_new_play_viseme_run_takes_over_and_crossfades device::tests::a_say_run_streams_the_provider_s_visemes_to_the_lips"
  echo "START $(date +%H:%M:%S)"
  S=$(date +%s)
  cargo test --locked -p vizij --bin vizij -- device::tests::a_play_viseme_run_plays_the_shape_through_its_envelope device::tests::a_new_play_viseme_run_takes_over_and_crossfades device::tests::a_say_run_streams_the_provider_s_visemes_to_the_lips 2>&1
  echo "EXIT $?"
  echo "END $(date +%H:%M:%S) duration=$(( $(date +%s) - S ))s"
} > $LOG 2>&1
