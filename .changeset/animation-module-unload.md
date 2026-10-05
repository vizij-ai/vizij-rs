---
"@vizij/animation-module": minor
---

The module (0.3.0) unloads through declared functions, and any client finds a player by name. `remove_player(player) -> bool` removes a player and its instances; `unload_animation(anim) -> bool` unloads an animation and every instance of it; both apply immediately, like `add_instance`. `add_instance` takes an optional `weight` (1 when left out): an instance added at 0 writes nothing until `set_weight` gives it one, so a client loads an animation silent within one step. `PlayerState` (record 1.1.0) adds `name`, the name `create_player` gave the player, `instances`, each an `InstanceState { instance, anim, weight }`, and `loop_mode` (`once`, `loop` or `ping_pong`); a reader of the 1.0.0 record ignores the three fields.
