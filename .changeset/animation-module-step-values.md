---
"@vizij/animation-module": minor
---

The module (2.0.0) steps returning values by position, with the keys sent only when they change. `output_keys() -> OutputKeys { revision, keys }` gives the key of each position: per player, in creation order, each key its instances write, in the order they first write it. Only a structural edit changes it (`add_instance`, `add_instance_with_weight`, `remove_instance`, `remove_player`, `unload_animation`, `reload_animation`), each that does giving a new `revision`. `step_values(dt_ns, time_ns?) -> StepValues { revision, values }` is `step` returning the values alone, a `Value::ArrayValue` as long as the table, `Value::Unit` at a position no instance weighs on this step; a `revision` other than the one the keys were read at says to read them again. `step`'s outputs come in the same order, where they came in an order that varied from step to step.
