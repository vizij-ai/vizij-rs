// The sliding joint's bar, ported from Studio's `materials/prism-material.tsx`.
//
// The linear twin of `joint_ring.wgsl`, and the same idea stripped of what a
// circle needs: a progress coordinate along the control compared against where
// the joint stands. Travelled reads solid, the rest of the range reads dim.
// Nothing wraps, because a slide has two ends.
//
// The original measures progress from the vertex's local `y`, a box being
// built centred on its own origin and spanning the range. Here it comes from
// the mesh's `uv.y`, which runs the same length — the fragment stage is handed
// UVs rather than a local position, and the two say the same thing.

#import bevy_pbr::forward_io::VertexOutput

struct JointBar {
    // The travel, in metres, and where the joint stands in it.
    min: f32,
    max: f32,
    current: f32,
    _padding: f32,
    color: vec4<f32>,
    background: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(0) var<uniform> bar: JointBar;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let travel = max(bar.max - bar.min, 1e-6);
    let along = 1.0 - in.uv.y;
    let reached = (bar.current - bar.min) / travel;

    if along <= reached {
        return vec4<f32>(bar.color.rgb, 1.0);
    }
    return vec4<f32>(mix(bar.background.rgb, bar.color.rgb, 0.6), 0.5);
}
