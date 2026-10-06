---
"@vizij/animation": patch
---

`bake_animation` and `bake_animation_with_derivatives` throw for a bake of more than 2²⁰ samples over all tracks, derivative samples counted, instead of allocating it; the error names the sample count. An animation not loaded throws "no animation is loaded under id N". A `start_time` past the clip's end bakes the clip's end. `bake_animation` samples the values alone.
