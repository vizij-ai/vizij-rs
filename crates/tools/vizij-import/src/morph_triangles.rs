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
//! the rest shape, each target and each pair of targets at full and at half
//! weight, and the mesh's default pose. These are samples of the shapes an
//! animation passes through, not a guarantee: a triangle can stay unfolded
//! at the sampled weights and still fold at one in between, or in a blend
//! of three targets. [`Mode`] sets whether a rewrite may make any one
//! checked shape worse. It
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
//! triangle count, so only the index data changes. A shape that bends
//! further than any triangulation of its vertices can follow still folds
//! afterwards, and the report gives its fold so the part can be remodelled.
//! A shape with almost no net area — collapsed to a line, or an outline
//! that crosses itself so its loops cancel — is still checked, measured
//! against the part's main facing direction, but the report gives it no
//! fold, having no area of its own to give it relative to.

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

/// The most triangle-and-shape evaluations one part's search may take:
/// every triangle of the outline (about n³/6) against every shape checked.
/// The search runs synchronously, in the browser on the main thread, and
/// this keeps one part to a few seconds there.
const MAX_EVALUATIONS: f64 = 3e8;

/// In [`Mode::Safe`], a rewrite is not written when it would add more than
/// this much reversed area to any one checked shape, measured against the
/// part's largest shape: no shape the pass checks comes out visibly worse,
/// even when the shapes as a whole would improve.
const VISIBLY_WORSE: f64 = 0.01;

/// A shape that still has more than this much reversed area after the
/// pass, relative to its own area, is one no triangulation of its vertices
/// follows: the part needs remodelling there.
pub const STILL_FOLDS: f64 = 0.01;

/// How far the pass may go for a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Rewrite a part only when no checked shape — rest, each target at
    /// full and half weight, every pair of targets at full and half weight,
    /// the default pose — comes out visibly worse: no shape may gain more
    /// wrongly drawn area than 1% of the part's largest shape. Fixes less.
    #[default]
    Safe,
    /// Rewrite a part whenever the checked shapes improve overall, even if
    /// some get worse; the report shows which. Fixes more.
    Overall,
}

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
    /// Its triangles were left as they were: no triangulation folds
    /// noticeably less, or the one that does would make some checked shape
    /// visibly worse.
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
    /// The outline and the shapes to check (every target and every pair of
    /// targets) are together more than the search takes.
    TooManyShapes,
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
            Skip::TooManyShapes => f.write_str(
                "has too many outline vertices and morph targets to check every blend",
            ),
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
    /// `rest`; a morph target's name (`target N` without one); two names
    /// joined by ` + ` for a blend of both targets; either followed by
    /// ` (halfway)` at half weight; or `default pose`.
    pub shape: String,
    pub kind: ShapeKind,
    pub before: Option<f64>,
    pub after: Option<f64>,
}

/// Which shape of a part a [`ShapeFold`] measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    /// No morph target applied.
    Rest,
    /// One morph target at full weight.
    Target,
    /// One morph target at half weight.
    TargetHalfway,
    /// Two morph targets together, both at full weight.
    Blend,
    /// Two morph targets together, both at half weight.
    BlendHalfway,
    /// The mesh's default weights, when any is set: what it shows undriven.
    DefaultPose,
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
pub(crate) fn optimize(json: &Value, bin: &mut [u8], mode: Mode) -> Report {
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
                mode,
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
    mode: Mode,
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
    // Rest first; then each target at full weight, at half weight; every
    // pair of targets at full and at half weight; and the default pose.
    // Each shape is a recipe of weighted targets, worked out only for the
    // outline of each part as it is measured: the pairs grow with the
    // square of the target count, and a full copy of the primitive's
    // vertices for each would outgrow memory on a large face.
    let mut deltas: Vec<Vec<[f32; 3]>> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut default_terms: Vec<(usize, f64)> = Vec::new();
    let targets = primitive
        .get("targets")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    for (t, target) in targets.enumerate() {
        names.push(
            target_names
                .get(t)
                .filter(|n| !n.is_empty())
                .cloned()
                .unwrap_or_else(|| format!("target {t}")),
        );
        deltas.push(match target.get("POSITION").and_then(Value::as_u64) {
            Some(acc) => doc
                .read_vec3(acc, Some(base.len()))
                .map_err(|u| fail(u.into()))?,
            None => vec![[0.0; 3]; base.len()],
        });
        let weight = default_weights
            .and_then(|w| w.get(t))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if weight != 0.0 {
            default_terms.push((t, weight));
        }
    }
    if base
        .iter()
        .chain(deltas.iter().flatten())
        .flatten()
        .any(|c| !c.is_finite())
        || default_terms.iter().any(|(_, w)| !w.is_finite())
    {
        return Err(fail(Skip::Unreadable(
            "positions or morph deltas are not finite".into(),
        )));
    }
    let mut shapes = vec![Recipe::new("rest".into(), ShapeKind::Rest, vec![])];
    for (t, name) in names.iter().enumerate() {
        shapes.push(Recipe::new(name.clone(), ShapeKind::Target, vec![(t, 1.0)]));
    }
    for (t, name) in names.iter().enumerate() {
        shapes.push(Recipe::new(
            format!("{name} (halfway)"),
            ShapeKind::TargetHalfway,
            vec![(t, 0.5)],
        ));
    }
    for (a, first) in names.iter().enumerate() {
        for (b, second) in names.iter().enumerate().skip(a + 1) {
            let pair = format!("{first} + {second}");
            shapes.push(Recipe::new(
                pair.clone(),
                ShapeKind::Blend,
                vec![(a, 1.0), (b, 1.0)],
            ));
            shapes.push(Recipe::new(
                format!("{pair} (halfway)"),
                ShapeKind::BlendHalfway,
                vec![(a, 0.5), (b, 0.5)],
            ));
        }
    }
    if !default_terms.is_empty() {
        shapes.push(Recipe::new(
            "default pose".into(),
            ShapeKind::DefaultPose,
            default_terms,
        ));
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
        let n = outline.len() as f64;
        if n * n * n / 6.0 * shapes.len() as f64 > MAX_EVALUATIONS {
            parts.push(part(Outcome::Skipped(Skip::TooManyShapes), Vec::new()));
            continue;
        }
        let measured = measure(&shapes, &base, &deltas, &outline);
        if measured.largest <= 0.0 {
            parts.push(part(Outcome::Skipped(Skip::NoArea), Vec::new()));
            continue;
        }
        let current: Vec<[u32; 3]> = piece.triangles.iter().map(|&t| triangles[t]).collect();
        let before = measured.fold_per_shape(&current);
        let chosen = measured.best_triangulation(&outline);
        let after = measured.fold_per_shape(&chosen);
        let harms_a_shape =
            mode == Mode::Safe && measured.worsens_any(&current, &chosen, VISIBLY_WORSE);
        let improved = !harms_a_shape
            && measured.weighted_total(&current) - measured.weighted_total(&chosen) > IMPROVEMENT;
        let folds = shapes
            .iter()
            .zip(before.iter().zip(&after))
            .map(|(recipe, (b, a))| ShapeFold {
                shape: recipe.name.clone(),
                kind: recipe.kind,
                before: *b,
                after: if improved { *a } else { *b },
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

/// A shape the pass checks: the rest shape plus each target scaled by its
/// weight.
struct Recipe {
    name: String,
    kind: ShapeKind,
    terms: Vec<(usize, f64)>,
}

impl Recipe {
    fn new(name: String, kind: ShapeKind, terms: Vec<(usize, f64)>) -> Self {
        Recipe { name, kind, terms }
    }

    /// This shape's position of `vertex`.
    fn at(&self, base: &[[f32; 3]], deltas: &[Vec<[f32; 3]>], vertex: usize) -> [f64; 3] {
        std::array::from_fn(|k| {
            base[vertex][k] as f64
                + self
                    .terms
                    .iter()
                    .map(|&(t, w)| w * deltas[t][vertex][k] as f64)
                    .sum::<f64>()
        })
    }
}

/// Each shape's outline points, in outline order, with the normal its
/// winding is measured against and its area.
struct Measured {
    shapes: Vec<Shape>,
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
    /// The shape has almost no net area: its outline collapsed to a line, or
    /// crosses itself so that its loops cancel. Its own normal is noise, so
    /// it is measured against the part's largest shape's normal instead,
    /// and it has no fold to report relative to its own area.
    degenerate: bool,
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

fn measure(
    shapes: &[Recipe],
    base: &[[f32; 3]],
    deltas: &[Vec<[f32; 3]>],
    outline: &[u32],
) -> Measured {
    let mut measured: Vec<Shape> = shapes
        .iter()
        .map(|recipe| {
            let points: Vec<[f64; 3]> = outline
                .iter()
                .map(|&v| recipe.at(base, deltas, v as usize))
                .collect();
            // Newell's method: the loop's vector area, whose direction is
            // the loop's normal for its winding.
            let mut n = [0.0; 3];
            for (i, &p) in points.iter().enumerate() {
                let q = points[(i + 1) % points.len()];
                let c = cross(p, q);
                n = [n[0] + c[0], n[1] + c[1], n[2] + c[2]];
            }
            let len = dot(n, n).sqrt();
            let normal = if len > 0.0 {
                [n[0] / len, n[1] / len, n[2] / len]
            } else {
                [0.0; 3]
            };
            Shape {
                points,
                normal,
                area: len / 2.0,
                degenerate: false,
            }
        })
        .collect();
    let (largest, reference) = measured.iter().fold((0.0, [0.0; 3]), |(area, normal), s| {
        if s.area > area {
            (s.area, s.normal)
        } else {
            (area, normal)
        }
    });
    for shape in &mut measured {
        if shape.area < DEGENERATE_AREA * largest {
            shape.degenerate = true;
            shape.normal = reference;
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

    /// Reversed area relative to the shape's own area; `None` for a
    /// degenerate shape, which has none to be relative to.
    fn fold_per_shape(&self, triangles: &[[u32; 3]]) -> Vec<Option<f64>> {
        self.shapes
            .iter()
            .map(|s| {
                (!s.degenerate).then(|| {
                    triangles
                        .iter()
                        .map(|t| {
                            let [i, j, k] = t.map(|v| self.position_of[&v]);
                            Self::reversed(s, i, j, k) / s.area
                        })
                        .sum()
                })
            })
            .collect()
    }

    /// Whether `chosen` adds more than `limit` (against the largest shape's
    /// area) of reversed area to any single shape that `current` draws.
    fn worsens_any(&self, current: &[[u32; 3]], chosen: &[[u32; 3]], limit: f64) -> bool {
        let reversed = |s: &Shape, triangles: &[[u32; 3]]| -> f64 {
            triangles
                .iter()
                .map(|t| {
                    let [i, j, k] = t.map(|v| self.position_of[&v]);
                    Self::reversed(s, i, j, k)
                })
                .sum()
        };
        self.shapes
            .iter()
            .any(|s| reversed(s, chosen) - reversed(s, current) > limit * self.largest)
    }

    /// Reversed area summed over every shape, against the largest shape's
    /// area: the quantity [`Self::best_triangulation`] minimises.
    fn weighted_total(&self, triangles: &[[u32; 3]]) -> f64 {
        let shapes = &self.shapes;
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
        let shapes = &self.shapes;
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
