//! Builds small face-like GLB files in code, so the tests exercise the pass
//! on shapes whose correct answer is known.

#![allow(dead_code)]

use serde_json::{json, Value};
use vizij_glb_migrate::glb::{Chunk, Glb, CHUNK_BIN};

/// How a mesh stores its morph deltas.
#[derive(Clone, Copy, PartialEq)]
pub enum Deltas {
    Dense,
    /// Only the vertices a target moves, over an accessor with no
    /// bufferView, as exporters write them.
    Sparse,
}

/// One mesh primitive: vertex positions, triangles, and morph targets given
/// as full target positions (the builder stores them as deltas).
pub struct MeshSpec {
    pub name: &'static str,
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub targets: Vec<(&'static str, Vec<[f32; 3]>)>,
    /// 1, 2 or 4 bytes per index.
    pub index_size: usize,
    pub deltas: Deltas,
}

impl MeshSpec {
    pub fn new(name: &'static str, positions: Vec<[f32; 3]>, triangles: Vec<[u32; 3]>) -> Self {
        MeshSpec {
            name,
            positions,
            triangles,
            targets: Vec::new(),
            index_size: 2,
            deltas: Deltas::Dense,
        }
    }

    pub fn target(mut self, name: &'static str, positions: Vec<[f32; 3]>) -> Self {
        self.targets.push((name, positions));
        self
    }
}

/// Where each mesh's index data sits in the binary chunk, to read back.
pub struct Built {
    pub bytes: Vec<u8>,
    index_ranges: Vec<(usize, usize, usize)>,
}

impl Built {
    /// The triangles of mesh `m` as stored in `bytes` (which may be a
    /// rewritten copy of [`Built::bytes`]).
    pub fn triangles(&self, bytes: &[u8], m: usize) -> Vec<[u32; 3]> {
        let bin = &Glb::parse(bytes).expect("valid GLB").tail[0].data;
        let (offset, count, size) = self.index_ranges[m];
        let read = |i: usize| -> u32 {
            let at = offset + i * size;
            match size {
                1 => bin[at] as u32,
                2 => u16::from_le_bytes([bin[at], bin[at + 1]]) as u32,
                _ => u32::from_le_bytes(bin[at..at + 4].try_into().unwrap()),
            }
        };
        (0..count / 3)
            .map(|t| [read(3 * t), read(3 * t + 1), read(3 * t + 2)])
            .collect()
    }
}

fn align(bin: &mut Vec<u8>) {
    while !bin.len().is_multiple_of(4) {
        bin.push(0);
    }
}

fn push_vec3s(bin: &mut Vec<u8>, points: &[[f32; 3]]) -> usize {
    align(bin);
    let offset = bin.len();
    for p in points {
        for c in p {
            bin.extend_from_slice(&c.to_le_bytes());
        }
    }
    offset
}

/// Builds a GLB holding `meshes`, one node each.
pub fn build(meshes: &[MeshSpec]) -> Built {
    let mut bin = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut json_meshes = Vec::new();
    let mut index_ranges = Vec::new();
    let mut view = |bin: &[u8], offset: usize| -> usize {
        views.push(json!({ "buffer": 0, "byteOffset": offset, "byteLength": bin.len() - offset }));
        views.len() - 1
    };
    for mesh in meshes {
        let n = mesh.positions.len();
        let offset = push_vec3s(&mut bin, &mesh.positions);
        let v = view(&bin, offset);
        accessors
            .push(json!({ "bufferView": v, "componentType": 5126, "count": n, "type": "VEC3" }));
        let position = accessors.len() - 1;

        align(&mut bin);
        let offset = bin.len();
        for t in &mesh.triangles {
            for &i in t {
                match mesh.index_size {
                    1 => bin.push(i as u8),
                    2 => bin.extend_from_slice(&(i as u16).to_le_bytes()),
                    _ => bin.extend_from_slice(&i.to_le_bytes()),
                }
            }
        }
        index_ranges.push((offset, mesh.triangles.len() * 3, mesh.index_size));
        let v = view(&bin, offset);
        let component = match mesh.index_size {
            1 => 5121,
            2 => 5123,
            _ => 5125,
        };
        accessors.push(json!({
            "bufferView": v, "componentType": component,
            "count": mesh.triangles.len() * 3, "type": "SCALAR"
        }));
        let indices = accessors.len() - 1;

        let mut targets = Vec::new();
        for (_, target) in &mesh.targets {
            let deltas: Vec<[f32; 3]> = target
                .iter()
                .zip(&mesh.positions)
                .map(|(t, p)| [t[0] - p[0], t[1] - p[1], t[2] - p[2]])
                .collect();
            let accessor = match mesh.deltas {
                Deltas::Dense => {
                    let offset = push_vec3s(&mut bin, &deltas);
                    let v = view(&bin, offset);
                    json!({ "bufferView": v, "componentType": 5126, "count": n, "type": "VEC3" })
                }
                Deltas::Sparse => {
                    let moved: Vec<usize> = (0..n).filter(|&i| deltas[i] != [0.0; 3]).collect();
                    align(&mut bin);
                    let offset = bin.len();
                    for &i in &moved {
                        bin.extend_from_slice(&(i as u32).to_le_bytes());
                    }
                    let indices_view = view(&bin, offset);
                    let values: Vec<[f32; 3]> = moved.iter().map(|&i| deltas[i]).collect();
                    let offset = push_vec3s(&mut bin, &values);
                    let values_view = view(&bin, offset);
                    json!({
                        "componentType": 5126, "count": n, "type": "VEC3",
                        "sparse": {
                            "count": moved.len(),
                            "indices": { "bufferView": indices_view, "componentType": 5125 },
                            "values": { "bufferView": values_view }
                        }
                    })
                }
            };
            accessors.push(accessor);
            targets.push(json!({ "POSITION": accessors.len() - 1 }));
        }
        let mut primitive = json!({ "attributes": { "POSITION": position }, "indices": indices });
        if !targets.is_empty() {
            primitive["targets"] = Value::Array(targets);
        }
        let names: Vec<&str> = mesh.targets.iter().map(|(name, _)| *name).collect();
        json_meshes.push(json!({
            "name": mesh.name,
            "primitives": [primitive],
            "extras": { "targetNames": names }
        }));
    }
    align(&mut bin);
    let nodes: Vec<Value> = (0..meshes.len()).map(|m| json!({ "mesh": m })).collect();
    let document = json!({
        "asset": { "version": "2.0" },
        "scene": 0,
        "scenes": [{ "nodes": (0..meshes.len()).collect::<Vec<_>>() }],
        "nodes": nodes,
        "meshes": json_meshes,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{ "byteLength": bin.len() }]
    });
    let bytes = Glb {
        version: 2,
        json: serde_json::to_vec(&document).unwrap(),
        tail: vec![Chunk {
            kind: CHUNK_BIN,
            data: bin,
        }],
    }
    .to_bytes();
    Built {
        bytes,
        index_ranges,
    }
}

/// A fan over vertices `0..n` from vertex 0: what an exporter often
/// produces for a large polygon, and folds once the outline bends.
pub fn fan(n: u32) -> Vec<[u32; 3]> {
    (1..n - 1).map(|i| [0, i, i + 1]).collect()
}

/// A mouth: an outline of `2m` vertices, the upper lip left to right then
/// the lower lip right to left, resting almost closed. Its `smile` target
/// bends the band into a crescent that no fan can follow.
pub fn mouth(m: usize) -> MeshSpec {
    let band = |bend: f32, half_width: f32| -> Vec<[f32; 3]> {
        let xs: Vec<f32> = (0..m)
            .map(|i| -1.0 + 2.0 * i as f32 / (m - 1) as f32)
            .collect();
        let centre = |x: f32| bend * (x * x - 1.0);
        let upper = xs.iter().map(|&x| [x, centre(x) + half_width, 0.0]);
        let lower = xs.iter().rev().map(|&x| [x, centre(x) - half_width, 0.0]);
        upper.chain(lower).collect()
    };
    // Counter-clockwise seen from +z: upper lip right to left, lower lip
    // left to right. Reverse to match.
    let rest: Vec<[f32; 3]> = band(0.0, 0.02).into_iter().rev().collect();
    let smile: Vec<[f32; 3]> = band(0.6, 0.08).into_iter().rev().collect();
    MeshSpec::new("mouth", rest, fan(2 * m as u32)).target("smile_shape", smile)
}

/// A pupil: a circle of `n` vertices whose `closed` target squashes it into
/// a thin arc, as a closing eye does.
pub fn pupil(n: usize) -> MeshSpec {
    let at = |i: usize| (i as f32) * std::f32::consts::TAU / n as f32;
    let circle: Vec<[f32; 3]> = (0..n).map(|i| [at(i).cos(), at(i).sin(), 0.0]).collect();
    let arc: Vec<[f32; 3]> = (0..n)
        .map(|i| {
            let x = at(i).cos();
            [x, 0.4 * (1.0 - x * x) + 0.05 * at(i).sin(), 0.0]
        })
        .collect();
    MeshSpec::new("pupil", circle, fan(n as u32)).target("closed", arc)
}

/// A disk with a centre vertex: triangles authored around an interior
/// vertex, which the pass must leave alone.
pub fn hub(n: usize) -> MeshSpec {
    let at = |i: usize| (i as f32) * std::f32::consts::TAU / n as f32;
    let mut points = vec![[0.0, 0.0, 0.0]];
    points.extend((0..n).map(|i| [at(i).cos(), at(i).sin(), 0.0]));
    let triangles = (0..n as u32)
        .map(|i| [0, 1 + i, 1 + (i + 1) % n as u32])
        .collect();
    let squashed = points.iter().map(|p| [p[0], 0.2 * p[1], 0.0]).collect();
    MeshSpec::new("hub", points, triangles).target("squash", squashed)
}

/// The area covered with reversed winding (seen from +z) by `triangles`
/// over `points`, as a fraction of the outline's area.
pub fn reversed_fraction(points: &[[f32; 3]], triangles: &[[u32; 3]]) -> f64 {
    let signed = |t: &[u32; 3]| -> f64 {
        let [a, b, c] = t.map(|i| points[i as usize]);
        0.5 * ((b[0] - a[0]) as f64 * (c[1] - a[1]) as f64
            - (b[1] - a[1]) as f64 * (c[0] - a[0]) as f64)
    };
    let total: f64 = triangles.iter().map(signed).sum();
    let reversed: f64 = triangles.iter().map(|t| (-signed(t)).max(0.0)).sum();
    reversed / total.abs()
}

/// `bytes` with its JSON chunk replaced by `edit` applied to the document;
/// the binary chunk is kept as it is.
pub fn with_json(bytes: &[u8], edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let mut glb = Glb::parse(bytes).expect("valid GLB");
    let mut document: Value = serde_json::from_slice(&glb.json).unwrap();
    edit(&mut document);
    glb.json = serde_json::to_vec(&document).unwrap();
    glb.to_bytes()
}

/// A unit square split along its 0–2 diagonal, as a fan from vertex 0.
pub fn square(name: &'static str) -> MeshSpec {
    let points = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    MeshSpec::new(name, points, fan(4))
}

/// `[x, y]` pairs as positions on the z = 0 plane.
pub fn flat(points: &[[f32; 2]]) -> Vec<[f32; 3]> {
    points.iter().map(|p| [p[0], p[1], 0.0]).collect()
}
