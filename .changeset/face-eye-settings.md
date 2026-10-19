---
"@vizij/runtime": minor
---

The `vizij-face` profile declares two more settings of the face, `standard/vizij/gaze/eye_spacing` (metres, resting at 0.06) and `standard/vizij/gaze/range` (radians, resting at 0.78), and has 98 keys. The ROS4HRI mapping aims the eyes with them, read under the face's rig prefix, where it held the incumbent's 6 cm and ±0.78 rad as constants; a face authors its own in its adaptation, as the default of an input node on the setting's path, and a face authoring neither is aimed as before. A face's settings rest at the value the face authors, so `reset` returns them to the face's own.
