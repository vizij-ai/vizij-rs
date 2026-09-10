#!/bin/bash
# The viseme feedback stream, end to end, from a native ROS 2 client:
#
#   router  — rmw_zenohd in the container, published on ROUTER_PORT
#   device  — this crate's binary (a `ros2-zenoh` build), --ros2 on its own
#             namespace, joining the router in Zenoh client mode
#   client  — `ros2 action send_goal /skill/say … --feedback` in the container,
#             speaking communication_skills/Say from the vendored definitions
#
# Passes when the goal is accepted, the feedback stream carries at least three
# distinct viseme shapes with a non-zero intensity, rest (`sil`) reports zero,
# and the goal ends SUCCEEDED. Needs docker and the image build-image.sh
# builds (or ROS2_ZENOH_IMAGE), and `cargo build -p vizij --features
# ros2-zenoh` done first (or VIZIJ_BIN).
#
# Usage: say-feedback.sh [text]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
IMAGE="${ROS2_ZENOH_IMAGE:-vizij-ros4hri:jazzy}"
BIN="${VIZIJ_BIN:-$ROOT/target/debug/vizij}"
GLB="${VIZIJ_GLB:-$ROOT/../vizij-web/Quori_Latest_ROS.glb}"
NS="${VIZIJ_NAMESPACE:-vizij_say_e2e}"
ROUTER_PORT="${ROUTER_PORT:-7451}"
ROUTER="vizij-say-e2e-$$"
TEXT="${1:-Hello, I am a talking face and my lips follow my words.}"
OUT="${OUT_DIR:-$(mktemp -d)}"; mkdir -p "$OUT"
ROS='source /opt/ros/jazzy/setup.bash && source /ros4hri_ws/install/setup.bash'

log() { echo "[say-e2e] $*" >&2; }
[ -x "$BIN" ] || { log "no device binary at $BIN — cargo build -p vizij --features ros2-zenoh"; exit 2; }
[ -f "$GLB" ] || { log "no face at $GLB (VIZIJ_GLB)"; exit 2; }

docker run -d --name "$ROUTER" -p "$ROUTER_PORT:7447" "$IMAGE" \
  bash -lc 'source /opt/ros/jazzy/setup.bash && exec ros2 run rmw_zenoh_cpp rmw_zenohd' >/dev/null
DEVICE=""
teardown() {
  if [ -n "$DEVICE" ]; then kill "$DEVICE" 2>/dev/null || true; wait "$DEVICE" 2>/dev/null || true; fi
  docker rm -f "$ROUTER" >/dev/null 2>&1 || log "could not remove the router container $ROUTER"
}
trap teardown EXIT
for i in $(seq 1 30); do docker logs "$ROUTER" 2>&1 | grep -q "Started Zenoh router" && break; sleep 1; done
log "router up on :$ROUTER_PORT"

export ZENOH_CONFIG_OVERRIDE="mode=\"client\";connect/endpoints=[\"tcp/127.0.0.1:$ROUTER_PORT\"]"
export RUST_LOG="${RUST_LOG:-info}"
"$BIN" --glb "$GLB" --headless --ros2 "$NS" --frame-rate 4 --no-autoplay > "$OUT/device.log" 2>&1 &
DEVICE=$!

# Discovery settles a few seconds after the bridge starts: poll for the skill.
for i in $(seq 1 24); do
  docker exec "$ROUTER" bash -lc "$ROS && ros2 action list -t" > "$OUT/actions.txt" 2>/dev/null || true
  grep -q "/skill/say \[communication_skills/action/Say\]" "$OUT/actions.txt" && break
  sleep 5
done
grep -q "/skill/say \[communication_skills/action/Say\]" "$OUT/actions.txt" || {
  log "FAIL: /skill/say is not served as communication_skills/action/Say"; cat "$OUT/actions.txt" >&2; exit 1; }
log "/skill/say is on the graph"

docker exec "$ROUTER" bash -lc "$ROS && timeout 90 ros2 action send_goal /skill/say communication_skills/action/Say \"{meta: {caller: 'say-e2e', priority: 128}, input: '$TEXT'}\" --feedback" \
  > "$OUT/goal.txt" 2>&1 || true

# The verdict, from the client's own transcript.
pairs() { paste -d' ' <(grep -E '^\s*viseme:' "$OUT/goal.txt" | awk '{print $2}') <(grep -E '^\s*intensity:' "$OUT/goal.txt" | awk '{print $2}'); }
SHAPES=$(pairs | awk '{print $1}' | uniq | tr '\n' ' ')
DISTINCT=$(pairs | awk '{print $1}' | sort -u | wc -l | tr -d ' ')
DRIVEN=$(pairs | awk '$1 != "sil" && $2 > 0.5' | wc -l | tr -d ' ')
SILENT_DRIVEN=$(pairs | awk '$1 == "sil" && $2 > 0' | wc -l | tr -d ' ')
log "visemes fed back: $SHAPES"
grep -q "Goal accepted" "$OUT/goal.txt" || { log "FAIL: the goal was not accepted (see $OUT/goal.txt, $OUT/device.log)"; exit 1; }
grep -q "Goal finished with status: SUCCEEDED" "$OUT/goal.txt" || { log "FAIL: the goal did not succeed (see $OUT/goal.txt)"; exit 1; }
[ "$DISTINCT" -ge 3 ] || { log "FAIL: $DISTINCT distinct visemes fed back, expected at least 3"; exit 1; }
[ "$DRIVEN" -ge 1 ] || { log "FAIL: no shape was fed back with intensity"; exit 1; }
[ "$SILENT_DRIVEN" -eq 0 ] || { log "FAIL: $SILENT_DRIVEN feedback frames report sil with a non-zero intensity"; exit 1; }
log "PASS — $DISTINCT distinct visemes, $DRIVEN driven frames, SUCCEEDED (transcript: $OUT/goal.txt)"
