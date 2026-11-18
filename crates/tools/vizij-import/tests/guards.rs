//! The parts the morph-triangle pass must leave alone, the inputs it must
//! survive, and the choices its search makes that only some shapes reveal.

mod common;

use common::{build, fan, flat, mouth, square, with_json, MeshSpec};
use serde_json::json;
use vizij_import::morph_triangles::{Outcome, Skip};
use vizij_import::optimize_morph_triangles;

fn only_outcome(bytes: &[u8]) -> Outcome {
    let optimized = optimize_morph_triangles(bytes).expect("the file parses");
    assert_eq!(optimized.report.parts.len(), 1, "{:?}", optimized.report);
    optimized.report.parts[0].outcome.clone()
}

fn skip_reason(bytes: &[u8]) -> Skip {
    match only_outcome(bytes) {
        Outcome::Skipped(why) => why,
        other => panic!("expected a skip, got {other:?}"),
    }
}

fn unreadable(bytes: &[u8]) -> String {
    match skip_reason(bytes) {
        Skip::Unreadable(why) => why,
        other => panic!("expected Unreadable, got {other:?}"),
    }
}

#[test]
fn a_part_with_no_area_in_any_shape_is_skipped() {
    let point = vec![[0.5, 0.5, 0.0]; 4];
    let spec = MeshSpec::new("dot", point.clone(), fan(4)).target("still_a_dot", point);
    assert_eq!(skip_reason(&build(&[spec]).bytes), Skip::NoArea);
}

#[test]
fn non_finite_positions_are_skipped() {
    let mut spec = square("square").target("t", flat(&[[0., 0.], [1., 0.], [1., 1.], [0., 1.]]));
    spec.targets[0].1[2][0] = f32::NAN;
    assert!(unreadable(&build(&[spec]).bytes).contains("not finite"));
}

#[test]
fn index_bytes_another_accessor_reads_are_left_alone() {
    // Mesh 1 has no targets, but its indices accessor reads the same bytes
    // as mesh 0's: rewriting mesh 0 would silently change mesh 1.
    let morphed = mouth(10);
    let plain = MeshSpec {
        targets: Vec::new(),
        ..mouth(10)
    };
    let built = build(&[morphed, plain]);
    let aliased = with_json(&built.bytes, |doc| {
        let copy = doc["accessors"][1].clone();
        doc["accessors"].as_array_mut().unwrap().push(copy);
        let last = doc["accessors"].as_array().unwrap().len() - 1;
        doc["meshes"][1]["primitives"][0]["indices"] = json!(last);
    });
    let optimized = optimize_morph_triangles(&aliased).unwrap();
    assert!(optimized.bytes.is_none());
    assert_eq!(
        optimized.report.parts[0].outcome,
        Outcome::Skipped(Skip::SharedIndices)
    );
}

#[test]
fn an_impossible_vertex_count_is_reported_not_allocated() {
    let built = build(&[mouth(6)]);
    let huge = with_json(&built.bytes, |doc| {
        doc["accessors"][0]["count"] = json!(1u64 << 40);
    });
    assert!(unreadable(&huge).contains("runs past"));
}

#[test]
fn an_overflowing_index_count_is_reported_not_panicked() {
    let built = build(&[mouth(6)]);
    let huge = with_json(&built.bytes, |doc| {
        doc["accessors"][1]["count"] = json!(u64::MAX / 2);
    });
    assert!(unreadable(&huge).contains("runs past"));
}

#[test]
fn a_target_count_unlike_the_mesh_is_reported() {
    let built = build(&[mouth(6)]);
    let short = with_json(&built.bytes, |doc| {
        doc["accessors"][2]["count"] = json!(3);
    });
    assert!(unreadable(&short).contains("not the mesh's"));
}

#[test]
fn primitives_the_pass_cannot_rewrite_are_left_alone() {
    let bytes = build(&[mouth(6)]).bytes;
    type Edit = fn(&mut serde_json::Value);
    let cases: [(Edit, Skip); 3] = [
        (
            |doc| {
                doc["meshes"][0]["primitives"][0]["extensions"] =
                    json!({ "KHR_draco_mesh_compression": {} })
            },
            Skip::Compressed,
        ),
        (
            |doc| doc["meshes"][0]["primitives"][0]["mode"] = json!(5),
            Skip::NotTriangles,
        ),
        (
            |doc| {
                doc["meshes"][0]["primitives"][0]
                    .as_object_mut()
                    .unwrap()
                    .remove("indices");
            },
            Skip::NotIndexed,
        ),
    ];
    for (edit, expected) in cases {
        assert_eq!(skip_reason(&with_json(&bytes, edit)), expected);
    }
}

#[test]
fn unreadable_data_is_reported_per_primitive() {
    let bytes = build(&[mouth(6)]).bytes;
    let external = with_json(&bytes, |doc| doc["buffers"][0]["uri"] = json!("face.bin"));
    assert!(unreadable(&external).contains("outside the GLB"));
    let quantized = with_json(&bytes, |doc| {
        doc["accessors"][0]["componentType"] = json!(5123)
    });
    assert!(unreadable(&quantized).contains("not 32-bit floats"));
    let sparse_indices = with_json(&bytes, |doc| {
        doc["accessors"][1]["sparse"] = json!({ "count": 0 });
    });
    assert!(unreadable(&sparse_indices).contains("sparse"));
}

#[test]
fn a_moebius_strip_is_not_taken_for_a_polygon() {
    // Five triangles around a five-vertex Möbius strip: one boundary loop
    // through every vertex, but five triangles where a polygon has three.
    let points = flat(&[[0., 0.], [1., 0.], [2., 0.], [1., 1.], [0., 1.]]);
    let triangles = vec![[0, 1, 3], [1, 2, 4], [2, 3, 0], [3, 4, 1], [4, 0, 2]];
    let spec = MeshSpec::new("strip", points.clone(), triangles).target("t", points);
    assert_eq!(skip_reason(&build(&[spec]).bytes), Skip::NotAPolygon);
}

#[test]
fn an_outline_longer_than_the_search_takes_is_left_alone() {
    let n = 257;
    let at = |i: usize| (i as f32) * std::f32::consts::TAU / n as f32;
    let circle: Vec<[f32; 3]> = (0..n).map(|i| [at(i).cos(), at(i).sin(), 0.0]).collect();
    let squashed = circle.iter().map(|p| [p[0], 0.1 * p[1], 0.0]).collect();
    let spec = MeshSpec {
        index_size: 2,
        ..MeshSpec::new("big", circle, fan(n as u32)).target("t", squashed)
    };
    assert_eq!(skip_reason(&build(&[spec]).bytes), Skip::TooManyVertices);
}

#[test]
fn halfway_shapes_decide_when_full_weight_ones_cannot() {
    // Both diagonals are unfolded at rest and at full weight; only the
    // halfway shape folds under the 0–2 diagonal the fan uses.
    let spec = square("quad").target(
        "turn",
        flat(&[[1.4, 0.3], [-1.0, 0.8], [0.5, -1.8], [1.0, -1.1]]),
    );
    let built = build(std::slice::from_ref(&spec));
    let optimized = optimize_morph_triangles(&built.bytes).unwrap();
    let rewritten = optimized.bytes.expect("the halfway fold is removed");
    assert_ne!(built.triangles(&rewritten, 0), spec.triangles);
    let halfway = optimized.report.parts[0]
        .folds
        .iter()
        .find(|f| f.shape == "turn (halfway)")
        .expect("halfway shapes are reported");
    assert!(halfway.before.unwrap() > 0.4 && halfway.after.unwrap() < 1e-9);
}

#[test]
fn shapes_are_weighed_by_the_area_a_viewer_sees() {
    // Each diagonal folds in one target. Measured against each shape's own
    // area the fan's diagonal costs less; measured against the part's
    // largest shape — the overdraw on screen — the other one does.
    let spec = square("quad")
        .target(
            "a",
            flat(&[[-0.41, 0.48], [0.4, -0.37], [0.07, 0.53], [1.22, -0.01]]),
        )
        .target(
            "b",
            flat(&[[-0.41, 1.43], [-1.33, 1.0], [0.55, 0.17], [-0.16, 0.75]]),
        );
    let built = build(std::slice::from_ref(&spec));
    let rewritten = optimize_morph_triangles(&built.bytes)
        .unwrap()
        .bytes
        .expect("the other diagonal draws less on screen");
    assert_ne!(built.triangles(&rewritten, 0), spec.triangles);
}

#[test]
fn a_shape_collapsed_to_a_line_reports_no_area() {
    // The mouth closed to a near-line: its normal is noise, so a fold
    // measured against it would be too.
    let closed = mouth(6)
        .positions
        .iter()
        .map(|p| [p[0], p[1] * 1e-6, p[2]])
        .collect();
    let spec = mouth(6).target("closed", closed);
    let optimized = optimize_morph_triangles(&build(&[spec]).bytes).unwrap();
    let closed = optimized.report.parts[0]
        .folds
        .iter()
        .find(|f| f.shape == "closed")
        .unwrap();
    assert_eq!((closed.before, closed.after), (None, None));
}
