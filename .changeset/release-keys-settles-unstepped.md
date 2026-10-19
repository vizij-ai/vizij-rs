---
"@vizij/runtime": minor
---

`releaseKeys` settles as the device's operations do. On a device with no `run()` going and no `step` from the page, the release asks for a `step(0)` in the next microtask, shared with the same turn's operations and `setValue`/`writeValues`. In that step, a released key that the face's behavior writes takes its writer's value again. Before, the key kept the page's value until something else stepped the device. A page that calls `step` in the same turn applies the release with its own step instead, and under `run()` the run's next step applies it, with no step added.
