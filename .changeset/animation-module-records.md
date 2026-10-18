---
"@vizij/animation-module": minor
---

The package ships the module's Arora records, so a device describes the exports that take or return a structure: `step_values`, `player_states`, `output_keys`, `load_animation`, the bakes. `loadAnimationModule()` returns `{ headerJson, wasmBytes, records }`, and `records` is also exported on its own: each record file's path under `records/` (`module/<id>@<version>.yaml`, `structure/<id>@<version>.yaml`, `folder/<id>.yaml`) mapped to its YAML, the map `arora-web`'s `withRecords` takes. The same files ship under `artifact/records/`, the directory a device's module directory carries beside `header.json`. The records are the module record (2.0.0) and the records of the structures its functions name, all in the `vizij` scope: the module record is `vizij.vizij-animation`. The header does not change.
