---
"@vizij/runtime": minor
---

`mount(canvas, { msaa })` picks the view's multisample anti-aliasing: `4` samples per pixel (the default) or `1`, which draws the faces' edges aliased and spares the GPU the multisampled target and its resolve every frame.
