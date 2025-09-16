use vizij_animation_core::{
    data::{Keypoint, Track, Transitions, Vec2},
    sampling::sample_track,
    value::Value,
};

#[test]
fn euler_vec3_bezier_vs_linear_shape() {
    // Treat Euler (r,p,y) as Vec3 per core mapping; compare default Bezier vs Linear at u=0.25
    // Left: [0,0,0] -> Right: [1,2,3]

    // Default Bezier (ease-in-out via sampler defaults: 0.42,0 and 0.58,1)
    let track_bez = Track {
        id: "t-bez".into(),
        name: "Euler Bezier".into(),
        animatable_id: "node.euler".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: Value::Vec3([0.0, 0.0, 0.0]),
                transitions: None,
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: Value::Vec3([1.0, 2.0, 3.0]),
                transitions: None,
            },
        ],
        settings: None,
    };

    // Linear curve encoded via per-point transitions:
    // left.out = (0,0), right.in = (1,1)
    let track_lin = Track {
        id: "t-lin".into(),
        name: "Euler Linear".into(),
        animatable_id: "node.euler".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: Value::Vec3([0.0, 0.0, 0.0]),
                transitions: Some(Transitions {
                    r#in: None,
                    r#out: Some(Vec2 { x: 0.0, y: 0.0 }),
                }),
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: Value::Vec3([1.0, 2.0, 3.0]),
                transitions: Some(Transitions {
                    r#in: Some(Vec2 { x: 1.0, y: 1.0 }),
                    r#out: None,
                }),
            },
        ],
        settings: None,
    };

    let vb = sample_track(&track_bez, 0.25);
    let vl = sample_track(&track_lin, 0.25);
    match (vb, vl) {
        (Value::Vec3(b), Value::Vec3(l)) => {
            // Ease-in-out should be slightly less than linear at u=0.25
            assert!(b[0] < l[0] + 1e-4, "x bez {} >= lin {}", b[0], l[0]);
            assert!(b[1] < l[1] + 1e-4, "y bez {} >= lin {}", b[1], l[1]);
            assert!(b[2] < l[2] + 1e-4, "z bez {} >= lin {}", b[2], l[2]);
        }
        _ => panic!("expected Vec3 values"),
    }
}

#[test]
fn color_bezier_with_explicit_ctrl_points() {
    // Color from black to white with strong ease; at u=0.25 expect below linear 0.25
    let track = Track {
        id: "t-color".into(),
        name: "Color".into(),
        animatable_id: "node.color".into(),
        points: vec![
            Keypoint {
                id: "k0".into(),
                stamp: 0.0,
                value: Value::ColorRgba([0.0, 0.0, 0.0, 1.0]),
                transitions: Some(Transitions {
                    r#in: None,
                    r#out: Some(Vec2 { x: 0.8, y: 0.0 }),
                }),
            },
            Keypoint {
                id: "k1".into(),
                stamp: 1.0,
                value: Value::ColorRgba([1.0, 1.0, 1.0, 1.0]),
                transitions: Some(Transitions {
                    r#in: Some(Vec2 { x: 0.2, y: 1.0 }),
                    r#out: None,
                }),
            },
        ],
        settings: None,
    };

    let v = sample_track(&track, 0.25);
    match v {
        Value::ColorRgba(c) => {
            // With strong ease-in, components should be noticeably below linear 0.25
            assert!(c[0] < 0.25, "r {} !< 0.25", c[0]);
            assert!(c[1] < 0.25, "g {} !< 0.25", c[1]);
            assert!(c[2] < 0.25, "b {} !< 0.25", c[2]);
            // alpha remains 1.0 (interpolated between 1 and 1)
            assert!((c[3] - 1.0).abs() < 1e-6, "alpha {}", c[3]);
        }
        _ => panic!("expected Color value"),
    }
}
