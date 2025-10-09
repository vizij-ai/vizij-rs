use vizij_animation_core::{
    data::{AnimationData, Track},
    parse_stored_animation_json,
};
use vizij_api_core::Value;

fn approx(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() <= eps, "left={a} right={b} eps={eps}");
}

#[test]
fn parses_new_format_fixture_and_preserves_points_and_transitions() {
    // Load the shared StoredAnimation-format fixture (duration in ms, stamps in 0..1)
    let json = vizij_test_fixtures::animations::json("vector-pose-combo")
        .expect("load vector-pose-combo fixture");
    let anim: AnimationData =
        parse_stored_animation_json(&json).expect("parse stored animation from shared fixture");

    // Duration is 5000 ms
    assert_eq!(anim.duration_ms, 5000);
    assert!(!anim.tracks.is_empty());

    // Track: cube-position-x is a scalar track with explicit in/out on some points
    let tx: &Track = anim
        .tracks
        .iter()
        .find(|t| t.animatable_id == "cube-position-x")
        .expect("cube-position-x track");

    assert!(tx.points.len() >= 2);
    // Stamps are normalized (0.25 stays 0.25)
    approx(tx.points[1].stamp, 0.25, 1e-6);

    // First segment [P0->P1] should use P0.out={0.65,0} and P1.in={0.35,1}
    let p0 = &tx.points[0];
    let p1 = &tx.points[1];
    let out0 = p0
        .transitions
        .as_ref()
        .and_then(|t| t.r#out.as_ref())
        .expect("p0.out");
    let in1 = p1
        .transitions
        .as_ref()
        .and_then(|t| t.r#in.as_ref())
        .expect("p1.in");
    approx(out0.x, 0.65, 1e-6);
    approx(out0.y, 0.00, 1e-6);
    approx(in1.x, 0.35, 1e-6);
    approx(in1.y, 1.00, 1e-6);

    // Track: object-position is a Vec3-like track (value stored as Vec3)
    let tv3: &Track = anim
        .tracks
        .iter()
        .find(|t| t.animatable_id == "object-position")
        .expect("object-position track");
    assert!(tv3.points.len() >= 2);

    // Track: material-color has no transitions on points; transitions Option should be None for those points
    let tcol: &Track = anim
        .tracks
        .iter()
        .find(|t| t.animatable_id == "material-color")
        .expect("material-color track");
    assert!(tcol.points.len() >= 2);
    let any_transitions = tcol.points.iter().any(|p| p.transitions.is_some());
    // In the fixture, color points do not specify transitions, so all should be None
    assert!(
        !any_transitions,
        "expected no transitions on material-color points"
    );
}

#[test]
fn parses_pose_quat_transform_fixture_with_extended_values() {
    let json = vizij_test_fixtures::animations::json("pose-quat-transform")
        .expect("load pose-quat-transform fixture");
    let anim: AnimationData =
        parse_stored_animation_json(&json).expect("parse pose-quat-transform animation");

    assert_eq!(anim.duration_ms, 3000);

    let rot_track = anim
        .tracks
        .iter()
        .find(|t| t.animatable_id == "rig/root.rotation")
        .expect("rotation track");
    assert!(matches!(rot_track.points[0].value, Value::Quat(_)));

    let transform_track = anim
        .tracks
        .iter()
        .find(|t| t.animatable_id == "rig/root.transform")
        .expect("transform track");
    match &transform_track.points[1].value {
        Value::Transform {
            translation,
            rotation,
            scale,
        } => {
            approx(translation[0], 0.2, 1e-6);
            approx(rotation[3], 0.991445, 1e-6);
            approx(scale[2], 1.1, 1e-6);
        }
        other => panic!("expected transform value, got {other:?}"),
    }
}
