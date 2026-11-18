//! The morph-triangle pass over synthetic faces whose correct answer is
//! known: a mouth that bends into a crescent and a pupil that squashes into
//! an arc can both be triangulated so no shape folds.

mod common;

use common::{build, hub, mouth, pupil, reversed_fraction, Deltas, MeshSpec};
use vizij_glb_migrate::glb::Glb;
use vizij_import::morph_triangles::{Outcome, Skip};
use vizij_import::optimize_morph_triangles;

/// Every shape of `spec` — rest and each target — folds by less than
/// `tolerance` under `triangles`.
fn assert_unfolded(spec: &MeshSpec, triangles: &[[u32; 3]], tolerance: f64) {
    let shapes =
        std::iter::once(("rest", &spec.positions)).chain(spec.targets.iter().map(|(n, p)| (*n, p)));
    for (name, points) in shapes {
        let folded = reversed_fraction(points, triangles);
        assert!(folded < tolerance, "{name} folds {folded}");
    }
}

#[test]
fn a_fan_folds_where_the_outline_bends() {
    // The premise the pass exists for: the exported fan folds in the
    // targets, not at rest.
    for spec in [mouth(12), pupil(16)] {
        assert!(reversed_fraction(&spec.positions, &spec.triangles) < 1e-9);
        let (_, target) = &spec.targets[0];
        assert!(reversed_fraction(target, &spec.triangles) > 0.05);
    }
}

#[test]
fn bent_and_squashed_parts_are_retriangulated_unfolded() {
    let specs = [mouth(12), pupil(16)];
    let built = build(&specs);
    let optimized = optimize_morph_triangles(&built.bytes).unwrap();
    let rewritten = optimized.bytes.expect("the fans fold, so the file changes");
    for (m, spec) in specs.iter().enumerate() {
        let triangles = built.triangles(&rewritten, m);
        assert_eq!(triangles.len(), spec.triangles.len(), "triangle count kept");
        assert_unfolded(spec, &triangles, 1e-9);
    }
    let outcomes: Vec<_> = optimized.report.parts.iter().map(|p| &p.outcome).collect();
    assert_eq!(
        outcomes,
        [&Outcome::Retriangulated, &Outcome::Retriangulated]
    );
    let smile = &optimized.report.parts[0].folds[1];
    assert_eq!(smile.shape, "smile_shape");
    assert!(smile.before.unwrap() > 0.05 && smile.after.unwrap() < 1e-9);
}

#[test]
fn only_index_bytes_change() {
    let built = build(&[mouth(10)]);
    let rewritten = optimize_morph_triangles(&built.bytes)
        .unwrap()
        .bytes
        .unwrap();
    let before = Glb::parse(&built.bytes).unwrap();
    let after = Glb::parse(&rewritten).unwrap();
    assert_eq!(before.json, after.json, "JSON chunk untouched");
    assert_eq!(rewritten.len(), built.bytes.len());
    // The positions occupy the first bytes of the binary chunk.
    let positions = 20 * 12;
    assert_eq!(
        before.tail[0].data[..positions],
        after.tail[0].data[..positions]
    );
}

#[test]
fn a_second_run_changes_nothing() {
    let built = build(&[mouth(10), pupil(12)]);
    let once = optimize_morph_triangles(&built.bytes)
        .unwrap()
        .bytes
        .unwrap();
    let twice = optimize_morph_triangles(&once).unwrap();
    assert!(twice.bytes.is_none());
    assert!(twice
        .report
        .parts
        .iter()
        .all(|p| p.outcome == Outcome::Kept));
}

#[test]
fn sparse_deltas_read_as_their_dense_equivalent() {
    let dense = build(&[mouth(10)]);
    let sparse = build(&[MeshSpec {
        deltas: Deltas::Sparse,
        ..mouth(10)
    }]);
    let from_dense = optimize_morph_triangles(&dense.bytes)
        .unwrap()
        .bytes
        .unwrap();
    let from_sparse = optimize_morph_triangles(&sparse.bytes)
        .unwrap()
        .bytes
        .unwrap();
    assert_eq!(
        dense.triangles(&from_dense, 0),
        sparse.triangles(&from_sparse, 0)
    );
}

#[test]
fn every_index_width_is_rewritten() {
    for index_size in [1, 2, 4] {
        let spec = MeshSpec {
            index_size,
            ..pupil(16)
        };
        let built = build(std::slice::from_ref(&spec));
        let rewritten = optimize_morph_triangles(&built.bytes)
            .unwrap()
            .bytes
            .unwrap();
        assert_unfolded(&spec, &built.triangles(&rewritten, 0), 1e-9);
    }
}

#[test]
fn authored_interior_vertices_are_left_alone() {
    let built = build(&[hub(8)]);
    let optimized = optimize_morph_triangles(&built.bytes).unwrap();
    assert!(optimized.bytes.is_none());
    assert_eq!(
        optimized.report.parts[0].outcome,
        Outcome::Skipped(Skip::NotAPolygon)
    );
}

#[test]
fn meshes_without_targets_are_not_considered() {
    let spec = MeshSpec {
        targets: Vec::new(),
        ..mouth(10)
    };
    let optimized = optimize_morph_triangles(&build(&[spec]).bytes).unwrap();
    assert!(optimized.bytes.is_none());
    assert!(optimized.report.parts.is_empty());
}

#[test]
fn a_shape_no_triangulation_can_follow_is_reported() {
    // One lower-lip vertex (the first half of the outline) pushed up
    // through the upper lip: the outline crosses itself, so some area folds
    // whatever the triangles.
    let mut spec = mouth(10);
    let n = spec.positions.len();
    let target = &mut spec.targets[0].1;
    target[n / 4][1] += 1.0;
    let optimized = optimize_morph_triangles(&build(&[spec]).bytes).unwrap();
    let smile = &optimized.report.parts[0].folds[1];
    assert!(smile.after.unwrap() > 0.01, "{smile:?}");
}
