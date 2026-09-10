# vizij-bundle

The face-bundle tool: reads and rewrites the `VIZIJ_bundle` a face GLB
carries, and validates a face's coverage of the Vizij standard. The GLB is a
build artifact — the bundle JSON is the reviewable source of truth, and this
tool is the deterministic bridge between the two (packing is idempotent, so
diffs stay meaningful).

It speaks in [profiles and mappings](../../../docs/profiles-and-mappings.md):
a **profile** is an interface (the store paths a party exposes, each typed),
a **mapping** is a graph implementing one profile in terms of another.

```
vizij-bundle inspect        face.glb
vizij-bundle unpack         face.glb -o bundle.json
vizij-bundle pack           face.glb --bundle bundle.json -o out.glb
vizij-bundle add-graph      face.glb --graph adaptation.json \
                            --kind standard-adaptation --id my_adaptation -o out.glb
vizij-bundle add-standard   face.glb --standard ros4hri -o out.glb
vizij-bundle add-profile    face.glb --profile vizij-face -o out.glb
vizij-bundle validate       face.glb [--min-level 2]
vizij-bundle profiles
vizij-bundle export-profile vizij-face -o vizij-face.json
vizij-bundle mappings
vizij-bundle export-mapping ros4hri -o ros4hri.json
vizij-bundle surface        adaptation.json --side output --scope face --id my_face
vizij-bundle export-skill   look_at -o look_at.json
```

- **inspect** — face summary as JSON: id, the profiles it declares, graphs,
  the input surface (store paths the rig listens on, rig prefix stripped),
  animatable features per node.
- **unpack / pack** — extract the bundle as a pretty-printed sidecar / write a
  sidecar back into a GLB. Binary chunks are preserved verbatim.
- **add-graph** — graft one graph into the bundle, replacing any entry with
  the same id. This is how a face gains a `standard-adaptation` graph (the
  asset-side mapping from `standard/vizij/*` controls onto the face's own pose
  weights and morphs) without a full unpack/pack cycle — see
  `fixtures/faces/quori/standard-adaptation.json` for the demo face's.
- **add-standard** — embed a shipped standard mapping (e.g. `ros4hri`) into
  the face: the mapping's control paths get the face's rig prefix, and it
  grafts under a stable id (`standard::<mapping>`), so re-running updates the
  embedded copy in place. This is how a GLB opts into a standard
  systematically; the same graph is available to the web authoring app
  through `@vizij/runtime` (`mapping(id, rigPrefix)`).
- **add-profile** — declare a shipped profile on the face, in the bundle's
  top-level `profiles` array: the interface its graphs are authored against
  travels with the asset. Re-running replaces the entry of the same id.
- **validate** — the standard-coverage report: which paths of each tier of
  the `vizij-face` profile (gaze & lids, expressions, visemes, muscle) the
  face's graphs listen on, the compliance level L0–L3, and what is missing.
  `--min-level` turns it into a CI gate.
- **profiles** — the profiles Vizij ships, as JSON: `vizij-face` (81 paths,
  face-scoped) and `ros4hri` (25 paths, device-scoped).
- **mappings** — the standard mappings Vizij ships, as JSON: the opt-in menu
  `add-standard` and the web's `mappings()` draw from.
- **surface** — lift a profile out of a mapping graph: its `input` nodes are
  the interface it consumes, its `output` nodes the one it produces. `--scope`
  states whether the lifted paths are per face (`face`) or absolute (`device`,
  the default) — a graph cannot tell. Use it to reconcile a mapping against a
  declared profile, or to read off the interface a face's own adaptation
  implements. A bootstrap, not a source of truth: a mapping only touches the
  part of a profile it needs.
- **export-profile / export-mapping / export-skill** — regenerate a shipped
  asset from its generator (the files under
  `crates/interop/vizij-arora-host/{profiles,mappings,skills}/<id>.json` that
  Rust embeds and the web runtime serves). Run the matching one after editing
  a generator; a test fails if the committed asset drifts from it.
