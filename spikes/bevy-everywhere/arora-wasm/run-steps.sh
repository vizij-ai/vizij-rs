#!/bin/zsh
# Runs the wasm32 compile spike step by step; one log per step, a summary at the end.
SP=/private/tmp/claude-501/-Users-victor-paleologue-Code-Semio-semio-studio/3db729c2-88a1-4330-986e-37ae26376064/scratchpad/spikes/arora-wasm
export CARGO_TARGET_DIR=$SP/target
cd $SP
: > logs/summary.txt
for step in step1 step1-urdf step2 step3 step3-frames step4 step5 step6 step7 step8-studio step9-native; do
  log=logs/$step.log
  start=$(date +%s)
  cargo check --target wasm32-unknown-unknown --features "$step" > "$log" 2>&1
  code=$?
  end=$(date +%s)
  echo "EXIT $code" >> "$log"
  first_error=$(grep -m1 -E '^error' "$log" | cut -c1-160)
  echo "$step: exit=$code time=$((end-start))s first_error=${first_error:-none}" | tee -a logs/summary.txt
done
echo "ALL DONE"
