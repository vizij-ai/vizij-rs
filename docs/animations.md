# Playing a face's animations

A face GLB's `VIZIJ_bundle` carries the animations authored for it. On every
target they live in the device's **animation module** (Arora's
`arora-animation-module`, host-linked by
[`modules::animation`](../crates/vizij/src/modules/animation.rs)), and every
client — in-process Rust, a bridge client, a page, the face's own graphs —
plays them through that module's declared functions. There is no second,
Vizij-side transport: a function the module already declares needs no wrapper
here.

## Play one

Run the face, then play an animation by the name it was authored under:

```bash
cargo run -p vizij -- --glb face.glb      # the bridge listens on 127.0.0.1:9000
python play.py wave
```

`play.py`, over the local bridge (`pip install websockets`):

```python
"""Play one of a face's animations: python play.py <animation> [port]"""

import asyncio
import json
import sys

import websockets

PLAYERS = "vizij/animations/players"
# PlayerState's player, name and instances; InstanceState's instance.
PLAYER = "76697a69-6a00-0000-0111-000000000001"
NAME = "76697a69-6a00-0000-0111-000000000006"
INSTANCES = "76697a69-6a00-0000-0111-000000000007"
INSTANCE = "76697a69-6a00-0000-0112-000000000001"


async def request(socket, frame):
    """Send a frame, skip the live pushes, return the answer."""
    await socket.send(json.dumps(frame))
    while True:
        answer = json.loads(await socket.recv())
        if answer["type"].endswith("_resp"):
            return answer


def by_id(structure):
    return {field["id"]: field["value"] for field in structure["fields"]}


async def play(animation, port=9000):
    async with websockets.connect(f"ws://127.0.0.1:{port}") as socket:
        answer = await request(socket, {"type": "read_values", "keys": [PLAYERS]})
        players = [by_id(p) for p in answer["values"][PLAYERS]["structs"]["elements"]]
        state = next((p for p in players if p[NAME]["str"] == animation), None)
        if state is None:
            names = [p[NAME]["str"] for p in players]
            sys.exit(f"no animation named {animation}; this face has {names}")
        player = state[PLAYER]
        instance = by_id(state[INSTANCES]["structs"]["elements"][0])[INSTANCE]
        # Weigh its instance in, then start its player.
        await request(
            socket,
            {
                "type": "invoke",
                "method": "set_weight",
                "args": {"player": player, "instance": instance, "weight": {"f32": 1.0}},
            },
        )
        await request(
            socket, {"type": "invoke", "method": "play", "args": {"player": player}}
        )
        print(f"playing {animation}")


asyncio.run(play(*sys.argv[1:]))
```

Asked for a name the face does not carry, it prints the names it does — the
ids the animations were authored under, which is what `name` holds. To stop
one, `invoke` `stop` with the same `player`; to make it let go of the keys it
writes, `set_weight` the instance back to `0`.

The rest of this page is why those two calls are the ones, and what else is
there.

## What loading leaves you

[`load_animations`](../crates/vizij/src/face.rs) runs when the device is
built. Per animation in the bundle, in order:

| Call | What it leaves |
|---|---|
| `load_clip` | the animation as the module's `Clip` (format 2.0), each track keyed by the store key its channel names through the face's rig ([`module_animation`](../crates/vizij/src/modules/animation.rs)) |
| `create_player` | a player **named after the animation's id** |
| `add_instance_with_weight` at `0` | one instance of the animation on it, silent |
| `stop` | the playhead at the animation's start |

So a loaded animation writes nothing and holds no key: the rig, the program
and the skills own the pose until a client asks for the animation. Playing one
is two calls — give its instance a weight, then start its player:

```text
set_weight(player, instance, 1.0)
play(player)
```

`player` and `instance` are the ids the module hands out, not names. A client
finds them in the `[PlayerState]` the composed animation source writes to
**`vizij/animations/players`** every tick
([`ANIMATION_PLAYERS_PATH`](../crates/interop/vizij-arora-host/src/lib.rs)) —
a plain store key, so it travels over every bridge — or by calling
`player_states()` directly. Match on the player's `name`.

`play_at(player, time_ns)` starts a player at a device time instead of at
once: the animation source steps the module with `arora/time`, so several
faces, or several animations, start on one anchor.

## The surfaces

**In-process (Rust).** Dispatch an Arora `Call` naming the module and the
function, arguments by parameter id. The ids are the module's declaration,
`arora_animation_module::animation::ids`:

```rust
arora.call(Call {
    module_id: Some(ids::MODULE),
    id: ids::play::FUNCTION,
    args: vec![StructureField { id: ids::play::PLAYER, value: Box::new(Value::U32(player)) }],
})?;
```

Arguments are matched by id, so the order they are sent in carries no meaning.

**The local WebSocket bridge.** Every `vizij` run serves it (`ws://127.0.0.1:9000`
by default; see [the app README](../crates/vizij/README.md#the-local-bridge)).
The bridge relays what the device's modules describe, so the animation
functions are listed by `list_methods` beside `reset` and the skills, and
`invoke` calls them by name:

```json
{"type": "invoke", "method": "play", "args": {"player": {"u32": 3}}, "request_id": "a1"}
```

Subscribe to `vizij/animations/players` on the same socket to learn the ids.

**A page — [`@vizij/runtime`](../npm/@vizij/runtime/README.md#animations).**
`invoke` by name, or `call` by id, and `decodePlayerStates` for the states:

```ts
const players = decodePlayerStates(quori.readValues([ANIMATION_PLAYERS_PATH])[ANIMATION_PLAYERS_PATH]);
const { player, instances } = players.find((p) => p.name === "wave")!;
await quori.invoke("set_weight", { player: { u32: player }, instance: { u32: instances[0].instance }, weight: { f32: 1 } });
await quori.invoke("play", { player: { u32: player } });
```

`describe(glb).animations` lists what a GLB carries before it loads, and
`loadVizij(id, glb, { animations: false })` keeps the bundle's animations out
of the module.

**The face's own graphs.** A node reaches a module through an
`externalfunction` node carrying the function's and module's ids
([node-graph.md](../crates/interop/vizij-arora-behavior/docs/node-graph.md#how-nodes-call-modules-and-animation)),
so a program or a skill fragment plays an animation exactly as a client does.
This is the route for an animation that starts from the face's own behavior —
and the route to one key a robot can write.

## What does not play one

- **ROS 2** binds `/skill/look_at` and `/skill/say` as actions and publishes
  the device's keys as topics; it binds no animation function. A ROS robot
  plays an animation by writing a key the face's program watches, or over the
  local bridge.
- **Semio Studio** addresses a face's *animatables* — its pose targets — not
  its animations.
- **No flag** of the `vizij` binary plays an animation at startup. The bundle's
  authored program is what runs from load
  ([`spawn_program`](../crates/vizij/src/face.rs)).

## Transport, past `play`

Every function the module declares is reachable from each surface above.

| Concern | Functions |
|---|---|
| Transport | `play`, `play_at`, `pause`, `stop`, `seek` |
| The player's timeline | `set_speed`, `set_loop` (`once`, `loop`, `bounce`, `ping_pong`), `set_window`, `set_length`, `set_reversed` |
| An instance on it | `set_weight`, `set_start_offset`, `set_rate`, `set_time_scale` (`rate = 1 / ts`), `add_instance`, `add_instance_with_weight`, `remove_instance` |
| What the device holds | `load_clip`, `reload_clip` (in place: the playback stands), `unload_animation`, `create_player`, `remove_player` |
| Reading it back | `player_states`, `output_keys`, `bake`, `bake_with_derivatives` |

`step`, `step_values` and `step_with_velocities` are the module's tick, which
the animation source calls with the device's clock; a client steps the module
only when it owns the clock.

Two consequences of a module never writing keys:

- A **stopped** player still writes its first frame at its weight. An
  animation lets go of its keys at **weight 0**, not on `stop`.
- An animation's keys keep their last values when it stops writing — after
  `unload_animation`, or at weight 0. Returning them to rest is the client's
  step: `reset`, or `reset_keys` with the keys to return.

## See also

- [the `vizij` app](../crates/vizij/README.md) — the device, its bridges and
  its flags
- [`@vizij/runtime`](../npm/@vizij/runtime/README.md) — the browser device
- [Skills](skills.md) — the other way a face's behavior is data
- [`arora-animation-module`](https://crates.io/crates/arora-animation-module) —
  the module's own reference
