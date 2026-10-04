# Changelog

All notable changes to `vizij`, the desktop app. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions follow
[Semantic Versioning](https://semver.org/). A version reaching `main` is
released by `release-vizij.yml`: a binary and an installer per desktop OS, and
the browser module, attached to the `vizij-v<version>` GitHub release.

## [Unreleased]

### Added

- A face's animations (its bundle's `animations`) load into the device's
  animation module, each on a player of its own, its tracks writing the rig
  inputs their channels name. A loaded animation is silent until played: a
  client plays it through the module's functions (`set_weight` on its
  instance, then `play`, `pause`, `stop`, `seek`, `set_speed`, `set_loop`),
  its players in the bundle's order.

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
