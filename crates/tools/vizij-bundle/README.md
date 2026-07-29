# vizij-bundle

The face-bundle tool: reads and rewrites the `VIZIJ_bundle` a face GLB
carries, and validates a face's coverage of the Vizij standard. The GLB is a
build artifact — the bundle JSON is the reviewable source of truth, and this
tool is the deterministic bridge between the two (packing is idempotent, so
diffs stay meaningful).

```
vizij-bundle inspect        face.glb
vizij-bundle unpack         face.glb -o bundle.json
vizij-bundle pack           face.glb --bundle bundle.json -o out.glb
vizij-bundle add-graph      face.glb --graph adaptation.json \
                            --kind standard-adaptation --id my_adaptation -o out.glb
vizij-bundle add-standard   face.glb --standard ros4hri -o out.glb
vizij-bundle validate       face.glb [--min-level 2]
vizij-bundle profiles
vizij-bundle export-profile ros4hri -o ros4hri.json
```

- **inspect** — face summary as JSON: id, graphs, the input surface (store
  paths the rig listens on, rig prefix stripped), animatable features per node.
- **unpack / pack** — extract the bundle as a pretty-printed sidecar / write a
  sidecar back into a GLB. Binary chunks are preserved verbatim.
- **add-graph** — graft one graph into the bundle, replacing any entry with
  the same id. This is how a face gains a `standard-adaptation` graph (the
  asset-side mapping from `standard/vizij/*` controls onto the face's own pose
  weights and morphs) without a full unpack/pack cycle — see
  `fixtures/faces/quori/standard-adaptation.json` for the demo face's.
- **add-standard** — embed a shipped standard profile (e.g. `ros4hri`) into the
  face: the profile's control paths get the face's rig prefix, and it grafts
  under a stable id (`standard::<profile>`), so re-running updates the embedded
  copy in place. This is how a GLB opts into a standard systematically; the
  same profile graph is available to the web authoring app through
  `@vizij/runtime` (`standardProfile(id, rigPrefix)`).
- **validate** — the standard-coverage report: which control paths of each
  tier (gaze & lids, expressions, visemes, muscle) the face's graphs listen
  on, the compliance level L0–L3, and what is missing. `--min-level` turns it
  into a CI gate.
- **profiles** — list the standard profiles Vizij ships, as JSON — the
  introspectable menu of what a face may opt into.
- **export-profile** — regenerate a profile's canonical asset (the file
  `crates/interop/vizij-arora-host/profiles/<id>.json` that Rust embeds and the
  web runtime serves). Run this after editing the profile's generator; a test
  fails if the committed asset drifts from it.
