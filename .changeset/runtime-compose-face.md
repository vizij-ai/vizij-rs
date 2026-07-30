---
"@vizij/runtime": minor
---

Add `composeFace(gltf, options?)`: the composed behavior graph of a face bundle — base graphs, embedded standard profiles (each suppressing the built-in of the same id), the built-in ROS4HRI profile unless opted out, and the selected program — exactly as the native `vizij` app deploys it. The returned spec feeds `startRuntime`/`Runtime.loadGraph`, so an exported GLB can be deployed and verified in JS without the native app (VIZ-93's autonomous verification loop).
