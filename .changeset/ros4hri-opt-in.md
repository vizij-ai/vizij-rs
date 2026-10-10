---
"@vizij/runtime": major
---

A face loaded with `loadVizij`, or composed with `composeVizij`, speaks the vizij standard by default: its `standard/vizij/*` controls drive it through the bundle's `standard-adaptation`, and the built-in ROS4HRI mapping is not composed. A page that drives the face through the ROS4HRI keys (`standard/ros4hri/*`) asks for the mapping with `{ ros4hri: true }`. The idle blink is the ROS4HRI mapping's, so a face loaded without it blinks only when its program or a client blinks it. A face that embeds its own `standard::ros4hri` mapping runs it either way.

A face's standard controls are declared as the `vizij-face` profile states them: `listKeys` reports each `standard/vizij/*` input as an `f32`, an eye position in `[-1, 1]` and a weight in `[0, 1]`, resting at the profile's rest, and the device takes a remote write of those keys only as an `f32` (a page's `setValue` with a number writes one).
