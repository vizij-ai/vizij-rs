//! A player's time stays on the timeline its updates add up to, a step
//! samples what a bake takes at the same clip time, and a bake takes its
//! frames up to the first at or past its end.

use vizij_animation_core::{
    AnimId, AnimationData, BakingConfig, Config, Engine, Inputs, InstanceCfg, Keypoint, LoopMode,
    PlayerCommand, PlayerId, Track, TrackValue, Value,
};

/// A clip `duration_ms` long ramping one track from -100 to 100.
fn ramp(duration_ms: u32) -> AnimationData {
    let point = |id: &str, stamp: f32, value: f32| Keypoint {
        id: id.into(),
        stamp,
        value: TrackValue::Float(value),
        transitions: None,
    };
    AnimationData {
        id: None,
        name: "ramp".into(),
        tracks: vec![Track {
            id: "t".into(),
            name: "x".into(),
            animatable_id: "x".into(),
            points: vec![point("a", 0.0, -100.0), point("b", 1.0, 100.0)],
            settings: None,
        }],
        groups: Default::default(),
        duration_ms,
    }
}

/// An engine playing `anim` once on one player, and the player.
fn playing_once(anim: AnimationData) -> (Engine, AnimId, PlayerId) {
    let mut eng = Engine::new(Config::default());
    let anim = eng.load_animation(anim);
    let player = eng.create_player("p");
    eng.add_instance(player, anim, InstanceCfg::default());
    eng.update(
        0.0,
        Inputs {
            player_cmds: vec![PlayerCommand::SetLoopMode {
                player,
                mode: LoopMode::Once,
            }],
            instance_updates: Vec::new(),
        },
    );
    (eng, anim, player)
}

/// 10 ms, as a device stepping at 100 Hz gives it in nanoseconds.
const DT_NS: u64 = 10_000_000;

#[test]
fn a_player_stepped_at_100_hz_for_an_hour_stays_on_its_timeline() {
    let (mut eng, _, _) = playing_once(ramp(7_200_000));
    let dt = DT_NS as f64 / 1e9;
    for n in 1..=360_000u64 {
        eng.update(dt, Inputs::default());
        let time = eng.list_players()[0].time;
        let nominal = (n * DT_NS) as f64 / 1e9;
        assert!(
            (time - nominal).abs() <= 1e-12,
            "after {n} steps the playhead is {time} s, {} s off {nominal} s",
            time - nominal
        );
    }
}

#[test]
fn a_step_samples_the_frame_a_bake_takes_at_its_time() {
    let (mut eng, anim, _) = playing_once(ramp(60_000));
    let baked = eng
        .bake_animation(
            anim,
            &BakingConfig {
                frame_rate: 100.0,
                ..Default::default()
            },
        )
        .expect("a bake");
    let frames = &baked.tracks[0].values;
    assert_eq!(frames.len(), 6_001);
    let dt = DT_NS as f64 / 1e9;
    for (n, frame) in frames.iter().enumerate() {
        let out = eng.update(if n == 0 { 0.0 } else { dt }, Inputs::default());
        let stepped: &Value = &out.changes[0].value;
        assert_eq!(stepped, frame, "frame {n}");
    }
}

#[test]
fn a_once_player_ends_on_the_step_that_reaches_its_end() {
    for (duration_ms, dt_ns) in [(5_000, DT_NS), (1_000, 100_000_000), (10_000, 20_000_000)] {
        let (mut eng, _, _) = playing_once(ramp(duration_ms));
        let steps = u64::from(duration_ms) * 1_000_000 / dt_ns;
        for _ in 0..steps - 1 {
            eng.update(dt_ns as f64 / 1e9, Inputs::default());
        }
        assert!(
            !eng.list_players()[0].ended,
            "{duration_ms} ms: one step short"
        );
        eng.update(dt_ns as f64 / 1e9, Inputs::default());
        let player = &eng.list_players()[0];
        assert!(player.ended, "{duration_ms} ms at {dt_ns} ns: {player:?}");
        assert_eq!(player.time, f64::from(duration_ms) / 1000.0);
    }
}

/// How many frames a bake of `ramp(duration_ms)` takes at `frame_rate`
/// between `start_time` and `end_time`.
fn frames(duration_ms: u32, frame_rate: f64, start_time: f64, end_time: Option<f64>) -> usize {
    let mut eng = Engine::new(Config::default());
    let anim = eng.load_animation(ramp(duration_ms));
    let cfg = BakingConfig {
        frame_rate,
        start_time,
        end_time,
        ..Default::default()
    };
    eng.bake_animation(anim, &cfg).expect("a bake").tracks[0]
        .values
        .len()
}

#[test]
fn a_bake_takes_its_frames_up_to_the_first_at_or_past_its_end() {
    // A span that is an inexact decimal takes the frames it means.
    assert_eq!(frames(70, 100.0, 0.0, None), 8);
    assert_eq!(frames(550, 100.0, 0.0, None), 56);
    assert_eq!(frames(280, 25.0, 0.0, None), 8);
    assert_eq!(frames(150, 100.0, 0.0, None), 16);
    assert_eq!(frames(8_100, 30.0, 0.0, None), 244);
    // An end just past a frame takes the frame after it.
    assert_eq!(frames(10_000, 1.0, 0.0, Some(3.000_000_5)), 5);
    assert_eq!(frames(10_000, 1.0, 0.0, Some(3.0)), 4);
    // An empty window takes its one frame.
    assert_eq!(frames(10_000, 60.0, 2.0, Some(2.0)), 1);
}
