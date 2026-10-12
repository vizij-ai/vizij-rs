---
"@vizij/runtime": major
---

A face loaded with `loadVizij`, or composed with `composeVizij`, speaks the vizij standard: its `standard/vizij/*` controls drive it through the bundle's `standard-adaptation`. The built-in ROS4HRI mapping composes when a page asks for it with `{ ros4hri: true }`, and brings the ROS4HRI keys (`standard/ros4hri/*`), an idle blink and the `look_at` skill's gaze. A face that embeds its own `standard::ros4hri` mapping runs it either way.

A face's standard controls are declared as the `vizij-face` profile states them: `listKeys` reports each `standard/vizij/*` input as an `f32`, an eye position in `[-1, 1]` and a weight in `[0, 1]`, resting at the profile's rest, and the device takes a remote write of those keys only as an `f32` (a page's `setValue` with a number writes one).
