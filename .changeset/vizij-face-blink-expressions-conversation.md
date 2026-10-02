---
"@vizij/runtime": minor
---

`profile("vizij-face")` declares 88 keys: the blink `standard/vizij/blink` (tier `gaze`; 0 open, 1 both eyes closed — a level the writer shapes over time, independent of the eyelid positions), the expressions `standard/vizij/expression/concerned` and `…/sleepy` (tier `expression`), and the conversation state `standard/vizij/conversation/{speaking,user_speaking,thinking}` (tier `conversation`, written 0 or 1 by the agent the face speaks for). Each is an `f32` weight in [0, 1] resting at 0. `mapping("ros4hri")` is unchanged: ROS4HRI names none of them.
