# ROS4HRI end to end, from a native client

The proof that a real ROS 2 client — `rclpy` on Jazzy with `rmw_zenoh_cpp`,
not a `ros2-client` peer — drives this device's ROS4HRI skill plane: a goal on
`/skill/say` (`communication_skills/action/Say`) is accepted, its feedback
streams the visemes as the face speaks, and it ends `SUCCEEDED`. Two peers of
ours match each other under any type hash, so an all-Rust test is blind to a
server no native node can reach; this is the test that is not.

It is manual. It needs docker, a network for the cloud text-to-speech provider,
and a `ros2-zenoh` build of the binary — and it cannot be a `cargo test`:
the crate's live-test dependency is the DDS `ros2-client`, which excludes the
Zenoh backend at compile time.

```bash
crates/vizij/tests/ros4hri/build-image.sh          # once: Jazzy + rmw_zenoh + the interfaces
cargo build -p vizij --features ros2-zenoh --bins  # the device
crates/vizij/tests/ros4hri/say-feedback.sh         # the test
```

- **The image** builds `std_skills` and `communication_skills` as real
  interface packages from the `.msg` / `.action` files `arora-msgs-ros2` vendors
  (found through `cargo metadata`), so the client speaks exactly what the bridge
  serves — Vizij's viseme feedback extension included.
- **The test** starts `rmw_zenohd` in the container (port `ROUTER_PORT`, 7451 by
  default, so a router you already run is neither joined nor collided with),
  the device on its own namespace (`VIZIJ_NAMESPACE`) joining it in Zenoh client
  mode, waits for `/skill/say` to be discovered, sends the goal with
  `--feedback`, and judges the client's transcript: accepted, at least three
  distinct shapes with a non-zero intensity, `sil` at zero, `SUCCEEDED`.

Knobs: `ROS2_ZENOH_IMAGE`, `VIZIJ_BIN`, `VIZIJ_GLB` (a face carrying the
standard adaptation; the Quori demo by default), `OUT_DIR` for the transcripts.
