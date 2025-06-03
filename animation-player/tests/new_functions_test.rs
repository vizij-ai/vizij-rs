use animation_player::{
    AnimationEngine, AnimationConfig, AnimationData, AnimationTime, 
    animation::{InstanceSettings, track::AnimationTrack, keypoint::AnimationKeypoint},
    value::Value,
};

#[test]
fn test_animation_ids_and_add_animation_to_player() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    
    // Initially no animations loaded
    assert_eq!(engine.animation_ids().len(), 0);
    
    // Create test animation data
    let mut track = AnimationTrack::new(
        "test_track".to_string(),
        "test_target".to_string(),
    );
    
    track.add_keypoint(AnimationKeypoint::new(
        AnimationTime::zero(),
        Value::Float(0.0),
    )).unwrap();
    
    track.add_keypoint(AnimationKeypoint::new(
        AnimationTime::new(1.0).unwrap(),
        Value::Float(1.0),
    )).unwrap();
    
    let mut animation_data = AnimationData::new(
        "test_animation".to_string(),
        "Test Animation".to_string(),
    );
    
    animation_data.add_track(track);
    
    // Load animation data
    engine.load_animation_data(animation_data).unwrap();
    
    // Check that animation ID is now available
    let animation_ids = engine.animation_ids();
    assert_eq!(animation_ids.len(), 1);
    assert_eq!(animation_ids[0], "test_animation");
    
    // Create a player
    engine.create_player("test_player").unwrap();
    
    // Initially player has no instances
    let player = engine.get_player("test_player").unwrap();
    assert_eq!(player.instance_ids().len(), 0);
    
    // Add animation to player with default settings
    let instance_id = engine.add_animation_to_player(
        "test_player",
        "test_animation", 
        None
    ).unwrap();
    
    // Check that instance was added
    let player = engine.get_player("test_player").unwrap();
    let instance_ids = player.instance_ids();
    assert_eq!(instance_ids.len(), 1);
    assert_eq!(instance_ids[0], instance_id);
    
    // Add animation to player with custom settings
    let custom_settings = InstanceSettings::new("test_animation")
        .with_timescale(2.0)
        .with_enabled(true);
        
    let instance_id2 = engine.add_animation_to_player(
        "test_player",
        "test_animation",
        Some(custom_settings)
    ).unwrap();
    
    // Check that second instance was added
    let player = engine.get_player("test_player").unwrap();
    let instance_ids = player.instance_ids();
    assert_eq!(instance_ids.len(), 2);
    assert!(instance_ids.contains(&instance_id.as_str()));
    assert!(instance_ids.contains(&instance_id2.as_str()));
}

#[test]
fn test_add_animation_to_nonexistent_player() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    
    // Create test animation data
    let animation_data = AnimationData::new(
        "test_animation".to_string(),
        "Test Animation".to_string(),
    );
    
    engine.load_animation_data(animation_data).unwrap();
    
    // Try to add animation to non-existent player
    let result = engine.add_animation_to_player(
        "nonexistent_player",
        "test_animation",
        None
    );
    
    assert!(result.is_err());
}

#[test]
fn test_add_nonexistent_animation_to_player() {
    let mut engine = AnimationEngine::new(AnimationConfig::default());
    
    // Create a player
    engine.create_player("test_player").unwrap();
    
    // Try to add non-existent animation to player
    let result = engine.add_animation_to_player(
        "test_player",
        "nonexistent_animation",
        None
    );
    
    assert!(result.is_err());
}
