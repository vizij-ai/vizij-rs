use vizij_animation_core::{
    data::{AnimationData, AuthoredTransition},
    parse_stored_animation_json, sample_track,
    value::Value,
};

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/animations/studio-v2")
        .join(format!("{name}.json"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read Studio v2 fixture {name}: {error}"))
}

fn parse_fixture(name: &str) -> AnimationData {
    parse_stored_animation_json(&fixture(name)).expect("parse Studio animation fixture")
}

fn approx(left: f32, right: f32, epsilon: f32) {
    assert!(
        (left - right).abs() <= epsilon,
        "left={left} right={right} epsilon={epsilon}"
    );
}

#[test]
fn parses_studio_v2_millisecond_stamps_and_authored_transitions() {
    let anim = parse_fixture("studio-v2-canonical");
    assert_eq!(anim.duration_ms, 3000);

    let smile = anim
        .tracks
        .iter()
        .find(|track| track.id == "face.smile.amount")
        .expect("smile track");
    assert_eq!(
        smile
            .points
            .iter()
            .map(|point| point.stamp as u32)
            .collect::<Vec<_>>(),
        vec![0, 1000, 2000, 3000]
    );

    assert!(matches!(
        smile.points[0].transitions.as_ref().and_then(|t| t.r#out.as_ref()),
        Some(AuthoredTransition::Name(name)) if name == "cubic"
    ));

    match smile.points[1]
        .transitions
        .as_ref()
        .and_then(|t| t.r#in.as_ref())
        .expect("explicit incoming handle")
    {
        AuthoredTransition::Explicit(delta) => {
            approx(delta.x, -240.0, 1e-6);
            approx(delta.y, 0.12, 1e-6);
        }
        other => panic!("expected explicit handle, got {other:?}"),
    }

    assert!(matches!(
        smile.points[3].transitions.as_ref().and_then(|t| t.r#in.as_ref()),
        Some(AuthoredTransition::Name(name)) if name == "inferred-auto-clamped"
    ));
}

#[test]
fn samples_studio_v2_bool_and_string_tracks_as_step_values() {
    let anim = parse_fixture("studio-v2-canonical");

    let eyes = anim
        .tracks
        .iter()
        .find(|track| track.id == "face.eyes.visible")
        .expect("eyes-visible track");
    assert_eq!(sample_track(eyes, 1000.0), Value::Bool(true));
    assert_eq!(sample_track(eyes, 1300.0), Value::Bool(false));

    let expression = anim
        .tracks
        .iter()
        .find(|track| track.id == "face.expression.label")
        .expect("expression-label track");
    assert_eq!(
        sample_track(expression, 1499.0),
        Value::Text("neutral".into())
    );
    assert_eq!(
        sample_track(expression, 2000.0),
        Value::Text("smile".into())
    );
}

#[test]
fn studio_v1_migration_matches_studio_golden_output() {
    let migrated_from_v1 = parse_fixture("legacy-v1-normalized-input");
    let studio_golden = parse_fixture("legacy-v1-migrated-v2");

    assert_eq!(migrated_from_v1.duration_ms, studio_golden.duration_ms);
    assert_eq!(
        migrated_from_v1.tracks[0]
            .points
            .iter()
            .map(|point| point.stamp as u32)
            .collect::<Vec<_>>(),
        vec![0, 1200, 2400]
    );
    assert_eq!(migrated_from_v1.tracks, studio_golden.tracks);
}
