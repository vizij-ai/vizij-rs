---
"@vizij/runtime": minor
---

The `vizij-face` profile declares the face's lipsync gain, `standard/vizij/lipsync/gain`, an input resting at 1 that the face's adaptation scales every viseme weight by. The ROS4HRI mapping relays the face's current viseme to `standard/ros4hri/speech/viseme`, which the `ros4hri` profile declares as an output.
