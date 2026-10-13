---
"@vizij/runtime": minor
---

The device registers the face skills `set_expression(expression, valence, arousal)`, `set_facial_action_units(codes, intensities)`, `set_viseme(viseme)` and `play_visemes(visemes, starts, durations)`, described by its interpreter and called by name like `look_at` and `play_viseme`. `skills()` lists seven. The two expression setters hold the face's whole expression and take it over from each other, crossfading; `set_viseme` and `play_visemes` take the lips over like `play_viseme`, and a lips command shows over a speaking `say` without halting it. With `ros4hri: true`, the mapping's expression keys run `set_expression` (the face boots at neutral from it), and the face standard's expression, muscle-tier and jaw keys are not in `listKeys` as inputs: a setter run holds them, so the expression is set through the setters or the ROS4HRI keys.
