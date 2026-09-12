// The joint ring, ported from Studio's `materials/torus-material.tsx`.
//
// Three bands around one torus: the span already travelled reads solid, the
// rest of the reachable range reads half-lit, and everything the joint cannot
// reach is hatched. Hatching rather than hiding is what makes the range read
// as a portion of a whole turn.
//
// The original takes its angle from the vertex position's `xy`, because
// three.js builds a torus in the XY plane. Bevy builds one in XZ, so the angle
// would have to come from `xz` instead — except that the mesh's own UVs
// already carry it: `u` runs once around the ring and `v` around the tube.
// Reading those is both simpler and independent of which plane the mesh
// happens to be built in.

#import bevy_pbr::forward_io::VertexOutput

struct JointRing {
    // All angles in radians, normalised to [0, 2pi).
    min: f32,
    max: f32,
    current: f32,
    // The scalars are padded out before the vectors that follow.
    _padding: f32,
    color: vec4<f32>,
    background: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> ring: JointRing;

const TAU: f32 = 6.28318530718;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let angle = fract(in.uv.x) * TAU;

    // Skewed across the tube as well as around the ring, so the hatching runs
    // diagonally rather than in rings.
    let stripe = (angle * 14.0 + in.uv.y * 6.0) % 2.0;
    var opacity = 0.4;
    var mix_factor = mix(0.1, 0.5, step(1.0, stripe));

    if ring.min < ring.max {
        if angle >= ring.min && angle < ring.current {
            opacity = 1.0;
            mix_factor = 1.0;
        } else if angle >= ring.current && angle < ring.max {
            opacity = 0.5;
            mix_factor = 0.6;
        }
    } else {
        // The range wraps through zero.
        if angle >= ring.min || angle < ring.current {
            opacity = 1.0;
            mix_factor = 1.0;
        } else if angle >= ring.current || angle < ring.max {
            opacity = 0.5;
            mix_factor = 0.6;
        }
    }

    return vec4<f32>(mix(ring.background.rgb, ring.color.rgb, mix_factor), opacity);
}
