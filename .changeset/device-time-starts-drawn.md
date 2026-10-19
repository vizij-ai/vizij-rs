---
"@vizij/runtime": minor
---

A Vizij's device keeps no time until the Vizij is first drawn (`ready`). Until then every `step(dtMs)` is a `step(0)`, whatever `dtMs` says, so it still carries the page's operations and writes through the face's graphs. The first step after advances the time since the frame that drew the Vizij, at most its `dtMs`, and the steps after that advance their `dtMs`. A `run()` called before then starts its loop at that frame; a `stop()` before it gives the run up, and its promise resolves. Before, a page stepping a face from `loadVizij` on — `@vizij/runtime-react`'s provider does — ran the face's springs, programs and animations through the load and the first draw, in steps as long as the frames the load blocked (0.8 s on an M1), and the face first showed mid-motion. A device with no Vizij (`startRuntime`) keeps time from its first step, as before.
