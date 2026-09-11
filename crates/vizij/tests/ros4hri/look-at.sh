#!/bin/bash
# The gaze skill's goal lifecycle, end to end, from a native ROS 2 client:
#
#   router  — rmw_zenohd in the container, published on ROUTER_PORT
#   device  — this crate's binary (a `ros2-zenoh` build), --ros2 on its own
#             namespace, joining the router in Zenoh client mode
#   client  — `ros2 action send_goal /skill/look_at …` in the container,
#             speaking interaction_skills/LookAt from the vendored definitions
#
# Exercises the standard's one-active-goal rule in the order a caller meets it:
#   1. track A      — accepted, keeps running (the tracked target moves the gaze)
#   2. track B      — same priority: accepted, and A ends ROS_EINTR (4)
#   3. low priority — rejected while B is active
#   4. glance       — same priority: accepted, B ends ROS_EINTR, glance SUCCEEDED
#   5. reset        — SUCCEEDED
# Passes when every transcript reads as above. Needs docker and the image
# build-image.sh builds (or ROS2_ZENOH_IMAGE), and `cargo build -p vizij
# --features ros2-zenoh` done first (or VIZIJ_BIN).
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
IMAGE="${ROS2_ZENOH_IMAGE:-vizij-ros4hri:jazzy}"
BIN="${VIZIJ_BIN:-$ROOT/target/debug/vizij}"
GLB="${VIZIJ_GLB:-$ROOT/../vizij-web/Quori_Latest_ROS.glb}"
NS="${VIZIJ_NAMESPACE:-vizij_look_at_e2e}"
ROUTER_PORT="${ROUTER_PORT:-7452}"
ROUTER="vizij-look-at-e2e-$$"
OUT="${OUT_DIR:-$(mktemp -d)}"; mkdir -p "$OUT"
ROS='source /opt/ros/jazzy/setup.bash && source /ros4hri_ws/install/setup.bash'
ACTION="/skill/look_at interaction_skills/action/LookAt"

log() { echo "[look-at-e2e] $*" >&2; }
fail() { log "FAIL: $*"; log "transcripts in $OUT (device.log, *.txt)"; exit 1; }
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
  grep -q "/skill/look_at \[interaction_skills/action/LookAt\]" "$OUT/actions.txt" && break
  sleep 5
done
grep -q "/skill/look_at \[interaction_skills/action/LookAt\]" "$OUT/actions.txt" || {
  cat "$OUT/actions.txt" >&2; fail "/skill/look_at is not served as interaction_skills/action/LookAt"; }
log "/skill/look_at is on the graph"

# A goal, sent from the container; the transcript is the client's own output.
# `send_goal` blocks until the goal ends, so a goal that keeps running (track)
# is sent in the background and its transcript read as the next goal lands.
goal() { # name, priority, policy, x, y, z
  local name="$1" priority="$2" policy="$3" x="$4" y="$5" z="$6"
  docker exec "$ROUTER" bash -lc "$ROS && timeout 60 ros2 action send_goal $ACTION \
    \"{meta: {caller: 'look-at-e2e', priority: $priority}, policy: '$policy', target: {header: {frame_id: face}, point: {x: $x, y: $y, z: $z}}}\"" \
    > "$OUT/$name.txt" 2>&1 || true
}
await() { # name, pattern — poll a transcript for a line
  local name="$1" pattern="$2"
  for i in $(seq 1 60); do grep -q "$pattern" "$OUT/$name.txt" 2>/dev/null && return 0; sleep 0.5; done
  return 1
}
result_code() { grep -E '^\s*error_code:' "$OUT/$1.txt" | awk '{print $2}' | tail -1; }

# 1. Track A: accepted, and it stays active (no result while tracking).
goal track-a 128 '' 1.0 0.3 0.1 &
await track-a "Goal accepted" || fail "track A was not accepted (device.log, track-a.txt)"
sleep 2
grep -q "Goal finished" "$OUT/track-a.txt" && fail "track A ended on its own; a tracking goal runs until replaced"
log "track A accepted and running"

# 2. Track B at the same priority replaces A: B accepted, A ends ROS_EINTR.
goal track-b 128 '' -1.0 0.3 0.1 &
await track-b "Goal accepted" || fail "track B (same priority) was not accepted"
await track-a "Goal finished" || fail "track A did not end when B replaced it"
[ "$(result_code track-a)" = "4" ] || fail "track A ended with error_code $(result_code track-a), expected ROS_EINTR (4)"
log "track B replaced A: A ended ROS_EINTR"

# 3. A lower-priority goal is rejected while B is active.
goal low 1 '' 0.0 0.0 1.0
grep -q "Goal was rejected" "$OUT/low.txt" || fail "a priority-1 goal was not rejected under an active priority-128 goal"
grep -q "Goal finished" "$OUT/track-b.txt" && fail "track B ended on the rejected goal"
log "lower priority rejected, B still active"

# 4. Glance at the same priority replaces B and succeeds once the gaze settles.
goal glance 128 glance 0.5 -0.4 0.2
await track-b "Goal finished" || fail "track B did not end when the glance replaced it"
[ "$(result_code track-b)" = "4" ] || fail "track B ended with error_code $(result_code track-b), expected ROS_EINTR (4)"
grep -q "Goal finished with status: SUCCEEDED" "$OUT/glance.txt" || fail "the glance did not succeed"
[ "$(result_code glance)" = "0" ] || fail "the glance succeeded with error_code $(result_code glance), expected ROS_ENOERR (0)"
log "glance replaced B and SUCCEEDED"

# 5. Reset the gaze.
goal reset 128 reset 0.0 0.0 0.0
grep -q "Goal finished with status: SUCCEEDED" "$OUT/reset.txt" || fail "the reset did not succeed"
log "PASS — track, replace (ROS_EINTR), reject, glance, reset (transcripts: $OUT)"
