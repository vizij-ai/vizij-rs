use animation_player::animation::{
    data::AnimationData,
    ids::KeypointId,
    track::AnimationTrack,
    transition::{AnimationTransition, TransitionVariant},
};

#[test]
fn test_track_creation() {
    let track = AnimationTrack::new("position", "transform.position");
    assert_eq!(track.name, "position");
    assert_eq!(track.target, "transform.position");
    assert!(track.enabled);
    assert_eq!(track.weight, 1.0);
    assert!(track.settings.is_none());
}

#[test]
fn test_transition_creation() {
    let kp1_id = KeypointId::new();
    let kp2_id = KeypointId::new();
    let transition = AnimationTransition::new(kp1_id, kp2_id, TransitionVariant::Bezier);

    assert_eq!(transition.from_keypoint(), kp1_id);
    assert_eq!(transition.to_keypoint(), kp2_id);
    assert_eq!(transition.variant, TransitionVariant::Bezier);
}

#[test]
fn test_animation_data_transitions() {
    let mut animation = AnimationData::new("test", "Test Animation");
    let kp1_id = KeypointId::new();
    let kp2_id = KeypointId::new();
    let transition = AnimationTransition::new(kp1_id, kp2_id, TransitionVariant::Spring);

    animation.add_transition(transition.clone());

    let found_transition = animation.get_transition_for_keypoints(kp1_id, kp2_id);
    assert!(found_transition.is_some());
    assert_eq!(found_transition.unwrap().variant, TransitionVariant::Spring);
}
