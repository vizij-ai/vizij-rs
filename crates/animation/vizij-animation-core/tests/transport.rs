//! Transport semantics: the play window in every loop mode, reversed playback,
//! per-instance offset and time scale, the anchored start, and the ended flag.

use serde_json::json;
use vizij_animation_core::engine::{PlaybackState, PlayerInfo};
use vizij_animation_core::{
    AnimId, AnimationData, Config, Engine, Inputs, InstId, InstanceCfg, InstanceUpdate, Keypoint,
    LoopMode, PlayerCommand, PlayerId, Track, TrackValue, Transitions, Value, Vec2,
};

/// A linear ramp from 0 to 1 over `duration_s` on `key`: its value is the
/// instance's local time over the duration.
fn ramp(key: &str, duration_s: f32) -> AnimationData {
    let point = |stamp: f32, value: f32, transitions| Keypoint {
        id: format!("{key}-{stamp}"),
        stamp,
        value: TrackValue::Float(value),
        transitions: Some(transitions),
    };
    AnimationData {
        id: None,
        name: key.into(),
        tracks: vec![Track {
            id: key.into(),
            name: key.into(),
            animatable_id: key.into(),
            points: vec![
                point(
                    0.0,
                    0.0,
                    Transitions {
                        r#in: None,
                        out: Some(Vec2 { x: 0.0, y: 0.0 }),
                    },
                ),
                point(
                    1.0,
                    1.0,
                    Transitions {
                        r#in: Some(Vec2 { x: 1.0, y: 1.0 }),
                        out: None,
                    },
                ),
            ],
            settings: None,
        }],
        groups: json!({}),
        duration_ms: (duration_s * 1000.0) as u32,
    }
}

/// An engine with one player playing a `duration_s` ramp on key `x`.
fn ramp_player(duration_s: f32) -> (Engine, PlayerId) {
    let mut engine = Engine::new(Config::default());
    let anim = engine.load_animation(ramp("x", duration_s));
    let player = engine.create_player("p");
    engine.add_instance(player, anim, InstanceCfg::default());
    (engine, player)
}

fn commands(commands: Vec<PlayerCommand>) -> Inputs {
    Inputs {
        player_cmds: commands,
        instance_updates: Vec::new(),
    }
}

/// Step by `dt` and return the sampled value of `key`.
fn sample(engine: &mut Engine, dt: f64, inputs: Inputs, key: &str) -> f32 {
    let out = engine.update(dt, inputs);
    let change = out
        .changes
        .iter()
        .find(|c| c.key == key)
        .unwrap_or_else(|| panic!("no output for {key}"));
    match change.value {
        Value::F32(v) => v,
        ref other => panic!("expected F32, got {other:?}"),
    }
}

fn info(engine: &Engine, player: PlayerId) -> PlayerInfo {
    engine
        .list_players()
        .into_iter()
        .find(|p| p.id == player.0)
        .expect("player info")
}

fn approx(actual: impl Into<f64>, expected: f64) {
    let actual = actual.into();
    assert!(
        (actual - expected).abs() < 1e-4,
        "expected {expected}, got {actual}"
    );
}

fn windowed(player: PlayerId, mode: LoopMode, start: f64, end: f64) -> Inputs {
    commands(vec![
        PlayerCommand::SetLoopMode { player, mode },
        PlayerCommand::SetWindow {
            player,
            start_time: start,
            end_time: Some(end),
        },
    ])
}

#[test]
fn once_clamps_the_playhead_into_the_window() {
    let (mut engine, p) = ramp_player(4.0);
    // Setting the window moves the playhead (0) to its start.
    approx(
        sample(&mut engine, 0.0, windowed(p, LoopMode::Once, 1.0, 3.0), "x"),
        0.25,
    );
    approx(info(&engine, p).time, 1.0);
    approx(sample(&mut engine, 1.5, Inputs::default(), "x"), 0.625);
    assert!(!info(&engine, p).ended);

    approx(sample(&mut engine, 5.0, Inputs::default(), "x"), 0.75);
    let state = info(&engine, p);
    approx(state.time, 3.0);
    assert!(state.ended, "held at the window end");
    assert_eq!(state.state, PlaybackState::Playing);
    approx(state.length, 4.0);
}

#[test]
fn loop_wraps_within_the_window() {
    let (mut engine, p) = ramp_player(4.0);
    sample(&mut engine, 0.0, windowed(p, LoopMode::Loop, 1.0, 3.0), "x");
    approx(sample(&mut engine, 1.5, Inputs::default(), "x"), 0.625); // 2.5 s
    approx(sample(&mut engine, 1.0, Inputs::default(), "x"), 0.375); // 3.5 s wraps to 1.5 s
    assert!(!info(&engine, p).ended, "a loop never ends");
}

#[test]
fn ping_pong_reflects_within_the_window() {
    let (mut engine, p) = ramp_player(4.0);
    sample(
        &mut engine,
        0.0,
        windowed(p, LoopMode::PingPong, 1.0, 3.0),
        "x",
    );
    approx(sample(&mut engine, 1.5, Inputs::default(), "x"), 0.625); // 2.5 s
    approx(sample(&mut engine, 1.0, Inputs::default(), "x"), 0.625); // 3.5 s reflects to 2.5 s
    approx(sample(&mut engine, 1.0, Inputs::default(), "x"), 0.375); // 4.5 s reflects to 1.5 s
}

#[test]
fn a_window_end_defaults_to_the_player_length() {
    let (mut engine, p) = ramp_player(4.0);
    let inputs = commands(vec![
        PlayerCommand::SetLoopMode {
            player: p,
            mode: LoopMode::Loop,
        },
        PlayerCommand::SetWindow {
            player: p,
            start_time: 2.0,
            end_time: None,
        },
    ]);
    approx(sample(&mut engine, 0.0, inputs, "x"), 0.5);
    approx(sample(&mut engine, 2.5, Inputs::default(), "x"), 0.625); // 4.5 s wraps to 2.5 s
}

#[test]
fn a_negative_speed_plays_backwards_through_the_window() {
    let (mut engine, p) = ramp_player(4.0);
    let mut inputs = windowed(p, LoopMode::Loop, 1.0, 3.0);
    inputs.player_cmds.extend([
        PlayerCommand::Seek {
            player: p,
            time: 2.0,
        },
        PlayerCommand::SetSpeed {
            player: p,
            speed: -1.0,
        },
    ]);
    approx(sample(&mut engine, 0.5, inputs, "x"), 0.375); // 1.5 s
    approx(sample(&mut engine, 1.0, Inputs::default(), "x"), 0.625); // 0.5 s wraps to 2.5 s
    let state = info(&engine, p);
    assert_eq!(state.state, PlaybackState::Playing);
    approx(state.speed, -1.0);

    // Once, backwards: held at the window start, ended; forwards again at once.
    sample(&mut engine, 0.0, windowed(p, LoopMode::Once, 1.0, 3.0), "x");
    approx(sample(&mut engine, 5.0, Inputs::default(), "x"), 0.25);
    assert!(info(&engine, p).ended, "held at the window start");
    let forward = commands(vec![PlayerCommand::SetSpeed {
        player: p,
        speed: 1.0,
    }]);
    approx(sample(&mut engine, 0.5, forward, "x"), 0.375);
    assert!(!info(&engine, p).ended);
}

/// Without a window end, `Once` holds the time at the player's length, so a
/// reversal moves the playhead in the very next step.
#[test]
fn once_reverses_from_the_end_it_reached() {
    let (mut engine, p) = ramp_player(2.0);
    let once = commands(vec![PlayerCommand::SetLoopMode {
        player: p,
        mode: LoopMode::Once,
    }]);
    approx(sample(&mut engine, 5.0, once, "x"), 1.0);
    assert!(info(&engine, p).ended);
    let back = commands(vec![PlayerCommand::SetSpeed {
        player: p,
        speed: -1.0,
    }]);
    approx(sample(&mut engine, 0.5, back, "x"), 0.75);
}

/// `start_offset` delays an instance on the player timeline and `time_scale`
/// stretches it, per instance.
#[test]
fn offset_and_time_scale_apply_per_instance() {
    let mut engine = Engine::new(Config::default());
    let a: AnimId = engine.load_animation(ramp("a", 2.0));
    let b = engine.load_animation(ramp("b", 2.0));
    let p = engine.create_player("p");
    let shifted: InstId = engine.add_instance(p, a, InstanceCfg::default());
    engine.add_instance(p, b, InstanceCfg::default());
    let update = Inputs {
        player_cmds: vec![PlayerCommand::SetLoopMode {
            player: p,
            mode: LoopMode::Once,
        }],
        instance_updates: vec![InstanceUpdate {
            player: p,
            inst: shifted,
            weight: None,
            time_scale: Some(2.0),
            start_offset: Some(1.0),
            enabled: None,
        }],
    };
    engine.update(0.0, update);
    approx(info(&engine, p).length, 5.0); // 1 + 2 * 2

    approx(sample(&mut engine, 0.5, Inputs::default(), "a"), 0.0); // before its start
    approx(sample(&mut engine, 0.0, Inputs::default(), "b"), 0.25);
    approx(sample(&mut engine, 2.5, Inputs::default(), "a"), 0.5); // (3 - 1) / 2 of 2 s
    approx(sample(&mut engine, 0.0, Inputs::default(), "b"), 1.0);

    // A negative offset starts the instance partway into its clip.
    let earlier = Inputs {
        player_cmds: vec![PlayerCommand::Seek {
            player: p,
            time: 0.0,
        }],
        instance_updates: vec![InstanceUpdate {
            player: p,
            inst: shifted,
            weight: None,
            time_scale: Some(1.0),
            start_offset: Some(-1.0),
            enabled: None,
        }],
    };
    approx(sample(&mut engine, 0.0, earlier, "a"), 0.5);
}

/// The playhead of a player started by `PlayAfter` depends on the time since
/// the anchor only, wherever the update boundaries fall.
#[test]
fn play_after_starts_at_the_anchor_whatever_the_step() {
    // The anchor is 0.3 s after the first update starts; two engines step by
    // 0.125 s and 0.2 s.
    let playhead_at = |step: f64, until: f64| {
        let (mut engine, p) = ramp_player(10.0);
        engine.update(0.0, commands(vec![PlayerCommand::Stop { player: p }]));
        engine.update(
            step,
            commands(vec![PlayerCommand::PlayAfter {
                player: p,
                delay: 0.3,
            }]),
        );
        let mut now = step;
        while now < until - 1e-6 {
            engine.update(step, Inputs::default());
            now += step;
        }
        info(&engine, p).time
    };
    approx(playhead_at(0.125, 1.0), 0.7);
    approx(playhead_at(0.2, 1.0), 0.7);
}

#[test]
fn play_after_on_a_step_boundary_starts_at_that_boundary() {
    let (mut engine, p) = ramp_player(10.0);
    engine.update(0.0, commands(vec![PlayerCommand::Stop { player: p }]));
    let anchored = commands(vec![PlayerCommand::PlayAfter {
        player: p,
        delay: 0.5,
    }]);
    engine.update(0.25, anchored);
    let waiting = info(&engine, p);
    assert_eq!(
        waiting.state,
        PlaybackState::Stopped,
        "holds until the anchor"
    );
    approx(waiting.time, 0.0);

    engine.update(0.25, Inputs::default());
    let started = info(&engine, p);
    assert_eq!(started.state, PlaybackState::Playing);
    approx(started.time, 0.0);

    engine.update(0.25, Inputs::default());
    approx(info(&engine, p).time, 0.25);
}

#[test]
fn play_after_in_the_past_catches_up() {
    let (mut engine, p) = ramp_player(10.0);
    engine.update(0.0, commands(vec![PlayerCommand::Stop { player: p }]));
    let late = commands(vec![PlayerCommand::PlayAfter {
        player: p,
        delay: -0.5,
    }]);
    engine.update(0.25, late);
    approx(info(&engine, p).time, 0.75);
}

/// A waiting player keeps the speed it is given for its start; a pause
/// cancels the wait.
#[test]
fn a_waiting_player_holds_until_its_start_or_a_pause() {
    let (mut engine, p) = ramp_player(10.0);
    engine.update(0.0, commands(vec![PlayerCommand::Stop { player: p }]));
    let reversed = commands(vec![
        PlayerCommand::Seek {
            player: p,
            time: 5.0,
        },
        PlayerCommand::PlayAfter {
            player: p,
            delay: 1.0,
        },
        PlayerCommand::SetSpeed {
            player: p,
            speed: -2.0,
        },
    ]);
    engine.update(0.5, reversed);
    approx(info(&engine, p).time, 5.0);
    engine.update(1.0, Inputs::default());
    approx(info(&engine, p).time, 4.0); // 0.5 s after the start, at -2

    engine.update(
        0.0,
        commands(vec![PlayerCommand::PlayAfter {
            player: p,
            delay: 1.0,
        }]),
    );
    engine.update(0.5, commands(vec![PlayerCommand::Pause { player: p }]));
    engine.update(2.0, Inputs::default());
    let paused = info(&engine, p);
    assert_eq!(paused.state, PlaybackState::Paused);
    approx(paused.time, 4.0);
}
