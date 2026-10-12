---
"@vizij/runtime": minor
---

The `vizij-face` profile declares the caption a host shows over the face, `standard/vizij/caption/text`, and who says it, `standard/vizij/caption/speaker` (`__system__` for the face itself): string inputs resting empty, in no tier. The profile has 96 keys. The `ros4hri` profile declares `standard/ros4hri/caption/{text,speaker}`, which a ROS bridge writes from `hri_actions_msgs/ClosedCaption`, and the ROS4HRI mapping relays them as is to the face's caption.
