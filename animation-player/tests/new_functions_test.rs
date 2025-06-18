use animation_player::{
    animation::{keypoint::AnimationKeypoint, track::AnimationTrack, AnimationSettings},
    value::Value,
    AnimationData, AnimationEngine, AnimationEngineConfig, AnimationTime,
};

#[test]
fn test_animation_ids_and_add_animation_to_player() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Initially no animations loaded
    assert_eq!(engine.animation_ids().len(), 0);

    // Create test animation data
    let mut track = AnimationTrack::new("test_track".to_string(), "test_target".to_string());
    track.settings = None; // Explicitly set settings to None

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::zero(),
            Value::Float(0.0),
        ))
        .unwrap();

    track
        .add_keypoint(AnimationKeypoint::new(
            AnimationTime::from_seconds(1.0).unwrap(),
            Value::Float(1.0),
        ))
        .unwrap();

    let mut animation_data = AnimationData::new("test_animation", "Test Animation".to_string());

    animation_data.add_track(track);

    // Load animation data
    let animation_id = engine.load_animation_data(animation_data).unwrap();

    // Check that animation ID is now available
    let animation_ids = engine.animation_ids();
    assert_eq!(animation_ids.len(), 1);
    assert_eq!(animation_ids[0], animation_id);

    // Create a player
    let player_id = engine.create_player();

    // Initially player has no instances
    let player = engine.get_player(&player_id).unwrap();
    assert_eq!(player.instance_ids().len(), 0);

    // Add animation to player with default settings
    let instance_id = engine
        .add_animation_to_player(&player_id, &animation_id, None)
        .unwrap();

    // Check that instance was added
    let player = engine.get_player(&player_id).unwrap();
    let instance_ids = player.instance_ids();
    assert_eq!(instance_ids.len(), 1);
    assert_eq!(instance_ids[0], instance_id);

    // Add animation to player with custom settings
    let custom_settings = AnimationSettings {
        timescale: 2.0,
        enabled: true,
        ..Default::default()
    };

    let instance_id2 = engine
        .add_animation_to_player(&player_id, &animation_id, Some(custom_settings))
        .unwrap();

    // Check that second instance was added
    let player = engine.get_player(&player_id).unwrap();
    let instance_ids = player.instance_ids();
    assert_eq!(instance_ids.len(), 2);
    assert!(instance_ids.contains(&instance_id.as_str()));
    assert!(instance_ids.contains(&instance_id2.as_str()));
}

#[test]
fn test_add_animation_to_nonexistent_player() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create test animation data
    let animation_data = AnimationData::new("test_animation", "Test Animation".to_string());

    let animation_id = engine.load_animation_data(animation_data).unwrap();

    // Try to add animation to non-existent player
    let result = engine.add_animation_to_player("nonexistent_player", &animation_id, None);

    assert!(result.is_err());
}

#[test]
fn test_add_nonexistent_animation_to_player() {
    let mut engine = AnimationEngine::new(AnimationEngineConfig::default());

    // Create a player
    let player_id = engine.create_player();

    // Try to add non-existent animation to player
    let result = engine.add_animation_to_player(&player_id, "nonexistent_animation", None);

    assert!(result.is_err());
}
