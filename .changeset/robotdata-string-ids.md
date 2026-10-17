---
"@vizij/runtime": patch
---

A Vizij whose RobotData ids are no UUIDs loads and draws: an element's and an animatable's id is any string, as the published three.js line reads it, where `loadVizij` and `describe` failed with "bad RobotData on node …: UUID parsing failed". Picks report the id as written, and `setSelection`, `setStaticFeature` and `hold` take it.
