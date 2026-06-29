# Phase 5 — Bridge convergence (`WsBridge: arora_bridge::Bridge`)

> Status: **prepared, not yet implemented.** Companion to
> [`proposal-vizij-on-arora.md`](./proposal-vizij-on-arora.md) §2.2 and §5.
> Phases 1–4 are in review (vizij-rs #19/#20/#21/#22/#23, arora-sdk #105).

## Goal

Vizij's `arora-websocket` (in `vizij-web`) is a **parallel reimplementation of
`arora-bridge`**: it ships a bespoke `AroraConnection` trait
(`arora-connection/src/traits.rs`) and an exclusive-single-client WebSocket
server that already calls itself *"the Arora protocol"*. Phase 5 retires that
parallel: Vizij's WS server becomes an **Arora `Bridge`**.

- A `WsBridge` that `impl arora_bridge::Bridge`, wrapping the existing
  `tokio-tungstenite` server.
- The runtime (VIZ-33 `arora::Runtime`) then drives it exactly like
  studio-bridge's Zenoh bridge — `commands()` in, `send_data()` out — with no
  Vizij-specific runtime code.
- The bespoke `AroraConnection` trait is deleted. **One protocol, one place.**

## The protocol today (`arora-websocket/src/messages.rs`)

Client → server (`Incoming`) and server → client (`Outgoing`), tagged JSON:

| `Incoming` | fields |
| --- | --- |
| `SetSlotValues` | `values: HashMap<String, Value>` |
| `GetSlotValues` | `slots: Vec<String>` |
| `ListSlots` | `path: Option<String>` |
| `ListMethods` | `path: Option<String>` |
| `Invoke` | `method: String, args: HashMap<String, Value>, request_id: Option<String>` |

`Outgoing`: `SetSlotValuesResp`, `GetSlotValuesResp { values }`,
`ListSlotsResp { slots: Vec<SlotInfo> }`, `ListMethodsResp { methods: Vec<MethodInfo> }`,
`InvokeResp { success, request_id, value, message }`, `Error { request_id, message }`.

The wire `Value` comes from **`arora-schema`** (a dependency pulled from a
*separate* repo, `git semio-ai/arora-types.git`) — see Decision B.

## Mapping onto `arora_bridge::Bridge`

Trait surface (`arora-bridge/src/lib.rs:106`):

| `Bridge` member | `WsBridge` behaviour |
| --- | --- |
| `commands() -> CommandStream` | each `Incoming` becomes a `BridgeCommand { op, reply }`; the WS read loop feeds this stream, and the `reply` oneshot is serialized back as the matching `Outgoing`. |
| `send_data(StateChange)` | broadcast as `Outgoing::SetSlotValues`-equivalent (`set_slot_values`) to subscribed clients. |
| `data_requested() -> DataRequestedStream` | toggles `true` when a client connects/subscribes, `false` on disconnect — gates whether the runtime bothers flushing. |
| `get_device_info` / `device_info_updated` / `update_device_info` | Vizij has **no device-registration** concept → stub like `FakeBridge` (`Ok(None)` / empty / echo). Revisit only if Vizij grows a device identity. |

`Incoming` → `BridgeOp` (the command `op`):

| `Incoming` | `BridgeOp` |
| --- | --- |
| `SetSlotValues { values }` | `Update(StateChange)` — each `(path, Value)` → `(Key, arora_types::Value)` |
| `GetSlotValues { slots }` | `Get(Vec<Key>)` → reply → `GetSlotValuesResp { values }` |
| `Invoke { method, args, request_id }` | `Call(Call)` → reply → `InvokeResp` |
| `ListSlots` / `ListMethods` | **no `BridgeOp` yet** → Decision A |

## Two arora-side prerequisites (decisions)

### Decision A — grow Arora's `Bridge` with introspection

`ListSlots` / `ListMethods` have no Arora equivalent. They are not noise — they
are exactly the **live-edit surface** (proposal §5): enumerate what paths/methods
exist and their declared types, for an editor UI. The convergence should *raise
Arora to Vizij's level*, not drop these.

Proposed: add to Arora (arora-sdk) a `list_slots(prefix) -> Vec<SlotInfo>` and
`list_methods(prefix) -> Vec<MethodInfo>` — most naturally as new `BridgeOp`
variants (`ListSlots`/`ListMethods`) answered by the runtime from its
`DataStore` + module registry, mirrored on `arora-web`'s JS surface.
This is a **separate arora-sdk PR**, a prerequisite for the WS mapping above to
be complete (until then `WsBridge` answers them with an `Error`/empty list).

### Decision B — one `Value` on the wire

`arora-websocket` serializes `arora-schema::Value` (from `arora-types.git`),
which is **not** arora-sdk's `arora_types::Value`. The store/Behavior/HAL work
(VIZ-35/37/41) is all on arora-sdk's `arora_types`. Two ways to converge:

- **B1 (recommended): migrate `arora-websocket` to arora-sdk's `arora-types`.**
  Drop the `arora-schema` dep; the wire `Value` becomes `arora_types::Value`
  (`vizij-arora` already converts Vizij ↔ that). One `Value` end to end — wire,
  store, behaviors. Cost: the JSON shape of `Value` on the wire changes, so the
  TS `@vizij/arora-types` client must track it.
- **B2: convert at the boundary.** Keep `arora-schema` on the wire; `WsBridge`
  translates `arora-schema::Value` ↔ `arora_types::Value`. Cheaper now, but
  perpetuates two `Value`s and a translation table — the exact duplication
  Phase 1 set out to kill.

`arora-types.git` is almost certainly the pre-consolidation Arora schema; B1
folds it into the consolidated `arora-sdk` line and is consistent with Phases
1–4. **Recommend B1**, sequenced after Decision A.

## Crate plan

Add the bridge impl **inside `arora-websocket`** (it already owns the server),
behind the existing `server` feature or a new `bridge` feature:

```
arora-websocket
├── src/messages.rs      # Incoming/Outgoing (unchanged shape; Value per Decision B)
├── src/server.rs        # tokio-tungstenite loop (kept; now feeds WsBridge)
└── src/bridge.rs   (new)# WsBridge: arora_bridge::Bridge
```

New deps: `arora-bridge` + `arora-types` (arora-sdk, git-pinned like the other
interop crates until published), `async-trait`, `futures`. The `AroraConnection`
trait + `arora-connection` crate are removed once `WsBridge` covers their use.

`Send`: `arora_bridge::Bridge` is `Send + Sync` and its streams are `Send` — fine
for the native tokio server. (The `arora-web`/wasm in-process bridge is a
separate, `!Send` path and is Phase 6, not this.)

## Implementation steps

1. **(arora-sdk)** Decision A: `BridgeOp::{ListSlots, ListMethods}` + runtime
   answers + `SlotInfo`/`MethodInfo` types. PR on arora-sdk.
2. **(vizij-web)** Decision B1: migrate `arora-websocket` off `arora-schema` onto
   arora-sdk `arora-types`; update `@vizij/arora-types` TS shapes.
3. **(vizij-web)** `WsBridge: arora_bridge::Bridge` in `arora-websocket/src/bridge.rs`,
   wrapping the server: read loop → `commands()`; `send_data` → broadcast;
   `data_requested` from client presence; device-info stubbed.
4. Delete the bespoke `AroraConnection` trait; point the server at `WsBridge`.
5. Test: a fake client `SetSlotValues` reaches the store via a queued runtime,
   and a runtime `send_data` arrives at the client as `set_slot_values`.

## Open decisions for review

1. **A** — extend Arora's `Bridge`/runtime with `ListSlots`/`ListMethods`
   introspection now (recommended; it is the live-edit surface), or stub them in
   `WsBridge` and defer?
2. **B** — **B1** migrate `arora-websocket` to arora-sdk `arora-types` (recommended,
   one `Value`), or **B2** translate at the boundary?
3. Confirm `WsBridge` lives **inside `arora-websocket`** (vs. a new
   `vizij-arora-bridge` crate), and that retiring `AroraConnection` is in scope.
