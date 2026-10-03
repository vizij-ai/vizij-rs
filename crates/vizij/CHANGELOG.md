# Changelog

All notable changes to `vizij`, the desktop app. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/). A version reaching `main` is
released by `release-vizij.yml`: a binary and an installer per desktop OS, and
the browser module, attached to the `vizij-v<version>` GitHub release.

## [Unreleased]

### Added

- A face's animations (its bundle's `animations`) load into its device when
  the face loads, natively and in the browser, through the animation
  module's declared functions — the calls any client sends
  (`face::load_animations`): `load_animation`, `create_player` named after
  the animation's id, `add_instance_with_weight` at 0, `stop`. Each track
  writes the rig input its channel names. A loaded animation is silent until
  played: a client finds its player by name in `player_states` (the
  `vizij/animations/players` key), gives its instance weight with
  `set_weight`, then drives it with `play`, `pause`, `stop`, `seek`,
  `set_speed` and `set_loop`; `remove_player` and `unload_animation` unload
  it. The browser module's `loadVizij` takes `animations: false` to leave
  them out.

### Changed

- Opening another face (`O`, a dropped `.glb`, a reload) keeps the current one
  on screen until the new one is ready, instead of an empty window meanwhile.

### Fixed

- A face whose RobotData carries static features (a value in place of an
  animatable) loads; it was refused as bad RobotData.

## [0.1.0] - 2026-10-02

### Added

- The `vizij` binary: a window over the Bevy view showing a face from its GLB
  (`--glb` takes a path or a URL and is remembered; a `.glb` dropped on the
  window or opened with `O` loads), with the face's Arora device behind it.
- The open local bridge on `--bind:--port` (`ws://127.0.0.1:9000` by default),
  with the control panel on the same port unless `--no-web-control`. A client
  lists the device's keys with their `__meta`, writes the face's inputs, calls
  the face's skills by name and `reset`.
- `--studio` registers the device with Semio Studio. The released binaries
  carry it; `--ros2` (a ROS4HRI face on a ROS 2 graph) is a build feature they
  leave out.
- Window flags: `--fullscreen`, `--display`, `--width`/`--height`,
  `--no-decorations`, `--always-on-top`; `vizij list-displays`.
