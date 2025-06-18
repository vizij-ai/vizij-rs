//! Tests for ensuring compatibility with the animation specification.

use animation_player::animation::{
    data::AnimationData,
    group::TrackGroup,
    keypoint::AnimationKeypoint,
    track::AnimationTrack,
    transition::{AnimationTransition, TransitionVariant},
};
use animation_player::value::{euler::Euler, Value};
use animation_player::AnimationTime;
use std::collections::HashMap;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_serialization_for_spec_compat() {
    // 1. Construct a canonical AnimationData object that uses all spec features.
    let mut animation = AnimationData::new("spec-compat-anim", "Spec Compatibility Test");

    // -- Add a TrackGroup --
    let mut group = TrackGroup::new("group-1", "Test Group");
    let track_id_for_group = animation_player::animation::ids::TrackId::new();
    group.tracks.push(track_id_for_group); // Add a dummy track ID
    animation.add_group(group);

    // -- Add a Track with Euler values and settings --
    let mut track = AnimationTrack::new("euler-track", "transform.rotation");
    track.id = track_id_for_group; // Use the same ID for consistency
    track.settings = Some({
        let mut settings = HashMap::new();
        settings.insert("color".to_string(), "#FF0000".to_string());
        settings
    });

    let keypoint1 = AnimationKeypoint::new(
        AnimationTime::from_seconds(0.0).unwrap(),
        Value::Euler(Euler::new(0.0, 0.0, 0.0)),
    );
    let keypoint2 = AnimationKeypoint::new(
        AnimationTime::from_seconds(1.0).unwrap(),
        Value::Euler(Euler::new(1.57, 0.0, 0.0)), // 90 degrees roll
    );
    track.add_keypoint(keypoint1.clone()).unwrap();
    track.add_keypoint(keypoint2.clone()).unwrap();
    animation.add_track(track);

    // -- Add a Transition --
    let transition =
        AnimationTransition::new(keypoint1.id, keypoint2.id, TransitionVariant::Linear);
    animation.add_transition(transition);

    // 2. Serialize the AnimationData to a JSON string.
    let json_output =
        serde_json::to_string_pretty(&animation).expect("Failed to serialize animation");

    // 3. Write the JSON to a file.
    let mut file = NamedTempFile::new().expect("Failed to create temporary file");
    let file_path = file.path().to_owned();
    file.write_all(json_output.as_bytes())
        .expect("Failed to write test animation file");

    // 4. Read the file back and deserialize it.
    let file_content =
        std::fs::read_to_string(file_path).expect("Failed to read test animation file");
    let deserialized_animation: AnimationData =
        serde_json::from_str(&file_content).expect("Failed to deserialize animation");

    // 5. Assert that the deserialized object is equal to the original.
    assert_eq!(
        animation, deserialized_animation,
        "The serialized and deserialized animation should be identical"
    );
}
