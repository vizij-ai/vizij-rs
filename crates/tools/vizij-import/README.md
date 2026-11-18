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

- pieces with vertices inside their outline (grids, rings of quads): their
  triangles were authored deliberately;
- primitives without index data, other than triangle lists, Draco-compressed,
  sharing their index data with another primitive, or with quantized
  positions;
- outlines longer than 512 vertices.

Some shapes still fold after the pass: an outline that crosses itself, or
that bends further than any triangulation of its vertices can follow (a
lip curling into a hook with few vertices along it). The report flags every
shape still folding by more than 1% so the part can be remodelled — more
vertices along the bend, or upper and lower edges whose vertices pair up in
every shape.

Morph targets are checked at weights between 0 and 1; combinations of
targets that a face's poses blend are not checked one by one.

## Usage

```
vizij-import --morph-triangles face.glb              # rewrite in place; original saved as face.glb.bak
vizij-import --morph-triangles face.glb -o out.glb   # write elsewhere; input untouched
vizij-import --morph-triangles --dry-run a.glb b.glb # report what would change; write nothing
vizij-import --morph-triangles --check a.glb b.glb   # like --dry-run, exit 1 if a file would change (CI)
```

At least one pass must be named. Exit status: `0` success (including
nothing to do), `1` `--check` found files that would change, `2` error.

The report lists each part the pass changed, skipped, or that still folds,
with the share of each shape's area drawn reversed before and after:

```
face.glb: mouth (primitive 0, 65 vertices): retriangulated
    smile_shape              folded 41.2% -> 12.1%  still folds
    approval_shape           folded 23.3% -> 0.9%
```

## Library

`vizij_import::optimize_morph_triangles(&bytes)` runs the same pass and
returns the rewritten file (when anything changed) with its
`morph_triangles::Report`.
