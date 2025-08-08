use bevy::prelude::*;
use ecs_animation_player::{
    animation::{AnimationData, AnimationMetadata, BakedAnimationData},
    ecs::{
        components::{AnimationPlayer, AnimationInstance, AnimatedColor},
        plugin::AnimationPlayerPlugin,
        resources::{AnimationOutput, IdMapping},
    },
    value::{Color, Value},
    AnimationTime, PlaybackMode,
};

#[test]
fn baked_animation_applies_and_outputs() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), AnimationPlayerPlugin));
    app.init_resource::<Assets<AnimationData>>();
    app.init_resource::<Assets<BakedAnimationData>>();

    let target = app
        .world_mut()
        .spawn((
            Name::new("Cube"),
            AnimatedColor(Color::rgba(0.0, 0.0, 0.0, 1.0)),
        ))
        .id();

    let duration = AnimationTime::from_seconds(1.0).unwrap();
    let mut anim = AnimationData::new("test_anim", "Test");
    anim.metadata.duration = duration;
    let handle = {
        let mut assets = app.world_mut().resource_mut::<Assets<AnimationData>>();
        assets.add(anim)
    };

    let mut meta = AnimationMetadata::new();
    meta.duration = duration;
    let mut baked = BakedAnimationData::new("test_anim", 1.0, duration, meta).unwrap();
    baked.add_track_data(
        "Cube/AnimatedColor",
        vec![
            (
                AnimationTime::from_seconds(0.0).unwrap(),
                Value::Color(Color::rgba(0.0, 0.0, 0.0, 1.0)),
            ),
            (
                AnimationTime::from_seconds(1.0).unwrap(),
                Value::Color(Color::rgba(0.0, 1.0, 0.0, 1.0)),
            ),
        ],
    );
    {
        let mut assets = app.world_mut().resource_mut::<Assets<BakedAnimationData>>();
        assets.add(baked);
    }

    let player_entity = app
        .world_mut()
        .spawn(AnimationPlayer {
            target_root: Some(target),
            playback_state: ecs_animation_player::PlaybackState::Playing,
            mode: PlaybackMode::Once,
            duration,
            ..default()
        })
        .id();
    let instance = AnimationInstance { animation: handle, weight: 1.0, ..default() };
    let instance_entity = app.world_mut().spawn(instance).id();
    app.world_mut()
        .entity_mut(player_entity)
        .add_child(instance_entity);

    let player_id = "player".to_string();
    app.world_mut()
        .resource_mut::<IdMapping>()
        .players
        .insert(player_id.clone(), player_entity);

    app.update();
    {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(std::time::Duration::from_secs_f64(1.0));
    }
    app.update();

    let color = app.world().get::<AnimatedColor>(target).unwrap();
    assert_eq!(color.0, Color::rgba(0.0, 1.0, 0.0, 1.0));

    let output = app.world().resource::<AnimationOutput>();
    let player_output = output.values.get(&player_id).unwrap();
    assert_eq!(
        player_output.get("Cube/AnimatedColor"),
        Some(&Value::Color(Color::rgba(0.0, 1.0, 0.0, 1.0)))
    );
}
