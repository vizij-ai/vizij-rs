---
"@vizij/animation-module": patch
---

The module (1.1.2) refuses a bake it cannot hold with a value. `bake` and `bake_with_derivatives` return an empty string, as for an animation not loaded, when the window at the frame rate would take more than 2²⁰ samples over all tracks (derivative samples included); such a bake used to trap the guest. A `start_time` past the clip's end bakes the clip's end. `bake` samples the values alone, without computing derivatives it does not return.
