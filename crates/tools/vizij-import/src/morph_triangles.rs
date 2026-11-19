//! Chooses triangles that stay unfolded across a mesh's morph targets.
//!
//! A glTF mesh stores one fixed set of triangles that every morph target
//! reuses. An exporter picks them from the rest shape alone, so a part
//! modelled as one large polygon — a mouth that rests as a flat line, a
//! pupil that squashes into an arc — keeps triangles that fold over each
//! other once a target bends the outline. Folded triangles cover ground
//! outside the outline, which reads as a shape filled in where it should be
//! open. Blender does not show this, because it re-triangulates a polygon
//! from its current outline every time a shape key changes it.
//!
//! This pass finds each such polygon (a connected piece of a morphed mesh
//! whose vertices all lie on one boundary loop) and replaces its triangles
//! with the triangulation of the same loop that folds least, summed over
//! the rest shape, each morph target at full weight, and each target at
//! half weight. The halfway shapes stand in for the shapes an animation
//! passes through; they are samples, not a guarantee: a triangle can stay
//! unfolded at 0, ½ and 1 and still fold at a weight in between. It
//! searches every triangulation of the loop with the standard O(n³)
//! dynamic programme for polygon triangulation.
//!
//! The cost is the area triangles cover with reversed winding, summed over
//! the shapes and measured against the part's largest shape, so shapes
//! trade off by the overdraw a viewer sees. Every triangulation of a loop has signed
//! areas summing to the loop's own area, so the area drawn in total is the
//! loop's area plus twice the reversed area: driving the reversed area to
//! zero is what makes the triangles cover exactly the outline, and any
//! remainder bounds the overdraw.
//!
//! Vertices are never added, moved or reordered, and a polygon keeps its
//! triangle count, so only the index data changes. A shape whose outline
//! crosses itself, or that bends further than any triangulation of its
//! vertices can follow, still folds afterwards; the report names those
//! shapes so the part can be remodelled.

use std::collections::HashMap;
use std::fmt;

use serde_json::Value;

use crate::gltf::{write_index, Document, IndexSlots, Unsupported};

/// Shapes whose area is below this fraction of their part's largest shape
/// are treated as having no area (a part hidden by collapsing it) and do
/// not count: any triangulation of them is equally invisible.
const DEGENERATE_AREA: f64 = 1e-3;

/// A replacement is written only when it removes more than this much
/// reversed area, summed over the shapes and measured against the part's
/// largest shape. Below it the difference is not visible, so a file is not
/// rewritten (or flagged by `--check`) for it; and a part already at its
/// best keeps its triangles, so a second run changes nothing.
const IMPROVEMENT: f64 = 1e-3;

/// Loops with more vertices than this are left alone. The search grows
/// with the cube of the vertex count times the number of shapes, and it runs
/// synchronously (in the browser, on the main thread): at this size and
/// thirty shapes it is about a hundred million triangle evaluations.
const MAX_LOOP_VERTICES: usize = 256;

/// A shape that still has more than this much reversed area after the
/// pass, relative to its own area, is one no triangulation of its vertices
/// follows: the part needs remodelling there.
pub const STILL_FOLDS: f64 = 0.01;

/// What [`crate::optimize_morph_triangles`] did with every polygon it considered.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub parts: Vec<Part>,
}

impl Report {
    /// Whether any part's triangles were replaced.
    pub fn changed(&self) -> bool {
        self.parts
            .iter()
            .any(|p| matches!(p.outcome, Outcome::Retriangulated))
    }
}

/// One polygon of a morphed mesh primitive, or a primitive the pass could
/// not read.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// The mesh's index in the document and its name, when it has one.
    pub mesh: usize,
    pub mesh_name: Option<String>,
    pub primitive: usize,
    /// How many vertices the polygon has (the primitive's vertex count when
    /// it was skipped as a whole).
    pub vertices: usize,
    pub outcome: Outcome,
    /// Reversed area before and after, per shape. Empty when skipped.
    pub folds: Vec<ShapeFold>,
}

/// What the pass did with a part.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Its triangles were replaced with ones that fold less.
    Retriangulated,
    /// Its triangles already fold as little as any triangulation can.
    Kept,
    /// It was left alone, for the reason given.
    Skipped(Skip),
}

/// Why a part was left alone.
#[derive(Debug, Clone, PartialEq)]
pub enum Skip {
    /// The primitive draws something other than a triangle list.
    NotTriangles,
    /// The primitive has no index data to rewrite.
    NotIndexed,
    /// Its geometry is Draco-compressed.
    Compressed,
    /// Its index bytes are also read by another accessor or primitive,
    /// which rewriting them would change too.
    SharedIndices,
    /// The piece is not a single polygon: it has vertices inside its
    /// outline, a hole, or more than one outline. Its triangles were authored
    /// deliberately (a grid, a ring) or cannot be re-chosen from an outline.
    NotAPolygon,
    /// The outline has more vertices than the search takes.
    TooManyVertices,
    /// No shape of the piece has any area, so every triangulation draws the
    /// same nothing.
    NoArea,
    /// The data could not be read.
    Unreadable(String),
}

impl fmt::Display for Skip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Skip::NotTriangles => f.write_str("not a triangle list"),
            Skip::NotIndexed => f.write_str("has no index data"),
            Skip::Compressed => f.write_str("is Draco-compressed"),
            Skip::SharedIndices => {
                f.write_str("shares its index data with other mesh data")
            }
            Skip::NotAPolygon => f.write_str(
                "is not a single polygon (vertices inside its outline, a hole, or several outlines)",
            ),
            Skip::NoArea => f.write_str("has no area in any shape"),
            Skip::TooManyVertices => {
                write!(f, "has more than {MAX_LOOP_VERTICES} outline vertices")
            }
            Skip::Unreadable(why) => write!(f, "could not be read: {why}"),
        }
    }
}

/// The area one shape's triangles cover with reversed winding, before and
/// after, as a multiple of the shape's own area. It exceeds 1 when the
/// wrongly drawn area is larger than the shape itself, as with a pupil
/// squashed to a thin arc. `None` when the shape has no area to speak of.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeFold {
    /// `rest`; a morph target's name (`target N` without one) at full
    /// weight; that name followed by ` (halfway)` at half weight; or
    /// `default pose`, the mesh's default weights, when any is set.
    pub shape: String,
    pub before: Option<f64>,
    pub after: Option<f64>,
}

impl From<Unsupported> for Skip {
    fn from(u: Unsupported) -> Self {
        Skip::Unreadable(match u {
            Unsupported::ExternalBuffer => "data is outside the GLB binary chunk".into(),
            Unsupported::NotFloat => "positions are not 32-bit floats".into(),
            Unsupported::SparseIndices => "indices are sparse".into(),
            Unsupported::Malformed(why) => why,
        })
    }
}

/// Re-triangulates every polygon of every morphed mesh in `json`, writing
/// the new indices into `bin` (the GLB binary chunk, buffer 0).
pub(crate) fn optimize(json: &Value, bin: &mut [u8]) -> Report {
    let mut report = Report::default();
    let index_users = accessor_uses(json);
    let reader_ranges = Document { json, bin: &*bin }.reader_ranges();
    let meshes = json.get("meshes").and_then(Value::as_array);
    for (m, mesh) in meshes.into_iter().flatten().enumerate() {
        let mesh_name = mesh.get("name").and_then(Value::as_str).map(str::to_owned);
        let target_names: Vec<String> = mesh
            .pointer("/extras/targetNames")
            .and_then(Value::as_array)
            .map(|names| {
                names
                    .iter()
                    .map(|n| n.as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        let primitives = mesh.get("primitives").and_then(Value::as_array);
        for (p, primitive) in primitives.into_iter().flatten().enumerate() {
            let has_targets = primitive
                .get("targets")
                .and_then(Value::as_array)
                .is_some_and(|t| !t.is_empty());
            if !has_targets {
                continue;
            }
            let skipped = |vertices: usize, why: Skip| Part {
                mesh: m,
                mesh_name: mesh_name.clone(),
                primitive: p,
                vertices,
                outcome: Outcome::Skipped(why),
                folds: Vec::new(),
            };
            let doc = Document { json, bin: &*bin };
            let (parts, writes) = match primitive_parts(
                &doc,
                primitive,
                &target_names,
                &index_users,
                &reader_ranges,
                mesh.get("weights").and_then(Value::as_array),
            ) {
                Ok(found) => found,
                Err((vertices, why)) => {
                    report.parts.push(skipped(vertices, why));
                    continue;
                }
            };
            for (slots, i, value) in writes {
                write_index(bin, slots, i, value);
            }
            report.parts.extend(parts.into_iter().map(|part| Part {
                mesh: m,
                mesh_name: mesh_name.clone(),
                primitive: p,
                ..part
            }));
        }
    }
    report
}

/// How many times each accessor is referenced: as a primitive's indices,
/// attribute or morph target, a skin's inverse bind matrices, or an
/// animation sampler's input or output. An index accessor is rewritten only
/// when its one use is the primitive being rewritten.
fn accessor_uses(json: &Value) -> HashMap<u64, usize> {
    let mut uses = HashMap::new();
    let mut count = |value: Option<&Value>| {
        if let Some(a) = value.and_then(Value::as_u64) {
            *uses.entry(a).or_insert(0) += 1;
        }
    };
    let list = |key: &str| {
        json.get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
    };
    for mesh in list("meshes") {
        let primitives = mesh.get("primitives").and_then(Value::as_array);
        for primitive in primitives.into_iter().flatten() {
            count(primitive.get("indices"));
            let attributes = primitive.get("attributes").and_then(Value::as_object);
            attributes
                .into_iter()
                .flatten()
                .for_each(|(_, a)| count(Some(a)));
            let targets = primitive.get("targets").and_then(Value::as_array);
            for target in targets.into_iter().flatten() {
                let target = target.as_object();
                target
                    .into_iter()
                    .flatten()
                    .for_each(|(_, a)| count(Some(a)));
            }
        }
    }
    for skin in list("skins") {
        count(skin.get("inverseBindMatrices"));
    }
    for animation in list("animations") {
        let samplers = animation.get("samplers").and_then(Value::as_array);
        for sampler in samplers.into_iter().flatten() {
            count(sampler.get("input"));
            count(sampler.get("output"));
        }
    }
    uses
}

type Write = (IndexSlots, usize, u32);

/// The parts of one primitive and the index writes that re-triangulate
/// them. Only the mesh-independent fields of each part are filled in.
fn primitive_parts(
    doc: &Document,
    primitive: &Value,
    target_names: &[String],
    index_users: &HashMap<u64, usize>,
    reader_ranges: &[(Option<u64>, std::ops::Range<usize>)],
    default_weights: Option<&Vec<Value>>,
) -> Result<(Vec<Part>, Vec<Write>), (usize, Skip)> {
    let position = primitive
        .pointer("/attributes/POSITION")
        .and_then(Value::as_u64)
        .ok_or((0, Skip::Unreadable("no POSITION attribute".into())))?;
    let vertex_count = doc.count(position).map_err(|u| (0, u.into()))?;
    let fail = |why: Skip| (vertex_count, why);
    if primitive
        .pointer("/extensions/KHR_draco_mesh_compression")
        .is_some()
    {
        return Err(fail(Skip::Compressed));
    }
    if primitive.get("mode").and_then(Value::as_u64).unwrap_or(4) != 4 {
        return Err(fail(Skip::NotTriangles));
    }
    let indices = primitive
        .get("indices")
        .and_then(Value::as_u64)
        .ok_or(fail(Skip::NotIndexed))?;
    if index_users.get(&indices).copied().unwrap_or(0) > 1 {
        return Err(fail(Skip::SharedIndices));
    }
    let base = doc.read_vec3(position, None).map_err(|u| fail(u.into()))?;
    let (index_values, slots) = doc.read_indices(indices).map_err(|u| fail(u.into()))?;
    // Anything else reading the same bytes (an aliasing copy of this
    // accessor, data packed over it, an image) would change with every
    // rewrite.
    let written = slots.range(index_values.len());
    if reader_ranges.iter().any(|(reader, r)| {
        *reader != Some(indices) && r.start < written.end && written.start < r.end
    }) {
        return Err(fail(Skip::SharedIndices));
    }
    if !index_values.len().is_multiple_of(3)
        || index_values.iter().any(|&i| i as usize >= base.len())
    {
        return Err(fail(Skip::Unreadable(
            "indices do not form triangles over the vertices".into(),
        )));
    }
    // Rest first, then each target at full weight, then each at half.
    let mut shapes: Vec<(Option<String>, Vec<[f64; 3]>)> =
        vec![(Some("rest".into()), widen(&base))];
    let mut halves = Vec::new();
    let mut default_pose = widen(&base);
    let targets = primitive
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    for (t, target) in targets.enumerate() {
        let name = target_names
            .get(t)
            .filter(|n| !n.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("target {t}"));
        let deltas = match target.get("POSITION").and_then(Value::as_u64) {
            Some(acc) => doc
                .read_vec3(acc, Some(base.len()))
                .map_err(|u| fail(u.into()))?,
            None => vec![[0.0; 3]; base.len()],
        };
        let at = |w: f64| -> Vec<[f64; 3]> {
            base.iter()
                .zip(&deltas)
                .map(|(b, d)| std::array::from_fn(|k| b[k] as f64 + w * d[k] as f64))
                .collect()
        };
        let weight = default_weights
            .and_then(|w| w.get(t))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if weight != 0.0 {
            for (point, delta) in default_pose.iter_mut().zip(&deltas) {
                for k in 0..3 {
                    point[k] += weight * delta[k] as f64;
                }
            }
        }
        halves.push((Some(format!("{name} (halfway)")), at(0.5)));
        shapes.push((Some(name), at(1.0)));
    }
    shapes.extend(halves);
    // The mesh's default weights are what it shows when nothing drives it,
    // which is not the rest shape when any of them is set.
    if default_weights.is_some_and(|w| w.iter().any(|v| v.as_f64().unwrap_or(0.0) != 0.0)) {
        shapes.push((Some("default pose".into()), default_pose));
    }
    if shapes
        .iter()
        .any(|(_, points)| points.iter().flatten().any(|c| !c.is_finite()))
    {
        return Err(fail(Skip::Unreadable(
            "positions or morph deltas are not finite".into(),
        )));
    }

    let triangles: Vec<[u32; 3]> = index_values
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let mut parts = Vec::new();
    let mut writes = Vec::new();
    for piece in pieces(&triangles, base.len()) {
        if piece.vertices < 4 {
            continue;
        }
        let part = |outcome: Outcome, folds: Vec<ShapeFold>| Part {
            mesh: 0,
            mesh_name: None,
            primitive: 0,
            vertices: piece.vertices,
            outcome,
            folds,
        };
        let Some(outline) = outline(&triangles, &piece) else {
            parts.push(part(Outcome::Skipped(Skip::NotAPolygon), Vec::new()));
            continue;
        };
        if outline.len() > MAX_LOOP_VERTICES {
            parts.push(part(Outcome::Skipped(Skip::TooManyVertices), Vec::new()));
            continue;
        }
        let measured = measure(&shapes, &outline);
        if measured.largest <= 0.0 {
            parts.push(part(Outcome::Skipped(Skip::NoArea), Vec::new()));
            continue;
        }
        let current: Vec<[u32; 3]> = piece.triangles.iter().map(|&t| triangles[t]).collect();
        let before = measured.fold_per_shape(&current);
        let chosen = measured.best_triangulation(&outline);
        let after = measured.fold_per_shape(&chosen);
        let improved =
            measured.weighted_total(&current) - measured.weighted_total(&chosen) > IMPROVEMENT;
        let folds = shapes
            .iter()
            .zip(before.iter().zip(&after))
            .filter_map(|((name, _), (b, a))| {
                name.as_ref().map(|shape| ShapeFold {
                    shape: shape.clone(),
                    before: *b,
                    after: if improved { *a } else { *b },
                })
            })
            .collect();
        if improved {
            for (slot, tri) in piece.triangles.iter().zip(&chosen) {
                for (k, &v) in tri.iter().enumerate() {
                    writes.push((slots, slot * 3 + k, v));
                }
            }
            parts.push(part(Outcome::Retriangulated, folds));
        } else {
            parts.push(part(Outcome::Kept, folds));
        }
    }
    Ok((parts, writes))
}

fn widen(points: &[[f32; 3]]) -> Vec<[f64; 3]> {
    points
        .iter()
        .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
        .collect()
}

/// A connected set of triangles, by triangle index.
struct Piece {
    triangles: Vec<usize>,
    vertices: usize,
}

/// Splits triangles into pieces that share vertices.
fn pieces(triangles: &[[u32; 3]], vertex_count: usize) -> Vec<Piece> {
    let mut parent: Vec<usize> = (0..vertex_count).collect();
    fn root(parent: &mut [usize], mut v: usize) -> usize {
        while parent[v] != v {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        v
    }
    for t in triangles {
        let a = root(&mut parent, t[0] as usize);
        for &v in &t[1..] {
            let b = root(&mut parent, v as usize);
            parent[b] = a;
        }
    }
    let mut by_root: HashMap<usize, (Vec<usize>, Vec<u32>)> = HashMap::new();
    let mut order = Vec::new();
    for (i, t) in triangles.iter().enumerate() {
        let r = root(&mut parent, t[0] as usize);
        let entry = by_root.entry(r).or_insert_with(|| {
            order.push(r);
            Default::default()
        });
        entry.0.push(i);
        entry.1.extend_from_slice(t);
    }
    order
        .into_iter()
        .map(|r| {
            let (triangles, mut vertices) = by_root.remove(&r).expect("root recorded");
            vertices.sort_unstable();
            vertices.dedup();
            Piece {
                triangles,
                vertices: vertices.len(),
            }
        })
        .collect()
}

/// The piece's boundary loop, in the direction its triangles wind, when
/// the piece is a triangulated polygon: one loop through every vertex, no
/// vertex inside, `vertices - 2` triangles. `None` otherwise.
fn outline(triangles: &[[u32; 3]], piece: &Piece) -> Option<Vec<u32>> {
    if piece.triangles.len() + 2 != piece.vertices {
        return None;
    }
    let mut uses: HashMap<(u32, u32), u32> = HashMap::new();
    for &t in &piece.triangles {
        let [a, b, c] = triangles[t];
        if a == b || b == c || a == c {
            return None;
        }
        for (u, v) in [(a, b), (b, c), (c, a)] {
            *uses.entry((u.min(v), u.max(v))).or_insert(0) += 1;
        }
    }
    if uses.values().any(|&n| n > 2) {
        return None;
    }
    let mut next: HashMap<u32, u32> = HashMap::new();
    for &t in &piece.triangles {
        let [a, b, c] = triangles[t];
        for (u, v) in [(a, b), (b, c), (c, a)] {
            if uses[&(u.min(v), u.max(v))] == 1 && next.insert(u, v).is_some() {
                return None;
            }
        }
    }
    if next.len() != piece.vertices {
        return None;
    }
    let start = *next.keys().min().expect("non-empty");
    let mut outline = vec![start];
    let mut v = next[&start];
    while v != start {
        if outline.len() == piece.vertices {
            return None;
        }
        outline.push(v);
        v = *next.get(&v)?;
    }
    (outline.len() == piece.vertices).then_some(outline)
}

/// Each shape's outline points, in outline order, with its unit normal and
/// area; `None` for a shape with no area to speak of.
struct Measured {
    shapes: Vec<Option<Shape>>,
    /// The part's largest shape area. The search weighs reversed area
    /// against it rather than against each shape's own area, so it trades
    /// shapes off by the overdraw a viewer sees: a sliver-thin shape (a
    /// mouth resting nearly closed) does not outweigh an open one.
    largest: f64,
    /// Vertex index → position in the outline.
    position_of: HashMap<u32, usize>,
}

struct Shape {
    points: Vec<[f64; 3]>,
    normal: [f64; 3],
    area: f64,
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn measure(shapes: &[(Option<String>, Vec<[f64; 3]>)], outline: &[u32]) -> Measured {
    let mut measured: Vec<Option<Shape>> = shapes
        .iter()
        .map(|(_, all)| {
            let points: Vec<[f64; 3]> = outline.iter().map(|&v| all[v as usize]).collect();
            // Newell's method: the loop's vector area, whose direction is
            // the loop's normal for its winding.
            let mut n = [0.0; 3];
            for (i, &p) in points.iter().enumerate() {
                let q = points[(i + 1) % points.len()];
                let c = cross(p, q);
                n = [n[0] + c[0], n[1] + c[1], n[2] + c[2]];
            }
            let len = dot(n, n).sqrt();
            (len > 0.0).then(|| Shape {
                points,
                normal: [n[0] / len, n[1] / len, n[2] / len],
                area: len / 2.0,
            })
        })
        .collect();
    let largest = measured
        .iter()
        .flatten()
        .map(|s| s.area)
        .fold(0.0, f64::max);
    for shape in &mut measured {
        if shape
            .as_ref()
            .is_some_and(|s| s.area < DEGENERATE_AREA * largest)
        {
            *shape = None;
        }
    }
    Measured {
        shapes: measured,
        largest,
        position_of: outline.iter().enumerate().map(|(i, &v)| (v, i)).collect(),
    }
}

impl Measured {
    /// The area triangle `(i, j, k)` (outline positions, in winding order)
    /// covers with reversed winding in one shape.
    fn reversed(shape: &Shape, i: usize, j: usize, k: usize) -> f64 {
        let (a, b, c) = (shape.points[i], shape.points[j], shape.points[k]);
        let signed = dot(cross(sub(b, a), sub(c, a)), shape.normal) / 2.0;
        (-signed).max(0.0)
    }

    fn fold(shape: &Shape, i: usize, j: usize, k: usize) -> f64 {
        Self::reversed(shape, i, j, k) / shape.area
    }

    fn fold_per_shape(&self, triangles: &[[u32; 3]]) -> Vec<Option<f64>> {
        self.shapes
            .iter()
            .map(|shape| {
                shape.as_ref().map(|s| {
                    triangles
                        .iter()
                        .map(|t| {
                            let [i, j, k] = t.map(|v| self.position_of[&v]);
                            Self::fold(s, i, j, k)
                        })
                        .sum()
                })
            })
            .collect()
    }

    /// Reversed area summed over every shape, against the largest shape's
    /// area: the quantity [`Self::best_triangulation`] minimises.
    fn weighted_total(&self, triangles: &[[u32; 3]]) -> f64 {
        let shapes: Vec<&Shape> = self.shapes.iter().flatten().collect();
        triangles
            .iter()
            .map(|t| {
                let [i, j, k] = t.map(|v| self.position_of[&v]);
                shapes
                    .iter()
                    .map(|s| Self::reversed(s, i, j, k))
                    .sum::<f64>()
            })
            .sum::<f64>()
            / self.largest
    }

    /// The triangulation of the outline with the least reversed area summed
    /// over every shape, as vertex-index triangles in winding order.
    fn best_triangulation(&self, outline: &[u32]) -> Vec<[u32; 3]> {
        let n = outline.len();
        let shapes: Vec<&Shape> = self.shapes.iter().flatten().collect();
        let cost = |i: usize, j: usize, k: usize| -> f64 {
            shapes
                .iter()
                .map(|s| Self::reversed(s, i, j, k))
                .sum::<f64>()
                / self.largest
        };
        // best[i][j]: least cost of triangulating the sub-polygon from
        // outline position i to j; apex[i][j]: the third corner of the
        // triangle on edge (i, j) that achieves it.
        let mut best = vec![0.0f64; n * n];
        let mut apex = vec![0u32; n * n];
        for len in 2..n {
            for i in 0..n - len {
                let j = i + len;
                let mut least = f64::INFINITY;
                // A valid apex from the start, so the rebuild below always
                // terminates whatever the costs compare as.
                apex[i * n + j] = (i + 1) as u32;
                for k in i + 1..j {
                    let c = best[i * n + k] + best[k * n + j] + cost(i, k, j);
                    if c < least {
                        least = c;
                        apex[i * n + j] = k as u32;
                    }
                }
                best[i * n + j] = least;
            }
        }
        let mut triangles = Vec::with_capacity(n - 2);
        let mut stack = vec![(0usize, n - 1)];
        while let Some((i, j)) = stack.pop() {
            if j - i < 2 {
                continue;
            }
            let k = apex[i * n + j] as usize;
            triangles.push([outline[i], outline[k], outline[j]]);
            stack.push((i, k));
            stack.push((k, j));
        }
        triangles
    }
}
