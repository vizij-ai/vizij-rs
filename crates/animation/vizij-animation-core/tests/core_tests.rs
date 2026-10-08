#![allow(clippy::approx_constant)]
use vizij_animation_core::{
    accumulate::AccumulatorWithDerivatives,
    baking::{export_baked_json, BakingConfig},
    binding::TargetResolver,
    config::Config,
    data::{AnimationData, Keypoint, Track, Transitions, Vec2},
    engine::{Engine, InstanceCfg, PlaybackState},
    ids::{AnimId, IdAllocator, PlayerId},
    inputs::{Inputs, InstanceUpdate, LoopMode, PlayerCommand},
    outputs::{CoreEvent, Outputs},
    sampling::{sample_track, sample_track_with_derivative},
    value::TrackValue,
};
use vizij_api_core::value::{as_quat, Value};

fn approx(a: f32, b: f32, eps: f32) {
    assert!((a - b).abs() <= eps, "left={a} right={b} eps={eps}");
}

fn norm4(q: [f32; 4]) -> f32 {
    (q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3]).sqrt()
}

fn mk_scalar_track_linear(path: &str, keys: &[(f32, f32)]) -> Track {
    // Build normalized keypoints with per-segment linear timing:
    // For each segment, left.out=(0,0), right.in=(1,1)
    let mut points: Vec<Keypoint> = Vec::with_capacity(keys.len());
    for (i, (stamp, v)) in keys.iter().enumerate() {
        let mut transitions: Option<Transitions> = None;
        let is_first = i == 0;
        let is_last = i + 1 == keys.len();
        if !is_last || !is_first {
            let mut t = Transitions {
                r#in: None,
                r#out: None,
            };
            if !is_last {
                t.r#out = Some(Vec2 { x: 0.0, y: 0.0 });
            }
            if !is_first {
                t.r#in = Some(Vec2 { x: 1.0, y: 1.0 });
            }
            // Only assign if at least one is present
            if t.r#in.is_some() || t.r#out.is_some() {
                transitions = Some(t);
            }
        }
        points.push(Keypoint {
            id: format!("k{i}"),
            stamp: *stamp,
            value: TrackValue::Float(*v),
            transitions,
        });
    }
    Track {
        id: format!("t-{}", path),
        name: path.to_string(),
        animatable_id: path.to_string(),
        points,
        settings: None,
    }
}

fn mk_quat_track_linear(path: &str, keys: &[(f32, [f32; 4])]) -> Track {
    let mut points: Vec<Keypoint> = Vec::with_capacity(keys.len());
    for (i, (stamp, q)) in keys.iter().enumerate() {
        let mut transitions: Option<Transitions> = None;
        let is_first = i == 0;
        let is_last = i + 1 == keys.len();
        if !is_last || !is_first {
            let mut t = Transitions {
                r#in: None,
                r#out: None,
            };
            if !is_last {
                t.r#out = Some(Vec2 { x: 0.0, y: 0.0 });
            }
            if !is_first {
                t.r#in = Some(Vec2 { x: 1.0, y: 1.0 });
            }
            if t.r#in.is_some() || t.r#out.is_some() {
                transitions = Some(t);
            }
        }
        points.push(Keypoint {
            id: format!("k{i}"),
            stamp: *stamp,
            value: TrackValue::Quat(*q),
            transitions,
        });
    }
    Track {
        id: format!("t-{}", path),
        name: path.to_string(),
        animatable_id: path.to_string(),
        points,
        settings: None,
    }
}

fn mk_anim(name: &str, duration_s: f32, tracks: Vec<Track>) -> AnimationData {
    serde_json::json!({}); // keep serde_json in scope for macros even if unused
    AnimationData {
        id: None,
        name: name.to_string(),
        tracks,
        groups: serde_json::json!({}),
        duration_ms: (duration_s * 1000.0) as u32,
    }
}

// A simple resolver used by tests
struct MapResolver(std::collections::HashMap<String, String>);
impl TargetResolver for MapResolver {
    fn resolve(&mut self, path: &str) -> Option<String> {
        self.0.get(path).cloned()
    }
}

/// it should allocate AnimId/PlayerId/InstId monotonically and reset via IdAllocator::reset
#[test]
fn ids_allocator_basics() {
    let mut alloc = IdAllocator::new();
    assert_eq!(alloc.alloc_anim().0, 0);
    assert_eq!(alloc.alloc_anim().0, 1);
    assert_eq!(alloc.alloc_player().0, 0);
    assert_eq!(alloc.alloc_player().0, 1);
    assert_eq!(alloc.alloc_inst().0, 0);
    assert_eq!(alloc.alloc_inst().0, 1);
    alloc.reset();
    assert_eq!(alloc.alloc_anim().0, 0);
}

/// it should sample linear/step/bezier correctly at representative points
#[test]
fn sampling_linear_step_bezier() {
    // Linear scalar 0..1 over [0,1]
    let track_lin = mk_scalar_track_linear("node.value", &[(0.0, 0.0), (1.0, 1.0)]);
    if let TrackValue::Float(v) = sample_track(&track_lin, 0.5) {
        approx(v, 0.5, 1e-6);
    } else {
        panic!();
    }
    if let TrackValue::Float(v) = sample_track(&track_lin, 0.0) {
        approx(v, 0.0, 1e-6);
    } else {
        panic!();
    }
    if let TrackValue::Float(v) = sample_track(&track_lin, 1.0) {
        approx(v, 1.0, 1e-6);
    } else {
        panic!();
    }

    // Step holds first until next key for Bool/Text kinds
    let bool_track = Track {
        id: "t-bool".into(),
        name: "node.step".into(),
        animatable_id: "node.step".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: TrackValue::Bool(true),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: TrackValue::Bool(false),
                transitions: None,
            },
        ],
        settings: None,
    };
    if let TrackValue::Bool(v) = sample_track(&bool_track, 0.5) {
        assert!(v);
    } else {
        panic!();
    }

    // Bezier ease-in-out via default control points
    let track_bezier_default = Track {
        id: "t-bezier".into(),
        name: "node.bezier".into(),
        animatable_id: "node.bezier".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: TrackValue::Float(0.0),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: TrackValue::Float(1.0),
                transitions: None,
            },
        ],
        settings: None,
    };
    if let TrackValue::Float(v) = sample_track(&track_bezier_default, 0.5) {
        assert!(v > 0.4 && v < 0.6, "bezier mid expected near 0.5 got {v}");
    } else {
        panic!();
    }
}

#[test]
fn accumulates_vector_values_componentwise() {
    let mut acc = AccumulatorWithDerivatives::new();
    acc.reset(1);
    acc.add(0, &TrackValue::Vector(vec![0.0, 2.0, 4.0]), None, 1.0);
    acc.add(0, &TrackValue::Vector(vec![2.0, 0.0, 2.0]), None, 1.0);
    let (value, derivative) = &acc.take(0).expect("vector entry");
    assert!(derivative.is_none());
    if let Value::ArrayF32(data) = value {
        assert_eq!(data, &vec![1.0, 1.0, 3.0]);
    } else {
        panic!("expected vector average, got {value:?}");
    }

    let mut acc_list = AccumulatorWithDerivatives::new();
    acc_list.reset(1);
    acc_list.add(0, &TrackValue::NumericArray(vec![0.0, 4.0]), None, 1.0);
    acc_list.add(0, &TrackValue::NumericArray(vec![2.0, 0.0]), None, 1.0);
    let (list_value, list_deriv) = &acc_list.take(0).expect("list entry");
    assert!(list_deriv.is_none());
    // Sequences are one ArrayValue kind: the old List/Tuple distinction is
    // gone, so the averaged result comes back as an ArrayValue of floats.
    match list_value {
        Value::ArrayValue(items) => {
            let floats: Vec<f32> = items
                .iter()
                .map(|v| match v {
                    Value::F32(f) => *f,
                    _ => panic!("expected numeric list component, got {v:?}"),
                })
                .collect();
            assert_eq!(floats, vec![1.0, 2.0]);
        }
        other => panic!("expected list average, got {other:?}"),
    }
}

#[test]
fn sampling_derivative_linear_and_step() {
    let track_lin = mk_scalar_track_linear("node.value", &[(0.0, 0.0), (1.0, 1.0)]);
    let sample = sample_track_with_derivative(&track_lin, 0.5, 1.0);
    if let TrackValue::Float(v) = sample.0 {
        approx(v, 0.5, 1e-6);
    } else {
        panic!();
    }
    if let Some(TrackValue::Float(dv)) = sample.1 {
        approx(dv, 1.0, 1e-6);
    } else {
        panic!();
    }

    let bool_track = Track {
        id: "t-bool".into(),
        name: "node.step".into(),
        animatable_id: "node.step".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: TrackValue::Bool(true),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: TrackValue::Bool(false),
                transitions: None,
            },
        ],
        settings: None,
    };
    let step_sample = sample_track_with_derivative(&bool_track, 0.5, 1.0);
    if let TrackValue::Bool(v) = step_sample.0 {
        assert!(v);
    } else {
        panic!();
    }
    // TODO: Determine what behavior is appropriate for derivative of boolean track
    // if let Some(TrackValue::Float(dv)) = step_sample.1 {
    //     approx(dv, 0.0, 1e-6);
    // } else {
    //     panic!("{:?}",step_sample);
    // }
}

/// it should nlerp quaternions and keep unit norm at midpoints
#[test]
fn sampling_quat_nlerp_shortest_arc() {
    // 180 deg around Y: from [0,0,0,1] to [0,1,0,0]
    let track = mk_quat_track_linear(
        "node.rot",
        &[(0.0, [0.0, 0.0, 0.0, 1.0]), (1.0, [0.0, 1.0, 0.0, 0.0])],
    );
    if let TrackValue::Quat(q) = sample_track(&track, 0.5) {
        let n = norm4(q);
        approx(n, 1.0, 1e-4);
    } else {
        panic!();
    }
}

/// it should resolve handles via prebind and fallback to canonical path when not bound
#[test]
fn binding_prebind_and_fallback() {
    let track = mk_scalar_track_linear("node.a", &[(0.0, 1.0), (1.0, 1.0)]);
    let anim = mk_anim("a", 1.0, vec![track]);
    let mut eng = Engine::new(Config::default());
    let anim_id = eng.load_animation(anim);
    let pid = eng.create_player("p");
    let _iid = eng.add_instance(pid, anim_id, InstanceCfg::default());

    let mut map = std::collections::HashMap::new();
    map.insert("node.a".to_string(), "HANDLE_A".to_string());
    let mut resolver = MapResolver(map);
    eng.prebind(&mut resolver);

    // On update, the key should be the resolved handle
    let out = eng.update(0.0, Inputs::default());
    assert!(!out.changes.is_empty());
    let keys: Vec<_> = out.changes.iter().map(|c| c.key.as_str()).collect();
    assert!(keys.contains(&"HANDLE_A"));

    // No prebind on another target: expect canonical path
    let track2 = mk_scalar_track_linear("node.fallback", &[(0.0, 1.0), (1.0, 1.0)]);
    let anim2 = mk_anim("b", 1.0, vec![track2]);
    let aid2 = eng.load_animation(anim2);
    let _iid2 = eng.add_instance(pid, aid2, InstanceCfg::default());
    let out2 = eng.update(0.0, Inputs::default());
    let keys2: Vec<_> = out2.changes.iter().map(|c| c.key.as_str()).collect();
    assert!(keys2.contains(&"node.fallback"));
}

/// it should handle Once/Loop/PingPong, window clamp, and seek behavior
#[test]
fn engine_loop_modes_and_window_and_seek() {
    // value = u (0..1)
    let track = mk_scalar_track_linear("node.t", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 1.0, vec![track]);
    let mut eng = Engine::new(Config::default());
    let anim_id = eng.load_animation(anim);
    let pid = eng.create_player("p");
    let _iid = eng.add_instance(pid, anim_id, InstanceCfg::default());

    // Once mode + window [0.2, 0.8], seek 1.0 clamps to 0.8
    let mut inputs = Inputs::default();
    inputs.player_cmds.push(PlayerCommand::SetLoopMode {
        player: pid,
        mode: LoopMode::Once,
    });
    inputs.player_cmds.push(PlayerCommand::SetWindow {
        player: pid,
        start_time: 0.2,
        end_time: Some(0.8),
    });
    inputs.player_cmds.push(PlayerCommand::Seek {
        player: pid,
        time: 1.0,
    });
    let out = eng.update(0.0, inputs);
    // expect value ~ 0.8
    let val = out
        .changes
        .iter()
        .find(|c| c.key == "node.t")
        .expect("change")
        .value
        .clone();
    if let Value::F32(v) = val {
        approx(v, 0.8, 1e-6);
    } else {
        panic!();
    }

    // Loop mode wraps within the window [0.2, 0.8): seek -0.25 wraps to 0.35
    let mut inputs2 = Inputs::default();
    inputs2.player_cmds.push(PlayerCommand::SetLoopMode {
        player: pid,
        mode: LoopMode::Loop,
    });
    inputs2.player_cmds.push(PlayerCommand::Seek {
        player: pid,
        time: -0.25,
    });
    let out2 = eng.update(0.0, inputs2);
    let val2 = out2
        .changes
        .iter()
        .find(|c| c.key == "node.t")
        .unwrap()
        .value
        .clone();
    if let Value::F32(v) = val2 {
        approx(v, 0.35, 1e-6);
    } else {
        panic!();
    }
}

/// it should reflect with PingPong and map 1.25 -> 0.75 for a 1s clip
#[test]
fn pingpong_reflection_mapping() {
    let track = mk_scalar_track_linear("node.pp", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 1.0, vec![track]);
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(anim);
    let p = eng.create_player("p");
    let _ = eng.add_instance(p, a, InstanceCfg::default());

    let mut inputs = Inputs::default();
    inputs.player_cmds.push(PlayerCommand::SetLoopMode {
        player: p,
        mode: LoopMode::PingPong,
    });
    inputs.player_cmds.push(PlayerCommand::Seek {
        player: p,
        time: 0.75,
    });
    let out = eng.update(0.0, inputs);
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.pp")
        .unwrap()
        .value
        .clone();
    if let Value::F32(s) = v {
        approx(s, 0.75, 1e-6);
    } else {
        panic!();
    }

    // reflect at > 1.0: 1.25 -> 0.75
    let mut inputs2 = Inputs::default();
    inputs2.player_cmds.push(PlayerCommand::Seek {
        player: p,
        time: 1.25,
    });
    let out2 = eng.update(0.0, inputs2);
    let v2 = out2
        .changes
        .iter()
        .find(|c| c.key == "node.pp")
        .unwrap()
        .value
        .clone();
    if let Value::F32(s) = v2 {
        approx(s, 0.75, 1e-6);
    } else {
        panic!();
    }
}

/// it should produce a static pose when time_scale=0 mapping to start_offset
#[test]
fn time_scale_zero_static_pose() {
    let track = mk_scalar_track_linear("node.static", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 1.0, vec![track]);
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(anim);
    let p = eng.create_player("p");
    // time_scale=0 -> local_t = start_offset always
    let _ = eng.add_instance(
        p,
        a,
        InstanceCfg {
            time_scale: 0.0,
            start_offset: 0.7,
            ..Default::default()
        },
    );

    let mut inputs = Inputs::default();
    inputs.player_cmds.push(PlayerCommand::Seek {
        player: p,
        time: 0.1,
    });
    let out = eng.update(0.0, inputs);
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.static")
        .unwrap()
        .value
        .clone();
    if let Value::F32(s) = v {
        approx(s, 0.7, 1e-5);
    } else {
        panic!();
    }

    let mut inputs2 = Inputs::default();
    inputs2.player_cmds.push(PlayerCommand::Seek {
        player: p,
        time: 10.0,
    });
    let out2 = eng.update(0.0, inputs2);
    let v2 = out2
        .changes
        .iter()
        .find(|c| c.key == "node.static")
        .unwrap()
        .value
        .clone();
    if let Value::F32(s) = v2 {
        approx(s, 0.7, 1e-5);
    } else {
        panic!();
    }
}

/// it should skip disabled instances in accumulation
#[test]
fn disabled_instance_skipped() {
    let track0 = mk_scalar_track_linear("node.d", &[(0.0, 0.0), (1.0, 0.0)]);
    let track1 = mk_scalar_track_linear("node.d", &[(0.0, 1.0), (1.0, 1.0)]);
    let anim0 = mk_anim("a0", 1.0, vec![track0]);
    let anim1 = mk_anim("a1", 1.0, vec![track1]);

    let mut eng = Engine::new(Config::default());
    let a0 = eng.load_animation(anim0);
    let a1 = eng.load_animation(anim1);
    let p = eng.create_player("p");
    let _i0 = eng.add_instance(
        p,
        a0,
        InstanceCfg {
            weight: 1.0,
            enabled: true,
            ..Default::default()
        },
    );
    let _i1 = eng.add_instance(
        p,
        a1,
        InstanceCfg {
            weight: 1.0,
            enabled: false,
            ..Default::default()
        },
    );
    let out = eng.update(0.0, Inputs::default());
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.d")
        .unwrap()
        .value
        .clone();
    if let Value::F32(s) = v {
        approx(s, 0.0, 1e-6);
    } else {
        panic!();
    }
}

/// it should exercise Outputs API basics: clear/empty/push
#[test]
fn outputs_api_basics() {
    let mut out = Outputs::default();
    assert!(out.is_empty());
    out.push_change(vizij_animation_core::outputs::Change {
        player: PlayerId(0),
        key: "a".into(),
        value: vizij_api_core::value::float(1.0),
    });
    assert!(!out.is_empty());
    out.clear();
    assert!(out.is_empty());
}

/// it should allow manually pushing events and reflect non-empty
#[test]
fn outputs_push_event_manual() {
    let mut out = Outputs::default();
    out.push_event(CoreEvent::PlaybackPaused {
        player: PlayerId(1),
    });
    assert!(!out.events.is_empty());
    assert!(!out.is_empty());
}

/// it should blend accumulated values and derivatives using weights
#[test]
fn accumulator_blends_values_and_derivatives() {
    let mut accum = AccumulatorWithDerivatives::new();
    accum.reset(3);
    let (key, flag) = (0, 1);
    accum.add(
        key,
        &TrackValue::Float(1.0),
        Some(&TrackValue::Float(4.0)),
        0.25,
    );
    accum.add(
        key,
        &TrackValue::Float(3.0),
        Some(&TrackValue::Float(2.0)),
        0.75,
    );
    // Derivatives omitted should not register.
    accum.add(flag, &TrackValue::Bool(true), None, 1.0);

    let (value, derivative) = accum.take(key).expect("blended scalar value present");
    if let Value::F32(v) = value {
        approx(v, 2.5, 1e-6);
    } else {
        panic!("expected blended float value");
    }
    if let Some(Value::F32(dv)) = derivative {
        approx(dv, 2.5, 1e-6);
    } else {
        panic!("expected blended float derivative");
    }
    let (_, flag_derivative) = accum.take(flag).expect("flag change should be present");
    assert!(flag_derivative.is_none());
    assert!(
        accum.take(2).is_none(),
        "nothing weighed on the third output"
    );
    assert!(accum.take(key).is_none(), "taking empties the output");
}

/// it should emit derivatives when requesting update_values_and_derivatives()
#[test]
fn engine_update_includes_derivatives() {
    let track = mk_scalar_track_linear("node.s", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 1.0, vec![track]);
    let mut engine = Engine::new(Config::default());
    let anim_id = engine.load_animation(anim);
    let player_id = engine.create_player("p");
    let _inst_id = engine.add_instance(player_id, anim_id, InstanceCfg::default());

    let (derivative_len, has_derivative) = {
        let out = engine.update_values_and_derivatives(0.016, Inputs::default());
        (
            out.changes.len(),
            out.changes.iter().any(|c| c.derivative.is_some()),
        )
    };
    assert!(derivative_len > 0);
    assert!(has_derivative);
    // Outputs struct should mirror values-only list
    let values_only = engine.update_values(0.016, Inputs::default());
    assert_eq!(values_only.changes.len(), derivative_len);
}

/// it should bake values at frame_rate and match sampler within epsilon
#[test]
fn baking_matches_sampling_and_counts() {
    // Linear 0..1 over [0,1]
    let track = mk_scalar_track_linear("node.s", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 1.0, vec![track.clone()]);
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(anim);

    let cfg = BakingConfig {
        frame_rate: 60.0,
        start_time: 0.0,
        end_time: Some(1.0),
        ..Default::default()
    };
    let baked = vizij_animation_core::baking::bake_animation_data(
        a,
        &mk_anim("clip", 1.0, vec![track.clone()]),
        &cfg,
    )
    .expect("a bake within the bound");
    assert_eq!(baked.frame_rate, 60.0);
    assert_eq!(baked.start_time, 0.0);
    approx(baked.end_time, 1.0, 1e-6);
    let expected_samples =
        (cfg.frame_rate * (cfg.end_time.unwrap() - cfg.start_time)).ceil() as usize + 1;
    assert_eq!(baked.tracks.len(), 1);
    assert_eq!(baked.tracks[0].values.len(), expected_samples);

    // Check a couple of points match sampling
    if let Value::F32(v0) = baked.tracks[0].values[0].clone() {
        approx(v0, 0.0, 1e-6);
    } else {
        panic!();
    }
    let mid_idx = expected_samples / 2;
    if let Value::F32(vm) = baked.tracks[0].values[mid_idx].clone() {
        approx(vm, (mid_idx as f32) / 60.0, 1e-2); // linear over [0,1]
    } else {
        panic!();
    }

    // Export JSON shape
    let j = export_baked_json(&baked);
    assert!(j.is_object());
}

/// it should align value/derivative tracks in bake bundle output
#[test]
fn baking_with_derivatives_aligns_tracks() {
    let track = mk_scalar_track_linear("node.s", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 1.0, vec![track.clone()]);

    let cfg = BakingConfig {
        frame_rate: 30.0,
        start_time: 0.0,
        end_time: Some(1.0),
        ..Default::default()
    };

    let (values, derivatives) =
        vizij_animation_core::baking::bake_animation_data_with_derivatives(AnimId(0), &anim, &cfg)
            .expect("a bake within the bound");

    assert_eq!(values.tracks.len(), derivatives.tracks.len());
    let values_only =
        vizij_animation_core::baking::bake_animation_data(AnimId(0), &anim, &cfg).expect("bake");
    assert_eq!(
        values_only.tracks[0].values, values.tracks[0].values,
        "a values-only bake samples the same values"
    );
    for (v_track, d_track) in values.tracks.iter().zip(derivatives.tracks.iter()) {
        assert_eq!(v_track.target_path, d_track.target_path);
        assert_eq!(v_track.values.len(), d_track.values.len());
    }
    let derivative_samples = &derivatives.tracks[0].values;
    assert!(derivative_samples.iter().any(|entry| entry.is_some()));
}

/// A bake whose window at its frame rate takes more than `MAX_BAKE_SAMPLES`
/// samples over all tracks is refused before anything is allocated, at any
/// rate; derivative samples count; a window starting past the clip's end
/// bakes the end; an animation not loaded is named in the refusal.
#[test]
fn baking_refuses_more_samples_than_the_bound() {
    use vizij_animation_core::{BakeError, MAX_BAKE_SAMPLES};
    let tracks: Vec<Track> = (0..4)
        .map(|i| mk_scalar_track_linear(&format!("node.{i}"), &[(0.0, 0.0), (1.0, 1.0)]))
        .collect();
    let anim = mk_anim("clip", 1.0, tracks);
    let bake = |frame_rate: f32| {
        let cfg = BakingConfig {
            frame_rate,
            ..Default::default()
        };
        vizij_animation_core::baking::bake_animation_data(AnimId(0), &anim, &cfg)
    };
    for rate in [1e9, f32::MAX] {
        assert!(
            matches!(bake(rate), Err(BakeError::TooManySamples { .. })),
            "{rate} Hz"
        );
    }
    // 4 tracks over 1 s: (rate + 1) frames each.
    let at_bound = (MAX_BAKE_SAMPLES / 4 - 1) as f32;
    assert_eq!(
        bake(at_bound).expect("at the bound").tracks[0].values.len(),
        MAX_BAKE_SAMPLES / 4
    );
    assert!(bake(at_bound + 1.0).is_err(), "one frame past it");
    let cfg = BakingConfig {
        frame_rate: at_bound,
        ..Default::default()
    };
    assert!(
        vizij_animation_core::baking::bake_animation_data_with_derivatives(AnimId(0), &anim, &cfg)
            .is_err(),
        "derivatives double the samples"
    );
    let half = BakingConfig {
        frame_rate: (MAX_BAKE_SAMPLES / 8 - 1) as f32,
        ..Default::default()
    };
    let (values, derivatives) =
        vizij_animation_core::baking::bake_animation_data_with_derivatives(AnimId(0), &anim, &half)
            .expect("values and derivatives at the bound");
    assert_eq!(
        values.tracks[0].values.len() + derivatives.tracks[0].values.len(),
        MAX_BAKE_SAMPLES / 4
    );

    // An inexact duration keeps its frame count: 0.1 s at 10 Hz is 2 frames.
    let short = mk_anim(
        "short",
        0.1,
        vec![mk_scalar_track_linear("node.s", &[(0.0, 0.0), (1.0, 1.0)])],
    );
    let ten = BakingConfig {
        frame_rate: 10.0,
        ..Default::default()
    };
    let baked = vizij_animation_core::baking::bake_animation_data(AnimId(0), &short, &ten)
        .expect("a short bake");
    assert_eq!(baked.tracks[0].values.len(), 2);

    // No tracks: nothing to sample at any rate.
    let empty = mk_anim("empty", 1.0, Vec::new());
    let huge = BakingConfig {
        frame_rate: f32::MAX,
        ..Default::default()
    };
    let baked = vizij_animation_core::baking::bake_animation_data(AnimId(0), &empty, &huge)
        .expect("an empty bake");
    assert!(baked.tracks.is_empty());

    // Out-of-range windows clamp into the clip; a rate that is not positive
    // and finite is 60 Hz.
    for (start_time, end_time, frame_rate) in [
        (f32::NAN, Some(f32::NAN), f32::NAN),
        (-1.0, Some(f32::INFINITY), -5.0),
        (0.5, Some(0.25), 0.0),
    ] {
        let cfg = BakingConfig {
            frame_rate,
            start_time,
            end_time,
            derivative_epsilon: Some(f32::NAN),
        };
        let baked = vizij_animation_core::baking::bake_animation_data(AnimId(0), &anim, &cfg)
            .expect("a clamped bake");
        assert_eq!(baked.frame_rate, 60.0);
        assert!(baked.start_time <= baked.end_time);
    }

    let late = BakingConfig {
        start_time: 5.0,
        ..Default::default()
    };
    let baked = vizij_animation_core::baking::bake_animation_data(AnimId(0), &anim, &late)
        .expect("a window past the end");
    assert_eq!((baked.start_time, baked.end_time), (1.0, 1.0));
    assert_eq!(baked.tracks[0].values, vec![Value::F32(1.0)]);

    let eng = Engine::new(Config::default());
    assert_eq!(
        eng.bake_animation(AnimId(7), &BakingConfig::default())
            .map(|_| ()),
        Err(BakeError::NotLoaded(AnimId(7)))
    );
}

// // it should bake animations through the engine facade using the same sampler
// #[test]
// fn engine_bake_animation_matches_standalone() {
//     let track = mk_scalar_track_linear("node.s", &[(0.0, 0.0), (1.0, 1.0)]);
//     let anim = mk_anim("clip", 1.0, vec![track.clone()]);
//     let mut eng = Engine::new(Config::default());
//     let aid = eng.load_animation(anim.clone());

//     let cfg = BakingConfig {
//         frame_rate: 24.0,
//         start_time: 0.0,
//         end_time: Some(1.0),
//     };

//     let baked_direct = vizij_animation_core::baking::bake_animation_data(aid, &anim, &cfg);
//     let baked_via_engine = eng
//         .bake_animation(aid, &cfg)
//         .expect("engine should know the animation");

//     let json_direct = serde_json::to_value(&baked_direct).unwrap();
//     let json_via_engine = serde_json::to_value(&baked_via_engine).unwrap();
//     assert_eq!(json_direct, json_via_engine);

//     let exported = eng
//         .bake_animation(aid, &cfg)
//         .expect("json export available");
//     assert_eq!(
//         exported,
//         vizij_animation_core::baking::export_baked_json(&baked_direct)
//     );
// }

/// it should produce identical Outputs for the same dt sequence (determinism)
#[test]
fn determinism_same_sequence_same_outputs() {
    // Simple anim with constant 0.5
    let track = mk_scalar_track_linear("node.k", &[(0.0, 0.5), (1.0, 0.5)]);
    let anim = mk_anim("clip", 1.0, vec![track.clone()]);

    let mut e1 = Engine::new(Config::default());
    let mut e2 = Engine::new(Config::default());
    let a1 = e1.load_animation(anim.clone());
    let a2 = e2.load_animation(anim);
    let p1 = e1.create_player("p");
    let p2 = e2.create_player("p");
    let _ = e1.add_instance(p1, a1, InstanceCfg::default());
    let _ = e2.add_instance(p2, a2, InstanceCfg::default());

    // Same dt sequence
    let seq = [0.016, 0.016, 0.016, 0.032, 0.0, 0.1];
    for dt in seq {
        let o1 = e1.update(dt, Inputs::default());
        let o2 = e2.update(dt, Inputs::default());
        // Compare serialized JSON to avoid implementing Eq; allow exact equality for constants
        let j1 = serde_json::to_string(o1).unwrap();
        let j2 = serde_json::to_string(o2).unwrap();
        assert_eq!(j1, j2);
    }
}

/// it should normalize the final quaternion when blending two quat contributions on the same key
#[test]
fn multi_quat_blend_normalized() {
    // Two quats roughly 90 degrees apart
    let q0 = [0.0, 0.0, 0.0, 1.0];
    let q1 = [0.0, 0.70710677, 0.0, 0.70710677];

    let t0 = mk_quat_track_linear("node.q", &[(0.0, q0), (1.0, q0)]);
    let t1 = mk_quat_track_linear("node.q", &[(0.0, q1), (1.0, q1)]);
    let a0 = mk_anim("a0", 1.0, vec![t0]);
    let a1 = mk_anim("a1", 1.0, vec![t1]);

    let mut eng = Engine::new(Config::default());
    let id0 = eng.load_animation(a0);
    let id1 = eng.load_animation(a1);
    let p = eng.create_player("p");
    // Two instances contributing to the same target path
    let _i0 = eng.add_instance(
        p,
        id0,
        InstanceCfg {
            weight: 0.5,
            ..Default::default()
        },
    );
    let _i1 = eng.add_instance(
        p,
        id1,
        InstanceCfg {
            weight: 0.5,
            ..Default::default()
        },
    );

    let out = eng.update(0.0, Inputs::default());
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.q")
        .unwrap()
        .value
        .clone();
    if let Some(qb) = as_quat(&v) {
        let n = norm4(qb);
        approx(n, 1.0, 1e-4);
    } else {
        panic!();
    }
}

/// it should sample outside ranges (hold ends), and single-key tracks return constant; engine skips empty-key track changes
#[test]
fn sampling_boundaries_single_and_empty() {
    // Outside ranges hold ends: keys at 0.25->2.0 and 0.75->4.0
    let track = mk_scalar_track_linear("node.bound", &[(0.0, 2.0), (1.0, 4.0)]);
    if let TrackValue::Float(v) = sample_track(&track, 0.0) {
        approx(v, 2.0, 1e-6)
    } else {
        panic!()
    }
    if let TrackValue::Float(v) = sample_track(&track, 1.0) {
        approx(v, 4.0, 1e-6)
    } else {
        panic!()
    }

    // Single-key track returns that key
    let single = Track {
        id: "t-single".into(),
        name: "single".into(),
        animatable_id: "node.single".into(),
        points: vec![Keypoint {
            id: "k".into(),
            stamp: 0.5,
            value: TrackValue::Float(7.0),
            transitions: None,
        }],
        settings: None,
    };
    if let TrackValue::Float(v) = sample_track(&single, 0.0) {
        approx(v, 7.0, 1e-6)
    } else {
        panic!()
    }
    if let TrackValue::Float(v) = sample_track(&single, 2.0) {
        approx(v, 7.0, 1e-6)
    } else {
        panic!()
    }

    // Empty keys: engine skips emitting Changes for empty-key tracks
    let empty = Track {
        id: "t-empty".into(),
        name: "empty".into(),
        animatable_id: "node.empty".into(),
        points: vec![],
        settings: None,
    };
    let anim = mk_anim("clip", 1.0, vec![empty]);
    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let p = eng.create_player("p");
    let _ = eng.add_instance(p, aid, InstanceCfg::default());
    let out = eng.update(0.0, Inputs::default());
    assert!(!out.changes.iter().any(|c| c.key == "node.empty"));
}

/// it should build BindingSet channels equal to animation.tracks and indices match track order
#[test]
fn binding_set_channels_len_and_indices() {
    let t0 = mk_scalar_track_linear("node.a", &[(0.0, 1.0), (1.0, 1.0)]);
    let t1 = mk_scalar_track_linear("node.b", &[(0.0, 2.0), (1.0, 2.0)]);
    let anim = mk_anim("clip", 1.0, vec![t0, t1]);

    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let p = eng.create_player("p");
    let iid = eng.add_instance(p, aid, InstanceCfg::default());

    let chs = eng.get_instance_channels(iid).expect("channels");
    assert_eq!(chs.len(), 2);
    assert_eq!(chs[0].track_idx, 0);
    assert_eq!(chs[1].track_idx, 1);
}

/// it should recompute player_total_duration on instance updates, and leave it under a window
#[test]
fn total_duration_follows_instances_not_the_window() {
    let t = mk_scalar_track_linear("node.t", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 10.0, vec![t]);

    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let p = eng.create_player("p");
    let i = eng.add_instance(
        p,
        aid,
        InstanceCfg {
            time_scale: 1.0,
            start_offset: 0.0,
            ..Default::default()
        },
    );
    // Full span is 10s
    assert!((eng.player_total_duration(p).unwrap() - 10.0).abs() < 1e-6);

    // A window [2,5] bounds the playhead; the length stays 10s
    let mut inputs = Inputs::default();
    inputs.player_cmds.push(PlayerCommand::SetWindow {
        player: p,
        start_time: 2.0,
        end_time: Some(5.0),
    });
    let _ = eng.update(0.0, inputs);
    assert!((eng.player_total_duration(p).unwrap() - 10.0).abs() < 1e-6);

    // An instance starting at 9s ends at 9 + 10
    let mut inputs2 = Inputs::default();
    inputs2.instance_updates.push(InstanceUpdate {
        player: p,
        inst: i,
        weight: None,
        time_scale: None,
        start_offset: Some(9.0),
        enabled: None,
    });
    let _ = eng.update(0.0, inputs2);
    assert!((eng.player_total_duration(p).unwrap() - 19.0).abs() < 1e-6);
}

/// it should hold time at speed 0 while playing, keep the speed through Play, and Stop reset to start_time
#[test]
fn speed_play_stop_controls() {
    let t = mk_scalar_track_linear("node.t", &[(0.0, 0.0), (1.0, 1.0)]);
    let anim = mk_anim("clip", 10.0, vec![t]);
    let mut eng = Engine::new(Config::default());
    let aid = eng.load_animation(anim);
    let p = eng.create_player("p");
    let _ = eng.add_instance(p, aid, InstanceCfg::default());
    let sample = |out: &Outputs| match out
        .changes
        .iter()
        .find(|c| c.key == "node.t")
        .unwrap()
        .value
    {
        Value::F32(s) => s,
        ref other => panic!("expected F32, got {other:?}"),
    };

    // Speed 0 holds time, the player still playing.
    let mut inputs = Inputs::default();
    inputs.player_cmds.push(PlayerCommand::SetSpeed {
        player: p,
        speed: 0.0,
    });
    let _ = eng.update(0.0, inputs);
    let _ = eng.update(1.0, Inputs::default());
    approx(sample(eng.update(0.0, Inputs::default())), 0.0, 1e-6);
    assert_eq!(eng.list_players()[0].state, PlaybackState::Playing);

    // Play keeps the speed it was given: time still holds.
    let mut inputs2 = Inputs::default();
    inputs2.player_cmds.push(PlayerCommand::Play { player: p });
    let _ = eng.update(1.0, inputs2);
    approx(sample(eng.update(0.0, Inputs::default())), 0.0, 1e-6);

    // Speed 1 advances it: with duration_ms=10s and dt=1s, normalized u ~= 0.1.
    let mut inputs3 = Inputs::default();
    inputs3.player_cmds.push(PlayerCommand::SetSpeed {
        player: p,
        speed: 1.0,
    });
    let _ = eng.update(1.0, inputs3);
    approx(sample(eng.update(0.0, Inputs::default())), 0.1, 1e-3);

    // Stop resets to start_time (0.0)
    let mut inputs4 = Inputs::default();
    inputs4.player_cmds.push(PlayerCommand::Stop { player: p });
    let _ = eng.update(0.0, inputs4);
    approx(sample(eng.update(0.0, Inputs::default())), 0.0, 1e-6);
}

/// it should produce empty Outputs on update when engine has no data
#[test]
fn update_with_no_data_is_safe_and_empty() {
    let mut eng = Engine::new(Config::default());
    let out = eng.update(0.016, Inputs::default());
    assert!(out.changes.is_empty() && out.events.is_empty());
}

/// it should ignore mismatched kinds for the same target and not panic
#[test]
fn mixed_kind_same_target_safe() {
    // Track A: Scalar on "node.mixed", Track B: Vec3 on same "node.mixed"
    let ta = mk_scalar_track_linear("node.mixed", &[(0.0, 1.0), (1.0, 1.0)]);
    let tb = Track {
        id: "tB".into(),
        name: "v3".into(),
        animatable_id: "node.mixed".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: TrackValue::Vec3([0.0, 0.0, 0.0]),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: TrackValue::Vec3([1.0, 1.0, 1.0]),
                transitions: None,
            },
        ],
        settings: None,
    };
    let a0 = mk_anim("a0", 1.0, vec![ta]);
    let a1 = mk_anim("a1", 1.0, vec![tb]);
    let mut eng = Engine::new(Config::default());
    let id0 = eng.load_animation(a0);
    let id1 = eng.load_animation(a1);
    let p = eng.create_player("p");
    let _ = eng.add_instance(
        p,
        id0,
        InstanceCfg {
            weight: 1.0,
            ..Default::default()
        },
    );
    let _ = eng.add_instance(
        p,
        id1,
        InstanceCfg {
            weight: 1.0,
            ..Default::default()
        },
    );

    // Should not panic; engine should ignore mismatched contributions gracefully.
    let _ = eng.update(0.0, Inputs::default());
}

/// it should normalize quaternion when two instances with different orientations contribute to same key
#[test]
fn multi_quat_instances_normalized() {
    let q0 = [0.0, 0.0, 0.0, 1.0];
    let q1 = [0.0, 0.38268343, 0.0, 0.9238795]; // 45 deg around Y
    let t0 = mk_quat_track_linear("node.rot", &[(0.0, q0), (1.0, q0)]);
    let t1 = mk_quat_track_linear("node.rot", &[(0.0, q1), (1.0, q1)]);
    let a0 = mk_anim("a0", 1.0, vec![t0]);
    let a1 = mk_anim("a1", 1.0, vec![t1]);

    let mut eng = Engine::new(Config::default());
    let id0 = eng.load_animation(a0);
    let id1 = eng.load_animation(a1);
    let p = eng.create_player("p");
    let _ = eng.add_instance(
        p,
        id0,
        InstanceCfg {
            weight: 0.3,
            ..Default::default()
        },
    );
    let _ = eng.add_instance(
        p,
        id1,
        InstanceCfg {
            weight: 0.7,
            ..Default::default()
        },
    );
    let out = eng.update(0.0, Inputs::default());
    let v = out
        .changes
        .iter()
        .find(|c| c.key == "node.rot")
        .unwrap()
        .value
        .clone();
    if let Some(qb) = as_quat(&v) {
        approx(norm4(qb), 1.0, 1e-4);
    } else {
        panic!();
    }
}

/// it should bake empty and single-key tracks with expected sequences
#[test]
fn baking_empty_and_single_key_tracks() {
    // Empty track -> baked values length equals expected samples but values come from sampler (which returns neutral 0.0)
    let empty = Track {
        id: "t-empty".into(),
        name: "empty".into(),
        animatable_id: "node.empty".into(),
        points: vec![],
        settings: None,
    };
    let single = Track {
        id: "t-single".into(),
        name: "single".into(),
        animatable_id: "node.single".into(),
        points: vec![Keypoint {
            id: "k".into(),
            stamp: 0.5,
            value: TrackValue::Float(3.14),
            transitions: None,
        }],
        settings: None,
    };
    let anim = mk_anim("clip", 1.0, vec![empty.clone(), single.clone()]);
    let cfg = BakingConfig {
        frame_rate: 10.0,
        start_time: 0.0,
        end_time: Some(1.0),
        ..Default::default()
    };
    let baked =
        vizij_animation_core::baking::bake_animation_data(AnimId(0), &anim, &cfg).expect("bake");
    assert_eq!(baked.tracks.len(), 2);
    // Single-key baked values should all equal the key's value
    assert!(baked.tracks.iter().any(|t| t.target_path == "node.single"
        && t.values
            .iter()
            .all(|v| matches!(v, Value::F32(x) if (*x - 3.14).abs() < 1e-6))));
}

/// it should round-trip Config and selected Value variants through serde
#[test]
fn config_and_value_serde_roundtrip() {
    // Config roundtrip
    let cfg = Config::default();
    let s = serde_json::to_string(&cfg).unwrap();
    let cfg2: Config = serde_json::from_str(&s).unwrap();
    assert!(cfg2.scratch_samples > 0);

    // Value roundtrips through the vocabulary encodings
    let vq = vizij_api_core::value::quat([0.0, 0.0, 0.0, 1.0]);
    let svq = serde_json::to_string(&vq).unwrap();
    let vq2: Value = serde_json::from_str(&svq).unwrap();
    assert_eq!(vq, vq2);

    let vt = vizij_api_core::value::transform(vizij_api_core::Transform {
        translation: [1.0, 2.0, 3.0],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0, 1.0, 1.0],
    });
    let svt = serde_json::to_string(&vt).unwrap();
    let vt2: Value = serde_json::from_str(&svt).unwrap();
    assert_eq!(vt, vt2);

    // Bool roundtrip
    let vb = vizij_api_core::value::bool_(true);
    let svb = serde_json::to_string(&vb).unwrap();
    let vb2: Value = serde_json::from_str(&svb).unwrap();
    assert_eq!(vb, vb2);

    // Text roundtrip
    let vtxt = vizij_api_core::value::text("hello");
    let svtxt = serde_json::to_string(&vtxt).unwrap();
    let vtxt2: Value = serde_json::from_str(&svtxt).unwrap();
    assert_eq!(vtxt, vtxt2);

    // Keypoint roundtrip: TrackValue serializes as the wire-form Value.
    let kp = Keypoint {
        id: "k0".into(),
        stamp: 0.5,
        value: TrackValue::Vec3([1.0, 2.0, 3.0]),
        transitions: None,
    };
    let skp = serde_json::to_string(&kp).unwrap();
    let kp2: Keypoint = serde_json::from_str(&skp).unwrap();
    assert_eq!(kp, kp2);
}

/// Outputs are laid out by structural edits: one per key per player, in
/// player order and then first-write order; `update_by_target` reports each
/// by its index (`None` where nothing weighs on it) and `update_values` the
/// same values as keyed changes, in that order. Transport leaves the layout;
/// structural edits bump its revision.
#[test]
fn outputs_are_laid_out_by_structural_edits() {
    use vizij_animation_core::OutputTarget;
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(mk_anim(
        "a",
        1.0,
        vec![
            mk_scalar_track_linear("k.x", &[(0.0, 1.0), (1.0, 1.0)]),
            mk_scalar_track_linear("k.y", &[(0.0, 2.0), (1.0, 2.0)]),
        ],
    ));
    let b = eng.load_animation(mk_anim(
        "b",
        1.0,
        vec![mk_scalar_track_linear("k.y", &[(0.0, 4.0), (1.0, 4.0)])],
    ));
    let p = eng.create_player("p");
    let q = eng.create_player("q");
    let r0 = eng.output_revision();
    eng.add_instance(q, b, InstanceCfg::default());
    eng.add_instance(p, a, InstanceCfg::default());
    let ib = eng.add_instance(
        p,
        b,
        InstanceCfg {
            weight: 0.0,
            ..InstanceCfg::default()
        },
    );
    assert_ne!(eng.output_revision(), r0);
    let target = |player: PlayerId, key: &str| OutputTarget {
        player,
        key: key.to_string(),
    };
    assert_eq!(
        eng.output_targets(),
        [target(p, "k.x"), target(p, "k.y"), target(q, "k.y")]
    );

    let values = eng.update_by_target(0.1, Inputs::default()).to_vec();
    assert_eq!(
        values,
        [
            Some(Value::F32(1.0)),
            Some(Value::F32(2.0)),
            Some(Value::F32(4.0))
        ]
    );
    let revision = eng.output_revision();
    let mut inputs = Inputs::default();
    inputs.instance_updates.push(InstanceUpdate {
        player: p,
        inst: ib,
        weight: Some(1.0),
        time_scale: None,
        start_offset: None,
        enabled: None,
    });
    let changes: Vec<(String, Value)> = eng
        .update_values(0.0, inputs)
        .changes
        .iter()
        .map(|c| (c.key.clone(), c.value.clone()))
        .collect();
    assert_eq!(
        changes,
        [
            ("k.x".to_string(), Value::F32(1.0)),
            ("k.y".to_string(), Value::F32(3.0)),
            ("k.y".to_string(), Value::F32(4.0)),
        ]
    );
    assert_eq!(
        eng.output_revision(),
        revision,
        "an update is no structural edit"
    );

    assert!(eng.remove_player(q));
    assert_ne!(eng.output_revision(), revision);
    assert_eq!(eng.output_targets(), [target(p, "k.x"), target(p, "k.y")]);
    assert!(eng.unload_animation(a));
    assert_eq!(eng.output_targets(), [target(p, "k.y")]);
    let pause = Inputs {
        player_cmds: vec![PlayerCommand::Pause { player: p }],
        ..Inputs::default()
    };
    assert_eq!(eng.update_by_target(0.0, pause), [Some(Value::F32(4.0))]);
}

/// Replacing an animation's data lays out the outputs its new tracks write;
/// a replacement writing the same keys keeps the revision.
#[test]
fn replacing_an_animation_lays_out_its_new_keys() {
    let mut eng = Engine::new(Config::default());
    let a = eng.load_animation(mk_anim(
        "a",
        1.0,
        vec![mk_scalar_track_linear("k.x", &[(0.0, 1.0), (1.0, 1.0)])],
    ));
    let p = eng.create_player("p");
    eng.add_instance(p, a, InstanceCfg::default());
    let revision = eng.output_revision();
    let keys = |eng: &Engine| -> Vec<String> {
        eng.output_targets().iter().map(|t| t.key.clone()).collect()
    };
    assert_eq!(keys(&eng), ["k.x"]);

    let wider = || {
        mk_anim(
            "b",
            1.0,
            vec![
                mk_scalar_track_linear("k.y", &[(0.0, 2.0), (1.0, 2.0)]),
                mk_scalar_track_linear("k.x", &[(0.0, 3.0), (1.0, 3.0)]),
            ],
        )
    };
    assert!(eng.replace_animation(a, wider()));
    assert_eq!(keys(&eng), ["k.y", "k.x"]);
    let replaced = eng.output_revision();
    assert_ne!(replaced, revision);
    assert_eq!(
        eng.update_by_target(0.0, Inputs::default()),
        [Some(Value::F32(2.0)), Some(Value::F32(3.0))]
    );
    assert!(eng.replace_animation(a, wider()));
    assert_eq!(eng.output_revision(), replaced, "the same keys");
}
