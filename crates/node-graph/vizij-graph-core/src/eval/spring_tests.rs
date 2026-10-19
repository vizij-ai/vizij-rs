//! A spring steps exactly: whatever the length of a step, it ends where that
//! much time leaves a damped spring, its state finite.
//!
//! Each case starts a spring at rest on 1 and moves its target to 0, then
//! steps it: against the closed form of its motion, one long step against
//! many short ones, and steps of a second and a minute — what a page hidden
//! for a while hands its device — on the springs Quori's program runs.

use super::*;
use crate::types::GraphSpec;
use serde_json::json;
use vizij_api_core::Value;

/// A spring of `stiffness`, `damping` and `mass` behind a constant target.
fn spring(stiffness: f32, damping: f32, mass: f32) -> GraphSpec {
    serde_json::from_value::<GraphSpec>(json!({
        "nodes": [
            { "id": "target", "type": "constant", "params": { "value": { "f32": 1.0 } } },
            { "id": "spring", "type": "spring",
              "params": { "stiffness": stiffness, "damping": damping, "mass": mass } }
        ],
        "edges": [
            { "from": { "node_id": "target" }, "to": { "node_id": "spring", "input": "in" } }
        ]
    }))
    .expect("a valid graph spec")
    .with_cache()
}

/// Set the spring's target.
fn aim(spec: &mut GraphSpec, target: f32) {
    spec.nodes[0].params.value = Some(Value::F32(target));
}

/// A spring of `stiffness`, `damping` and `mass` at rest on 1, its target
/// moved to 0, no time passed yet.
fn released(stiffness: f32, damping: f32, mass: f32) -> (GraphRuntime, GraphSpec) {
    let mut spec = spring(stiffness, damping, mass);
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("the spring settles on 1");
    aim(&mut spec, 0.0);
    (rt, spec)
}

/// Advance the graph's clock by `dt` and evaluate it.
fn step(rt: &mut GraphRuntime, spec: &GraphSpec, dt: f32) {
    rt.dt = dt;
    rt.t += dt;
    evaluate_all(rt, spec).expect("the graph evaluates");
}

/// The spring's position and velocity.
fn state(rt: &GraphRuntime) -> (f32, f32) {
    match rt.node_states.get("spring") {
        Some(graph_runtime::NodeRuntimeState::Spring(spring)) => {
            (spring.position[0], spring.velocity[0])
        }
        other => panic!("the spring keeps a spring state, got {other:?}"),
    }
}

/// The spring's output port.
fn output(rt: &GraphRuntime) -> f32 {
    match rt
        .outputs
        .get("spring")
        .and_then(|ports| ports.get("out"))
        .map(|port| &port.value)
    {
        Some(Value::F32(value)) => *value,
        other => panic!("the spring outputs a float, got {other:?}"),
    }
}

/// A spring's `(stiffness, damping, mass)`.
type Params = (f32, f32, f32);

/// A displacement as a function of the time since release.
type Motion = Box<dyn Fn(f64) -> f64>;

/// The displacement at `t` of a spring released at rest from 1 toward 0, in
/// the textbook form of each damping regime: the spring, then the closed
/// form.
fn closed_forms() -> Vec<(Params, Motion)> {
    // Underdamped, Spring's defaults: a = 10, β = √20.
    let beta = 20f64.sqrt();
    let under =
        move |t: f64| (-10.0 * t).exp() * ((beta * t).cos() + 10.0 / beta * (beta * t).sin());
    // Critically damped: a = ω = 10.
    let critical = |t: f64| (-10.0 * t).exp() * (1.0 + 10.0 * t);
    // Overdamped: roots r₁, r₂ of r² + 40r + 10.
    let disc = (40f64 * 40.0 - 40.0).sqrt();
    let (r1, r2) = ((-40.0 + disc) / 2.0, (-40.0 - disc) / 2.0);
    let over = move |t: f64| (r2 * (r1 * t).exp() - r1 * (r2 * t).exp()) / (r2 - r1);
    // Undamped: a cosine of ω = √120 for good.
    let omega = 120f64.sqrt();
    let undamped = move |t: f64| (omega * t).cos();
    vec![
        ((120.0, 20.0, 1.0), Box::new(under)),
        ((100.0, 20.0, 1.0), Box::new(critical)),
        ((10.0, 40.0, 1.0), Box::new(over)),
        ((120.0, 0.0, 1.0), Box::new(undamped)),
    ]
}

#[test]
fn a_spring_follows_its_closed_form_in_short_steps_and_in_one() {
    for ((stiffness, damping, mass), closed) in closed_forms() {
        for t in [0.05f32, 0.25, 1.0] {
            let expected = closed(f64::from(t)) as f32;

            let (mut rt, spec) = released(stiffness, damping, mass);
            step(&mut rt, &spec, t);
            assert!(
                (output(&rt) - expected).abs() < 1.0e-5,
                "k={stiffness} c={damping} m={mass}: one step of {t} s lands on {expected}, got {}",
                output(&rt)
            );

            let (mut rt, spec) = released(stiffness, damping, mass);
            let steps = (t * 240.0).round() as usize;
            for _ in 0..steps {
                step(&mut rt, &spec, t / steps as f32);
            }
            assert!(
                (output(&rt) - expected).abs() < 1.0e-4,
                "k={stiffness} c={damping} m={mass}: {steps} steps to {t} s land on {expected}, got {}",
                output(&rt)
            );
        }
    }
}

#[test]
fn one_long_step_lands_where_many_short_ones_do() {
    let cases = [
        // Spring's defaults, Quori's program's two springs, a spring with no
        // damping, one whose mass rounds up to the least the node allows,
        // and one with no stiffness, which only damps the velocity it has.
        (120.0, 20.0, 1.0),
        (150.0, 30.0, 3.0),
        (120.0, 30.0, 5.0),
        (120.0, 0.0, 1.0),
        (120.0, 20.0, 0.0),
        (0.0, 5.0, 1.0),
    ];
    for (stiffness, damping, mass) in cases {
        // Some motion first, so the long step starts with a velocity too.
        let (mut long, spec) = released(stiffness, damping, mass);
        let (mut short, _) = released(stiffness, damping, mass);
        for rt in [&mut long, &mut short] {
            step(rt, &spec, 0.05);
        }
        step(&mut long, &spec, 1.0);
        for _ in 0..60 {
            step(&mut short, &spec, 1.0 / 60.0);
        }
        let ((long_pos, long_vel), (short_pos, short_vel)) = (state(&long), state(&short));
        assert!(
            (long_pos - short_pos).abs() < 1.0e-4 && (long_vel - short_vel).abs() < 1.0e-3,
            "k={stiffness} c={damping} m={mass}: one 1 s step ends at ({long_pos}, {long_vel}), \
             60 steps of 1/60 s at ({short_pos}, {short_vel})"
        );
    }
}

#[test]
fn steps_of_a_second_and_of_a_minute_keep_the_spring_finite_and_settle_it() {
    for (stiffness, damping, mass) in [(150.0, 30.0, 3.0), (120.0, 30.0, 5.0)] {
        let omega2 = stiffness / mass;
        // Twice the energy per unit mass, which damping only ever lowers.
        let energy = |(pos, vel): (f32, f32)| vel * vel + omega2 * pos * pos;
        for (dt, steps) in [(1.0f32, 40usize), (60.0, 3)] {
            let (mut rt, spec) = released(stiffness, damping, mass);
            let start = energy(state(&rt));
            for n in 1..=steps {
                step(&mut rt, &spec, dt);
                let (pos, vel) = state(&rt);
                assert!(
                    pos.is_finite() && vel.is_finite(),
                    "k={stiffness} c={damping} m={mass}: step {n} of {dt} s leaves ({pos}, {vel})"
                );
                assert!(
                    energy((pos, vel)) <= start * (1.0 + 1.0e-5),
                    "k={stiffness} c={damping} m={mass}: step {n} of {dt} s gains energy"
                );
            }
            let (pos, vel) = state(&rt);
            assert!(
                pos.abs() < 1.0e-5 && vel.abs() < 1.0e-5,
                "k={stiffness} c={damping} m={mass}: {steps} steps of {dt} s settle on 0, \
                 got ({pos}, {vel})"
            );
        }
    }
}

#[test]
fn a_target_that_is_not_a_number_holds_the_spring_and_keeps_its_state_finite() {
    let (mut rt, mut spec) = released(150.0, 30.0, 3.0);
    step(&mut rt, &spec, 0.1);
    let moving = state(&rt);

    aim(&mut spec, f32::NAN);
    step(&mut rt, &spec, 1.0);
    assert_eq!(state(&rt), moving, "the spring holds at a NaN target");
    assert_eq!(output(&rt), moving.0, "and shows where it holds");

    aim(&mut spec, 0.0);
    for _ in 0..60 {
        step(&mut rt, &spec, 1.0 / 30.0);
    }
    let (pos, vel) = state(&rt);
    assert!(
        pos.abs() < 1.0e-3 && vel.abs() < 1.0e-2,
        "the spring moves on toward a target that is a number again, got ({pos}, {vel})"
    );
}

#[test]
fn a_spring_seeded_from_a_target_that_is_not_a_number_starts_again_on_the_next() {
    let mut spec = spring(150.0, 30.0, 3.0);
    aim(&mut spec, f32::NAN);
    let mut rt = GraphRuntime::default();
    evaluate_all(&mut rt, &spec).expect("the spring is seeded");
    step(&mut rt, &spec, 1.0 / 60.0);

    aim(&mut spec, 0.25);
    step(&mut rt, &spec, 1.0 / 60.0);
    assert_eq!(
        state(&rt),
        (0.25, 0.0),
        "the spring starts at rest on its target"
    );
}
