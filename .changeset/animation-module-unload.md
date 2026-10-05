---
"@vizij/animation-module": minor
---

The module (0.3.0) adds an instance at a given weight and unloads through declared functions, and any client finds a player by name. `remove_player(player) -> bool` removes a player and its instances; `unload_animation(anim) -> bool` unloads an animation and every instance of it; both apply immediately, like `add_instance`. `add_instance_with_weight(player, anim, weight) -> u32` adds an instance blending at `weight`: added at 0, it writes nothing until `set_weight` gives it a weight, so a client loads an animation silent although it learns the instance's id only from the reply. `add_instance(player, anim)` is unchanged and adds an instance at weight 1. `PlayerState` (record 1.1.0) adds `name`, the name `create_player` gave the player, `instances`, each an `InstanceState { instance, anim, weight }`, and `loop_mode` (`once`, `loop` or `ping_pong`); a reader of the 1.0.0 record ignores the three fields.
