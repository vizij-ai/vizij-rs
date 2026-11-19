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

/// Points on a circle, one per vertex, so any triangles over them have area.
fn ring(n: usize) -> Vec<[f32; 3]> {
    let at = |i: usize| (i as f32) * std::f32::consts::TAU / n as f32;
    (0..n).map(|i| [at(i).cos(), at(i).sin(), 0.0]).collect()
}

fn squashed(points: &[[f32; 3]]) -> Vec<[f32; 3]> {
    points.iter().map(|p| [p[0], 0.3 * p[1], p[2]]).collect()
}

#[test]
fn an_index_accessor_another_primitive_also_uses_is_left_alone() {
    let built = build(&[mouth(6), mouth(6)]);
    let shared = with_json(&built.bytes, |doc| {
        doc["meshes"][1]["primitives"][0]["indices"] = json!(1);
    });
    let optimized = optimize_morph_triangles(&shared).unwrap();
    assert!(optimized.bytes.is_none());
    assert!(optimized
        .report
        .parts
        .iter()
        .all(|p| p.outcome == Outcome::Skipped(Skip::SharedIndices)));
}

#[test]
fn an_index_accessor_also_used_as_an_attribute_is_left_alone() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["meshes"][0]["primitives"][0]["attributes"]["_FEATURE_ID_0"] = json!(1);
    });
    assert_eq!(skip_reason(&bytes), Skip::SharedIndices);
}

#[test]
fn index_bytes_an_image_reads_are_left_alone() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["images"] = json!([{ "bufferView": 1, "mimeType": "image/png" }]);
    });
    assert_eq!(skip_reason(&bytes), Skip::SharedIndices);
}

#[test]
fn index_bytes_another_accessors_sparse_indices_read_are_left_alone() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["accessors"].as_array_mut().unwrap().push(json!({
            "componentType": 5126, "count": 12, "type": "VEC3",
            "sparse": {
                "count": 2,
                "indices": { "bufferView": 1, "componentType": 5123 },
                "values": { "bufferView": 0 }
            }
        }));
    });
    assert_eq!(skip_reason(&bytes), Skip::SharedIndices);
}

#[test]
fn index_accessors_packed_side_by_side_in_one_view_are_both_rewritten() {
    // Each accessor's exact range counts, not the whole view they share.
    let built = build(&[mouth(6), mouth(6)]);
    let packed = with_json(&built.bytes, |doc| {
        let first = doc["accessors"][1]["bufferView"].as_u64().unwrap() as usize;
        let views = doc["bufferViews"].as_array().unwrap().clone();
        let second_accessor = doc["meshes"][1]["primitives"][0]["indices"]
            .as_u64()
            .unwrap() as usize;
        let second = doc["accessors"][second_accessor]["bufferView"]
            .as_u64()
            .unwrap() as usize;
        let start = views[first]["byteOffset"].as_u64().unwrap();
        let end = views[second]["byteOffset"].as_u64().unwrap()
            + views[second]["byteLength"].as_u64().unwrap();
        doc["bufferViews"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "buffer": 0, "byteOffset": start, "byteLength": end - start }));
        let wide = doc["bufferViews"].as_array().unwrap().len() - 1;
        doc["accessors"][1]["bufferView"] = json!(wide);
        doc["accessors"][second_accessor]["bufferView"] = json!(wide);
        doc["accessors"][second_accessor]["byteOffset"] =
            json!(views[second]["byteOffset"].as_u64().unwrap() - start);
    });
    let optimized = optimize_morph_triangles(&packed).unwrap();
    let outcomes: Vec<_> = optimized.report.parts.iter().map(|p| &p.outcome).collect();
    assert_eq!(
        outcomes,
        [&Outcome::Retriangulated, &Outcome::Retriangulated]
    );
}

#[test]
fn a_buffer_other_than_the_binary_chunk_is_not_read() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        let length = doc["buffers"][0]["byteLength"].clone();
        doc["buffers"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "byteLength": length }));
        doc["bufferViews"][1]["buffer"] = json!(1);
    });
    assert!(unreadable(&bytes).contains("outside the GLB"));
}

#[test]
fn an_index_past_the_vertices_is_reported_not_panicked() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["accessors"][0]["count"] = json!(3);
    });
    assert!(unreadable(&bytes).contains("do not form triangles"));
}

#[test]
fn a_view_past_the_binary_chunk_is_reported_not_panicked() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["bufferViews"][0]["byteLength"] = json!(1_000_000);
    });
    assert!(unreadable(&bytes).contains("past the binary chunk"));
}

#[test]
fn positions_without_a_view_are_reported_not_allocated() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["accessors"][0]
            .as_object_mut()
            .unwrap()
            .remove("bufferView");
        doc["accessors"][0]["count"] = json!(1u64 << 36);
    });
    assert!(unreadable(&bytes).contains("no bufferView"));
}

#[test]
fn outlines_that_are_not_one_loop_are_not_polygons() {
    // Found by fuzzing the outline walk: in the first, two triangles leave
    // the same vertex along the boundary; in the second, the boundary closes
    // a loop that misses some of the piece's vertices.
    for triangles in [
        vec![[1, 3, 0], [3, 5, 2], [1, 4, 5], [5, 7, 3], [4, 1, 2]],
        vec![[0, 2, 6], [5, 6, 2], [3, 6, 4], [5, 0, 6]],
    ] {
        let points = ring(8);
        let spec = MeshSpec::new("odd", points.clone(), triangles).target("t", squashed(&points));
        assert_eq!(skip_reason(&build(&[spec]).bytes), Skip::NotAPolygon);
    }
}

#[test]
fn a_lone_triangle_is_not_considered() {
    let points = ring(3);
    let spec = MeshSpec::new("tri", points.clone(), vec![[0, 1, 2]]).target("t", squashed(&points));
    let optimized = optimize_morph_triangles(&build(&[spec]).bytes).unwrap();
    assert!(optimized.report.parts.is_empty());
}

#[test]
fn a_part_kept_reports_its_triangles_as_they_are() {
    let once = optimize_morph_triangles(&build(&[mouth(10)]).bytes)
        .unwrap()
        .bytes
        .unwrap();
    let twice = optimize_morph_triangles(&once).unwrap();
    let part = &twice.report.parts[0];
    assert_eq!(part.outcome, Outcome::Kept);
    assert!(part.folds.iter().all(|f| f.before == f.after), "{part:?}");
}

#[test]
fn the_default_pose_is_checked_when_the_mesh_sets_one() {
    let bytes = with_json(&build(&[mouth(6)]).bytes, |doc| {
        doc["meshes"][0]["weights"] = json!([0.5]);
    });
    let optimized = optimize_morph_triangles(&bytes).unwrap();
    assert!(optimized.report.parts[0]
        .folds
        .iter()
        .any(|f| f.shape == "default pose"));
}

#[test]
fn a_gain_too_small_to_see_keeps_the_triangles_and_says_so() {
    // The target pushes vertex 3 just across the fan's 0–2 diagonal: that
    // diagonal folds a sliver the other one would not, but the sliver is
    // far below what a viewer notices.
    let spec = square("quad").target(
        "nudge",
        flat(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.5005, 0.4995]]),
    );
    let optimized = optimize_morph_triangles(&build(&[spec]).bytes).unwrap();
    assert!(optimized.bytes.is_none());
    let part = &optimized.report.parts[0];
    assert_eq!(part.outcome, Outcome::Kept);
    let nudge = part.folds.iter().find(|f| f.shape == "nudge").unwrap();
    assert!(nudge.before.unwrap() > 0.0);
    assert_eq!(nudge.before, nudge.after);
}
