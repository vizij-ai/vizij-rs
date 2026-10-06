---
"@vizij/runtime": minor
---

The ROS4HRI mapping shows a named expression (`standard/ros4hri/expression/name`) at the commanded arousal (`standard/ros4hri/expression/arousal`) as its intensity, clamped to 0..1, instead of at full weight: a name with an arousal of 0 or below shows no expression. An empty name still blends the expressions by valence and arousal on the circumplex.
