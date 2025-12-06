use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;
use vizij_animation_core::{
    parse_stored_animation_json, AnimationData, Config, Engine, Inputs, InstanceCfg, Keypoint,
    Track, Transitions,
};
use vizij_api_core::Value;
use vizij_test_fixtures::animations;

fn make_track(id: usize, keyframes: usize, _duration_ms: u32) -> Track {
    let keyframes = keyframes.max(2);
    let mut points = Vec::with_capacity(keyframes);
    let last = (keyframes - 1) as f32;
    for idx in 0..keyframes {
        let t = idx as f32 / last;
        // Alternate transitions for variety: ease-out then ease-in
        let transitions = if idx == 0 || idx == keyframes - 1 {
            None
        } else if idx % 2 == 0 {
            Some(Transitions {
                r#in: None,
                r#out: Some(vizij_animation_core::data::Vec2 { x: 0.3, y: 0.0 }),
            })
        } else {
            Some(Transitions {
                r#in: Some(vizij_animation_core::data::Vec2 { x: 0.7, y: 1.0 }),
                r#out: None,
            })
        };
        points.push(Keypoint {
            id: format!("k{idx}"),
            stamp: t,
            value: Value::Float(idx as f32),
            transitions,
        });
    }
    Track {
        id: format!("track_{id}"),
        name: format!("Track {id}"),
        animatable_id: match id % 3 {
            0 => format!("rig/joint_{id}.translation"),
            1 => format!("rig/joint_{id}.rotation"),
            _ => format!("rig/joint_{id}.weight"),
        },
        points,
        settings: None,
    }
}

fn synthetic_animation(track_count: usize, keyframes: usize) -> AnimationData {
    let tracks: Vec<Track> = (0..track_count)
        .map(|i| make_track(i, keyframes, 2_000))
        .collect();

    AnimationData {
        id: None,
        name: format!("{track_count} tracks x {keyframes} keys"),
        tracks,
        groups: serde_json::Value::Null,
        duration_ms: 2_000,
    }
}

fn load_fixture_animation(name: &str) -> AnimationData {
    let raw = animations::json(name).unwrap_or_else(|_| panic!("load animation fixture {name}"));
    parse_stored_animation_json(&raw).expect("parse animation fixture")
}

fn bench_animation_steps(c: &mut Criterion) {
    let cases: Vec<(String, AnimationData)> = vec![
        (
            "fixture/pose-quat-transform".into(),
            load_fixture_animation("pose-quat-transform"),
        ),
        ("mixed-16x32".into(), synthetic_animation(16, 32)),
        ("mixed-64x16".into(), synthetic_animation(64, 16)),
        ("mixed-256x8".into(), synthetic_animation(256, 8)),
    ];

    let mut group = c.benchmark_group("animation_step");
    group.sample_size(50);
    for (name, anim) in cases {
        // Cold: setup + single step
        group.bench_with_input(BenchmarkId::new("cold", &name), &anim, |b, anim| {
            b.iter(|| {
                let mut eng = Engine::new(Config::default());
                let anim_id = eng.load_animation(anim.clone());
                let player = eng.create_player("bench");
                eng.add_instance(player, anim_id, InstanceCfg::default());
                let _ = eng.update(black_box(1.0 / 60.0), Inputs::default());
            });
        });

        group.sample_size(10);
        // Amortized per-step: setup + 100 steps @30fps; normalized per step
        group.bench_with_input(
            BenchmarkId::new("amortized_per_step", &name),
            &anim,
            |b, anim| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let mut eng = Engine::new(Config::default());
                        let anim_id = eng.load_animation(anim.clone());
                        let player = eng.create_player("bench");
                        eng.add_instance(player, anim_id, InstanceCfg::default());
                        let start = std::time::Instant::now();
                        for _ in 0..100 {
                            let _ = eng.update(black_box(1.0 / 30.0), Inputs::default());
                        }
                        total += start.elapsed() / 100;
                    }
                    total
                });
            },
        );
        group.sample_size(50);
    }
    group.finish();
}

criterion_group!(benches, bench_animation_steps);
criterion_main!(benches);
