# vizij-glb-migrate

Rewrites the Value-bearing JSON embedded in Vizij face-bundle `.glb` files to
the canonical arora `Value` serde forms (`{"f32": …}`, `{"str": …}`,
`{"struct": …}`, …). Everything else in the file — including the binary
chunk — is preserved.

## Usage

```
vizij-glb-migrate face.glb              # rewrite in place; original saved as face.glb.bak
vizij-glb-migrate face.glb -o out.glb   # write elsewhere; input untouched
vizij-glb-migrate --dry-run a.glb b.glb # report what would change; write nothing
vizij-glb-migrate --check a.glb b.glb   # like --dry-run, exit 1 if migration is needed (CI)
```

Exit status: `0` success (including nothing to do), `1` `--check` found files
needing migration, `2` error. An existing `.bak` is overwritten. Files whose
documents are already canonical are left untouched, byte-for-byte.

## What it touches

All rewrites happen inside the glTF JSON chunk:

- `scenes[*].extensions.VIZIJ_bundle` and `nodes[*].extensions.VIZIJ_bundle`:
  each `graphs[*].spec` / `graphs[*].ir` entry — an inline object or an
  embedded JSON string — is a node-graph document, normalized with
  `vizij_api_core::json::normalize_graph_spec_value`: node `params.value` and
  `input_defaults.<port>.value` payloads become canonical Values, legacy
  `inputs` wiring becomes `edges`, and shape shorthand expands. Inline edge
  `default` / `default_value` payloads are normalized value-by-value.
- `nodes[*].extensions.RobotData.features.<id>.value.default`: web raw forms
  (bare primitives, `{x,y}`, `{x,y,z}`, `{r,g,b[,a]}` — alpha defaults to 1)
  become canonical Values. `default` is the only Value-bearing field of a
  feature `value`; the sibling `constraints` hold plain numeric bounds and
  stay as they are.

Left alone:

- the `BIN` chunk and any other non-JSON chunk, byte-for-byte;
- `VIZIJ_bundle.poses.config` and
  `VIZIJ_bundle.animations[*].clip.tracks[*].keyframes[*].value` (plain
  scalars, not Values);
- unrecognized value payloads (reported as warnings on stderr).

When a rewrite happens, the JSON chunk is re-serialized compactly with its
member order preserved (`serde_json`'s `preserve_order`), so migrated files
diff cleanly; chunk lengths, 4-byte padding (spaces for the JSON chunk), and
the header total length are recomputed.
