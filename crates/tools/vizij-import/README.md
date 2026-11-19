# vizij-import

Optional passes a face's `.glb` goes through when it is imported, before it
becomes a face bundle. Each pass rewrites mesh data in place and reports
what it did; the JSON chunk is preserved byte-for-byte.

## Passes

### `--morph-triangles`

Chooses triangles that stay unfolded across a mesh's morph targets.

A glTF mesh stores one set of triangles that every morph target reuses, and
an exporter picks them from the rest shape alone. A part modelled as one
large polygon — a mouth that rests as a nearly flat line, a pupil that
squashes into an arc to close — then keeps triangles that fold over each
other once a target bends its outline. Folded triangles cover ground outside
the outline, so the shape reads as filled in where it should be open.
Blender does not show this, because it re-triangulates a polygon from its
current outline whenever a shape key changes it; a glTF file has no
polygons to re-triangulate.

For each connected piece of a morphed mesh whose vertices all lie on one
boundary loop (the shape a single exported polygon leaves), the pass
searches every triangulation of that loop and keeps the one with the least
area drawn with reversed winding, summed over the rest shape, each morph
target at full weight, and each target at half weight. Vertices are not
added, moved or reordered and the triangle count is kept, so only index
data changes. A file the pass has already been through is left untouched.

Left alone, and reported:

- pieces that are not a single polygon — vertices inside their outline
  (a grid), a hole (a ring), or several outlines: their triangles were
  authored deliberately or cannot be re-chosen from one outline;
- pieces with no area in any shape;
- primitives without index data, other than triangle lists, Draco-compressed,
  with quantized or non-finite positions, with data outside the GLB binary
  chunk (buffer 0), or whose index bytes anything else also reads — another
  primitive or accessor, an attribute, an image — since a rewrite would
  change that data too;
- outlines with more vertices than the search takes (the report names the
  limit): the search grows with the cube of the vertex count and runs
  synchronously.

Some shapes still fold after the pass: an outline that crosses itself, or
that bends further than any triangulation of its vertices can follow (a
lip curling into a hook with few vertices along it). The report flags those
shapes (`morph_triangles::STILL_FOLDS`) so the part can be remodelled — more
vertices along the bend, or upper and lower edges whose vertices pair up in
every shape.

The shapes checked are the rest shape, each morph target and each pair of
targets at full and at half weight, and the mesh's default pose when its
default weights set any target. Those are samples: a triangle can stay
unfolded at 0, ½ and 1 and still fold at a weight in between, and blends of
three or more targets are not checked. A part is rewritten only when the
improvement is large enough to see, and the mode sets what else must hold:

- `safe` (the default): no checked shape may come out visibly worse. Fixes
  less; harms nothing it checks.
- `overall`: the checked shapes must improve overall. Fixes more; blends that
  got worse are listed in the report.

The search grows with the number of shapes as well as the outline, so a part
with very many targets may be skipped as too large to check every blend.

## Usage

```
vizij-import --morph-triangles face.glb              # safe mode; rewrite in place, original saved as face.glb.bak
vizij-import --morph-triangles=overall face.glb      # overall mode
vizij-import --morph-triangles face.glb -o out.glb   # write elsewhere; input untouched
vizij-import --morph-triangles --dry-run a.glb b.glb # report what would change; write nothing
vizij-import --morph-triangles --check a.glb b.glb   # like --dry-run, exit 1 if a file would change (CI)
```

At least one pass must be named. Exit status: `0` success (including
nothing to do), `1` `--check` found files that would change, `2` error.

The report lists each part the pass changed, skipped, or that still folds.
For each shape checked it
gives the area drawn with reversed winding before and after, relative to
that shape's own area. It exceeds 100% when the wrongly drawn area is larger
than the shape itself, as with a pupil squashed to a thin arc:

```
face.glb: mouth (primitive 0, 65 vertices): retriangulated
    smile_shape              folded 41.2% -> 12.1%  still folds
    approval_shape           folded 23.3% -> 0.9%
```

## Library

`vizij_import::optimize_morph_triangles(&bytes, Mode::Safe)` runs the same pass and
returns the rewritten file (when anything changed) with its
`morph_triangles::Report`.
