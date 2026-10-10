---
"@vizij/runtime": major
---

A face loaded with `loadVizij`, or composed with `composeVizij`, speaks the vizij standard by default: its `standard/vizij/*` controls drive it through the bundle's `standard-adaptation`, and the built-in ROS4HRI mapping is not composed. A page that drives the face through the ROS4HRI keys (`standard/ros4hri/*`) asks for the mapping with `{ ros4hri: true }`. The idle blink is the ROS4HRI mapping's, so a face loaded without it blinks only when its program or a client blinks it. A face that embeds its own `standard::ros4hri` mapping runs it either way.
