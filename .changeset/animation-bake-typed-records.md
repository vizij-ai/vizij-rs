---
"@vizij/animation-module": major
---

The module (2.0.0) returns a bake as typed records of Arora values instead of a JSON string. `bake(anim, frame_rate?, start_ns?, end_ns?)` and `bake_with_derivatives(…)` return `BakedAnimation? { frame_rate, start_ns, end_ns, tracks: [BakedTrack { animatable_id, values, derivatives }] }`: `values` is a `Value::ArrayValue` with each frame's sample as a dynamic value, as `Keypoint::value` is, and `derivatives` one `Value::Option` per frame from `bake_with_derivatives`, empty from `bake`. An animation not loaded, or a bake beyond 2²⁰ samples, returns none, where it returned an empty string. The window arguments are nanoseconds of the clip's time, like the module's other times, where they were seconds: they keep their parameter ids, so a caller still sending seconds as `f32` is refused, naming the parameter, rather than misread (an integer it sends is read as nanoseconds). The window reads back as requested, clamped into the clip.
