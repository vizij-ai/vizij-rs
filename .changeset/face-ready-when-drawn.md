---
"@vizij/runtime": patch
---

`ready` and `whenReady` report a Vizij once the GPU has finished a frame drawing every mesh visible when it was drawn, each with its pipeline compiled, rather than once its scene is indexed. Until then the Vizij draws under the canvas's clear, so its rectangle shows the previous scene or the background, never a face with meshes missing; a reload's previous scene draws until the new one is ready. Under a software GPU, where a face's first draw compiles its pipelines for seconds, a page no longer stalls after `ready`: it draws on from there. Should something keep a Vizij from being drawn — a material that never loads, a mesh whose pipeline cannot be specialized — the console says what after 120 frames, and the Vizij is ready as it draws after 600 (ten seconds at 60 Hz).
