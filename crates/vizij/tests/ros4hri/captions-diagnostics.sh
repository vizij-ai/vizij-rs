#!/bin/bash
# Closed captions in and diagnostics out, end to end, from a native ROS 2
# client:
#
#   router  — rmw_zenohd in the container, published on ROUTER_PORT
#   device  — this crate's binary (a `ros2-zenoh` build), --ros2 on its own
#             namespace, joining the router in Zenoh client mode, its local
#             bridge on LOCAL_PORT
#   client  — an rclpy node in the container publishing an
#             hri_actions_msgs/ClosedCaption on /closed_captions and keeping
#             the next face frame, then `ros2 topic echo /diagnostics`
#
# Passes when the caption lands on the ROS4HRI caption keys and, relayed, on
# the face's own caption keys — read back over the device's local bridge —
# and a /diagnostics report reaches the client with the view's frame rate
# and the face's compliance level. The face frame taken after the caption
# is kept as frame.png for a look at the overlay. Needs docker and the image
# build-image.sh builds (or ROS2_ZENOH_IMAGE), python3 with `websockets`,
# and `cargo build -p vizij --features ros2-zenoh` done first (or VIZIJ_BIN).
#
# Usage: captions-diagnostics.sh [text]
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../../../.." && pwd)"
IMAGE="${ROS2_ZENOH_IMAGE:-vizij-ros4hri:jazzy}"
BIN="${VIZIJ_BIN:-$ROOT/target/debug/vizij}"
GLB="${VIZIJ_GLB:-$ROOT/../vizij-web/Quori_Latest_ROS.glb}"
NS="${VIZIJ_NAMESPACE:-vizij_captions_e2e}"
ROUTER_PORT="${ROUTER_PORT:-7453}"
LOCAL_PORT="${LOCAL_PORT:-9453}"
ROUTER="vizij-captions-e2e-$$"
TEXT="${1:-Hello, I am a talking face, and these are my words as I say them.}"
OUT="${OUT_DIR:-$(mktemp -d)}"; mkdir -p "$OUT"
ROS='source /opt/ros/jazzy/setup.bash && source /ros4hri_ws/install/setup.bash'

log() { echo "[captions-e2e] $*" >&2; }
[ -x "$BIN" ] || { log "no device binary at $BIN — cargo build -p vizij --features ros2-zenoh"; exit 2; }
[ -f "$GLB" ] || { log "no face at $GLB (VIZIJ_GLB)"; exit 2; }
python3 -c 'import websockets' 2>/dev/null || { log "python3 needs websockets (pip install websockets)"; exit 2; }

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
"$BIN" --glb "$GLB" --headless --ros2 "$NS" --port "$LOCAL_PORT" --frame-rate 4 --no-autoplay \
  > "$OUT/device.log" 2>&1 &
DEVICE=$!

# Discovery settles a few seconds after the bridge starts: poll for the
# caption subscription and the diagnostics publisher.
for i in $(seq 1 24); do
  docker exec "$ROUTER" bash -lc "$ROS && ros2 topic info /closed_captions && ros2 topic info /diagnostics" \
    > "$OUT/topics.txt" 2>/dev/null || true
  grep -q "Subscription count: [1-9]" "$OUT/topics.txt" && grep -q "Publisher count: [1-9]" "$OUT/topics.txt" && break
  sleep 5
done
grep -q "Subscription count: [1-9]" "$OUT/topics.txt" || {
  log "FAIL: nothing subscribes /closed_captions"; cat "$OUT/topics.txt" >&2; exit 1; }
grep -q "Publisher count: [1-9]" "$OUT/topics.txt" || {
  log "FAIL: nothing publishes /diagnostics"; cat "$OUT/topics.txt" >&2; exit 1; }
log "/closed_captions and /diagnostics are on the graph"

# The client: publish the caption once its subscriber matches, then keep the
# first face frame published two seconds later.
cat > "$OUT/caption.py" <<'PY'
import sys, time
import rclpy
from rclpy.qos import QoSProfile, ReliabilityPolicy
from hri_actions_msgs.msg import ClosedCaption
from sensor_msgs.msg import CompressedImage

rclpy.init()
node = rclpy.create_node("captions_e2e")
publisher = node.create_publisher(ClosedCaption, "/closed_captions", 10)
frames = []
node.create_subscription(
    CompressedImage, "/robot_face/image_raw/compressed",
    lambda frame: frames.append((time.monotonic(), bytes(frame.data))),
    QoSProfile(depth=1, reliability=ReliabilityPolicy.BEST_EFFORT))
deadline = time.monotonic() + 30
while publisher.get_subscription_count() == 0 and time.monotonic() < deadline:
    rclpy.spin_once(node, timeout_sec=0.1)
if publisher.get_subscription_count() == 0:
    sys.exit("no subscriber matched /closed_captions")
publisher.publish(ClosedCaption(speaker_id=ClosedCaption.SPEAKER_ID_SYSTEM, text=sys.argv[1], locale="en_GB"))
sent = time.monotonic()
print("caption published", flush=True)
while time.monotonic() < sent + 10:
    rclpy.spin_once(node, timeout_sec=0.1)
    shown = [data for at, data in frames if at > sent + 2]
    if shown:
        open("/tmp/frame.png", "wb").write(shown[0])
        print("frame kept", flush=True)
        break
node.destroy_node()
rclpy.shutdown()
PY
docker cp "$OUT/caption.py" "$ROUTER:/tmp/caption.py"
docker exec "$ROUTER" bash -lc "$ROS && timeout 60 python3 /tmp/caption.py \"$TEXT\"" > "$OUT/client.txt" 2>&1 || true
grep -q "caption published" "$OUT/client.txt" || { log "FAIL: the caption was not published (see $OUT/client.txt)"; exit 1; }
docker cp "$ROUTER:/tmp/frame.png" "$OUT/frame.png" 2>/dev/null && log "face frame after the caption: $OUT/frame.png" \
  || log "no face frame kept (see $OUT/client.txt)"

# The caption keys, over the device's local bridge: the ROS4HRI ones the
# bridge wrote, and the face's own the mapping relays them to.
python3 - "$LOCAL_PORT" > "$OUT/keys.json" <<'PY'
import asyncio, json, sys
import websockets

async def main(port):
    async with websockets.connect(f"ws://127.0.0.1:{port}", max_size=None) as socket:
        async def ask(message, answer):
            await socket.send(json.dumps(message))
            while True:
                reply = json.loads(await socket.recv())
                if reply.get("type") == answer:
                    return reply
        listed = await ask({"type": "list_keys"}, "list_keys_resp")
        keys = [k["path"] for k in listed["keys"] if "/caption/" in k["path"]]
        read = await ask({"type": "read_values", "keys": keys}, "read_values_resp")
        print(json.dumps(read["values"], indent=1, sort_keys=True))

asyncio.run(main(int(sys.argv[1])))
PY
log "caption keys: $(tr -d '\n ' < "$OUT/keys.json")"
value_of() { python3 -c 'import json,sys; v=json.load(open(sys.argv[1])); print(next((x["str"] for k,x in v.items() if k.endswith(sys.argv[2])), ""))' "$OUT/keys.json" "$1"; }
[ "$(value_of standard/ros4hri/caption/text)" = "$TEXT" ] || { log "FAIL: standard/ros4hri/caption/text is not the caption"; exit 1; }
[ "$(value_of standard/ros4hri/caption/speaker)" = "__system__" ] || { log "FAIL: standard/ros4hri/caption/speaker is not __system__"; exit 1; }
[ "$(value_of standard/vizij/caption/text)" = "$TEXT" ] || { log "FAIL: the face's caption is not relayed"; exit 1; }
[ "$(value_of standard/vizij/caption/speaker)" = "__system__" ] || { log "FAIL: the face's caption speaker is not relayed"; exit 1; }
log "the caption landed on the ROS4HRI keys and the face's"

docker exec "$ROUTER" bash -lc "$ROS && timeout 20 ros2 topic echo --once /diagnostics" > "$OUT/diagnostics.txt" 2>&1 || true
grep -q "name: 'vizij: view'" "$OUT/diagnostics.txt" || { log "FAIL: no view status on /diagnostics (see $OUT/diagnostics.txt)"; exit 1; }
grep -q "key: frame_rate" "$OUT/diagnostics.txt" || { log "FAIL: no frame rate in the report"; exit 1; }
grep -q "name: 'vizij: face " "$OUT/diagnostics.txt" || { log "FAIL: no face status on /diagnostics"; exit 1; }
grep -q "key: compliance_level" "$OUT/diagnostics.txt" || { log "FAIL: no compliance level in the report"; exit 1; }
log "PASS — the caption is on the face's keys, /diagnostics reports (transcripts: $OUT)"
