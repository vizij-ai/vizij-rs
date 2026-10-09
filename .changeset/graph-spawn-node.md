---
"@vizij/node-graph": minor
---

A `spawn` node starts a task run of its function each time its arguments change: the `value` param's fields, then the keyed `args` inputs, one per parameter id in `record_keys`. The first evaluation only records them. The host serves the request after the evaluation and owns the run.
