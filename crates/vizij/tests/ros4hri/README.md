# ROS4HRI end to end, from a native client

The proof that a real ROS 2 client — `rclpy` on Jazzy with `rmw_zenoh_cpp`,
not a `ros2-client` peer — drives this device's ROS4HRI skill plane. Two peers
of ours match each other under any type hash, so an all-Rust test is blind to
a server no native node can reach; these are the tests that are not.

- **`say-feedback.sh`** — a goal on `/skill/say`
  (`communication_skills/action/Say`) is accepted, its feedback streams the
  visemes as the face speaks, and it ends `SUCCEEDED`.
- **`look-at.sh`** — the gaze skill's goal lifecycle on `/skill/look_at`
  (`interaction_skills/action/LookAt`): a tracking goal is accepted and keeps
  running; a second goal at the same priority replaces it (the first ends
  `ROS_EINTR`); a lower-priority goal is rejected; a `glance` replaces the
  tracker and ends `SUCCEEDED`; a `reset` ends `SUCCEEDED`.

They are manual. They need docker, a `ros2-zenoh` build of the binary (and,
for `say`, a network for the cloud text-to-speech provider) — and they cannot
be `cargo test`s:
the crate's live-test dependency is the DDS `ros2-client`, which excludes the
Zenoh backend at compile time.

```bash
crates/vizij/tests/ros4hri/build-image.sh          # once: Jazzy + rmw_zenoh + the interfaces
cargo build -p vizij --features ros2-zenoh --bins  # the device
crates/vizij/tests/ros4hri/say-feedback.sh         # the speech test
crates/vizij/tests/ros4hri/look-at.sh              # the gaze test
```

- **The image** builds `std_skills`, `communication_skills`,
  `interaction_skills` and `hri_msgs` as real interface packages from the `.msg` / `.action`
  files `arora-msgs-ros2` vendors (found through `cargo metadata`), so the
  client speaks exactly what the bridge serves — Vizij's viseme feedback
  extension included. It is also what makes `ros2 action send_goal` complete
  and accept the skill types: without the packages installed, the CLI has no
  definition to speak.
- **Each test** starts `rmw_zenohd` in the container (port `ROUTER_PORT` — 7451
  for `say`, 7452 for `look_at` — so a router you already run is neither joined
  nor collided with), the device on its own namespace (`VIZIJ_NAMESPACE`)
  joining it in Zenoh client mode, waits for the skill to be discovered, sends
  its goals with the CLI, and judges the client's own transcripts.

Knobs: `ROS2_ZENOH_IMAGE`, `VIZIJ_BIN`, `VIZIJ_GLB` (a face carrying the
standard adaptation; the Quori demo by default), `OUT_DIR` for the transcripts.
