# @vizij/import

Optional passes a face's GLB goes through when it is imported, before it
becomes a face bundle, compiled to WebAssembly from the
[`vizij-import`](../../../crates/tools/vizij-import) crate. Each pass rewrites
mesh data only and leaves the glTF JSON byte-for-byte.

## Usage

```ts
import { init, optimizeMorphTriangles, stillFolding } from "@vizij/import";

await init();
const bytes = new Uint8Array(await file.arrayBuffer());
const { bytes: optimized, report } = optimizeMorphTriangles(bytes);
const glb = optimized ?? bytes; // null when the pass changed nothing
for (const { part, folds } of stillFolding(report)) {
  console.warn(part.meshName, folds.map((f) => f.shape));
}
```

`init()` loads the wasm module once; in a browser it is fetched next to the
package's JavaScript. Outside a browser, pass the `.wasm` bytes or URL.

## Passes

### `optimizeMorphTriangles(glb)`

A glTF mesh stores one set of triangles that every morph target reuses, and
exporters pick them from the rest shape. A part modelled as one large polygon
— a mouth resting as a nearly flat line, a pupil that squashes into an arc —
then keeps triangles that fold once a target bends its outline, so the shape
draws filled in where it should be open.

The pass replaces each such polygon's triangles with the triangulation of the
same outline that folds least across the rest shape and every morph target.
Vertices and triangle counts are unchanged. It returns the rewritten file
(`null` when nothing changed) and a report: for each part, whether it was
`retriangulated`, `kept` (already least folded) or `skipped` (with the
reason), and for each shape the share of its area drawn with reversed
winding before and after.

Some shapes still fold afterwards — an outline that crosses itself, or bends
further than any triangulation of its vertices can follow.
`stillFolding(report, threshold = 0.01)` lists them; those parts need
remodelling. The crate README explains which parts the pass considers.

## Development

```
pnpm run build:wasm:import   # from the repository root: wasm-pack build into pkg/
pnpm --filter @vizij/import test
```
