# Proposal: running Vizij on Arora (HAL, Behavior, Bridge, DataStore)

Status: draft, for review.
Date: 2026-06-29.
Author: Victor (with research help from an LLM agent).

This document proposes how the **Vizij** real-time animation runtime
(`vizij-rs` + `vizij-web`) can run *on top of Arora* instead of
reimplementing Arora's concepts, and what Arora must generalize to host
it. It is the concrete continuation of the `arora-web` thread opened in
arora-engine `docs/proposal-split-arora-repos.md` §2.4: there we named `arora-web` as the
`wasm-bindgen` surface that "the Vizij Workspace could depend on
directly". This proposal works out what that dependency actually looks
like, end to end.

The thesis is simple: **Vizij and Arora are the same shape.** Vizij has
independently re-grown three of Arora's four pillars (a device/model, a
remote-control protocol, an in-memory store) and added a fourth kind of
behavior content (a data-flow node graph instead of a behavior tree).
The integration is therefore mostly *recognition and convergence*, plus
**one genuine generalization in Arora**: turning behavior from a concrete
struct into a trait so a node graph can be a first-class behavior.

A useful side effect: the APIs we expose for Vizij's *live editing* (live
data writes, live graph patch/reload) are not Vizij-specific. They are
the archetypal surface any web app would use to drive an Arora at
runtime. We design them as such.

## Discussions and Agreements

- **Victor (Value strategy, 2026-06-29):** Vizij's `Shape`/`ShapeId`
  resemble Arora's value-type system — "specific structured types". They
  may stay as concrete Vizij Rust types **as long as they convert to and
  from Arora `Value`**. We should *exercise `arora-module-authoring`* to
  declare those types and generate the bindings, rather than hand-write a
  parallel conversion layer. The free-form `meta` part of `Shape`
  (`unit`/`space`/`range`/`color_space`) is metadata, not type — it
  belongs in a **distinct sidecar key**, not inside `Value`.
- _Open for review:_ disposition of Vizij's orchestrator merge semantics
  (§2.5), and the live-edit op vocabulary on the Bridge (§5).

---

## 1. Context and goal

### 1.1 What Vizij is, in Arora terms

Vizij drives a GLB rig (bones, morph targets) from behavior content (a
node graph, plus animation clips), and can be controlled remotely over a
WebSocket. Stripped of vocabulary, that is an Arora: a thing being
actuated, behavior that computes actuation, a remote that observes and
commands, and an in-memory store everyone reads and writes.

The mapping is nearly one-to-one:

| Vizij today | Arora concept | Fit |
| --- | --- | --- |
| GLB rig + Three.js renderer (actuated; outputs applied to bones/morphs) | **HAL** (`Hal` + `HalAssets::model_glb`) | clean — the renderer *is* a HAL |
| `arora-connection` / `arora-websocket` (already "the Arora protocol") | **Bridge** (`Bridge`, `BridgeOp::{Get,Update,Call}`) | clean — Vizij's is *richer* (has introspection) |
| Blackboard (`HashMap<TypedPath, BlackboardEntry>`, conflict logs, subscribe) | **DataStore** (`read`/`write`/`snapshot`/`slot`/`subscribe`) | clean — Vizij's is a *better impl* |
| Node graph (`GraphSpec` + `GraphRuntime`, `evaluate_all`) | **BehaviorTree** | **the one real seam** — needs a trait |
| Orchestrator (multi-controller, pass schedule, merge) | _(nothing — Arora has no orchestrator)_ | mostly dissolves into `step()`; merge needs a home |

### 1.2 What we want

1. Vizij reuses Arora wherever Arora already covers the concern (store,
   bridge, runtime loop, type/codegen tooling), deleting Vizij's
   parallel implementations.
2. Arora gains the minimum generality to host (a) a node-graph behavior
   and (b) Vizij's richer store, **without** absorbing graphics-specific
   concerns into its core types.
3. The result runs in two places that matter: the Tauri standalone
   (native Rust) and the browser (`arora-web`, the `wasm-bindgen`
   surface). Both drive the same `step()`.
4. The live-edit APIs that fall out are designed as Arora's general
   runtime-control surface, not a Vizij one-off.

### 1.3 Non-goals

- Not merging the two `Value` enums into one. Vizij keeps its graphics
  value types as concrete Rust types (§3).
- Not pushing graphics semantics (units, color spaces, coordinate
  frames) into Arora's type system. That metadata stays in a sidecar
  (§3.3).
- Not preserving Vizij's `vizij-orchestrator-core` as a long-term crate.
  Its scheduling dissolves; only its *merge semantics* are load-bearing
  and need a deliberate home (§2.5).

---

## 2. Concept mapping (Vizij ↔ Arora)

### 2.1 HAL ↔ renderer / model

Arora's `Hal` (`arora-hal/src/lib.rs:61`) is async and interior-mutable:

```rust
pub trait Hal: Send + Sync {
    async fn describe(&self) -> HalDescription;
    async fn read(&self, keys: &[Key]) -> HalResult<Vec<Option<Value>>>;
    async fn read_all(&self) -> HalResult<State>;
    async fn write(&self, changes: StateChange) -> HalResult<()>;
    fn updates(&self) -> Subscription;
}
pub trait HalAssets: Send + Sync {
    async fn model_glb(&self) -> HalResult<Option<Vec<u8>>>;
}
```

In Arora the HAL is the robot; in Vizij the "robot" is the GLB rig being
driven. So **Vizij's renderer is a HAL**:

- `model_glb()` → the loaded GLB (Arora already models the world as a
  GLB — `HalAssets` exists for exactly this).
- `write(StateChange)` → apply outputs to the rig (set bone transforms /
  morph weights). This is the actuation path; in Arora the io-pump calls
  `hal.write` with the store changes a behavior produced.
- `read` / `read_all` → current rig state (mostly write-only in practice;
  read returns last-applied).
- `updates()` → typically empty for a pure renderer; non-empty if the
  surface feeds anything back (e.g. picking, measured pose).

The current web data path (`frame.merged_writes` → `store.setValues()` →
renderer, in `@vizij/runtime-react`) *is* this flow, just spelled
differently: behavior writes the store, the store change is applied to
the renderer-as-HAL.

### 2.2 Bridge ↔ WebSocket protocol

Arora's `Bridge` (`arora-bridge/src/lib.rs:106`) carries device-info,
data-requested, and command streams, with operations:

```rust
pub enum BridgeOp {
    Get(Vec<Key>),
    Update(StateChange),
    Call(Call),
}
```

Vizij's `arora-websocket` messages map straight onto these:

| Vizij `Incoming` | Arora `BridgeOp` |
| --- | --- |
| `SetSlotValues { values }` | `Update(StateChange)` |
| `GetSlotValues { slots }` | `Get(Vec<Key>)` |
| `Invoke { method, args }` | `Call(Call)` |
| `ListSlots { path }` | *(none — Arora's Bridge lacks introspection)* |
| `ListMethods { path }` | *(none)* |

Two findings:

1. Vizij already calls this "the Arora protocol" and even ships a
   bespoke `AroraConnection` trait + an exclusive-single-client WS
   server. This is a *parallel reimplementation of `arora-bridge`*. It
   should become a `WsBridge: arora_bridge::Bridge`, and the bespoke
   `AroraConnection` trait should be retired. One protocol, one place.
2. Vizij's protocol is **ahead** of Arora's: it has `ListSlots` /
   `ListMethods` — *introspection*. That is not noise; it is precisely
   the live-edit surface (§5). Arora's Bridge should grow it.

### 2.3 DataStore ↔ Blackboard

Arora's store is already a trait (`arora-types/src/data/store.rs:94`):

```rust
pub trait DataStore: Send + Sync {
    fn read(&self, keys: &[Key]) -> Vec<Option<Value>>;
    fn write(&self, changes: StateChange) -> Result<(), DataError>;
    fn snapshot(&self) -> State;
    fn slot(&self, key: &Key) -> Box<dyn Slot>;
    fn subscribe(&self) -> Subscription;
}
```

`SimpleDataStore` is just the reference impl. Vizij's Blackboard
(`HashMap<TypedPath, BlackboardEntry>`, with `BlackboardEntry { value,
shape, epoch, source, priority }` and `apply_writebatch → Vec<ConflictLog>`)
is a *richer* `DataStore`: it is provenance-tracked (epoch + writer
source) and records conflict logs. It becomes an Arora `DataStore` impl,
and the conflict-log/provenance machinery is a capability Arora gains for
free.

The only blocker is that `Runtime` hard-codes the field type
`store: SimpleDataStore` (`arora/src/runtime.rs:104`). That becomes
`Arc<dyn DataStore>` (or a generic) — a small, contained change (§4.4).

Note the addressing match: Arora `Key { path: String }`
("namespace/entity.attribute") and Vizij `TypedPath`
("namespace/.../target.field") are the same path grammar with different
parsers. They converge on one path type.

### 2.4 Behavior ↔ node graph — the one real seam

This is where Arora must change. Arora's runtime hard-codes
`trees: VecDeque<BehaviorTree>` and ticks them via `tick_tree()`; there
is **no `Behavior` trait**. `BehaviorTree` is a concrete struct
(`arora-behavior-tree/src/behavior_tree.rs:31`) ticked through
`CallBridge` into modules.

Vizij's node graph is a *different kind of behavior*: `GraphSpec` +
`GraphRuntime`, driven once per frame by `evaluate_all(rt, spec)` with
`rt.t`/`rt.dt` set by the caller, inputs staged by `TypedPath`, outputs
read from `outputs: HashMap<NodeId, HashMap<String, PortValue>>`.

To host it, Arora generalizes behavior to a trait that both the tree and
the graph implement. Full design in §4. This is the central — and
essentially the *only* structural — change required in Arora.

### 2.5 The orchestrator dissolves (but its merge does not)

Most of `vizij-orchestrator-core` is replaced by Arora's serial `step()`
loop. But one responsibility has **no Arora equivalent and is genuinely
valuable**: when multiple controllers (graphs + animations) write the
same path in a frame, the orchestrator merges their `WriteBatch`es with a
*conflict strategy* (`error` / `namespace` / `blend` / `add`) under a
chosen *schedule* (`SinglePass` / `TwoPass`). Arora ticks **one** behavior
per step; it has nothing that blends N write batches.

Disposition (recommended, migration-friendly): **wrap the whole
orchestrator as a single Arora `Behavior` first.** It already has
`step(dt) -> OrchestratorFrame`; satisfying the trait is immediate and
preserves all merge semantics on day one. Then, incrementally:

- single-graph cases drop the orchestrator and tick a bare
  `ProcessingGraph`;
- the multi-writer *merge* becomes either a `Behavior` combinator
  (a behavior that composes child behaviors and blends their writes) or,
  later, a first-class Arora runtime feature.

What we must *not* do is lose the merge strategies in the shuffle. They
are the one piece of orchestrator IP worth carrying forward.

---

## 3. Value strategy (the linchpin)

### 3.1 What Vizij's `Shape` actually is

`Shape` (`vizij-api-core/src/shape.rs:77`) is **two things bundled**:

```rust
pub struct Shape {
    pub id: ShapeId,                       // structural TYPE
    pub meta: HashMap<String, String>,     // free-form METADATA
}
```

- **`ShapeId`** is the *structural type*: `Scalar`, `Bool`, `Vec2/3/4`,
  `Quat`, `ColorRgba`, `Transform`, `Text`, `Vector{len}`,
  `Record(fields)`, `Array(inner,len)`, `List(inner)`, `Tuple`, `Enum`.
  It is **functional**: `shape_helpers.rs` uses it to validate node
  outputs (`value_matches_shape`, `enforce_output_shapes`) and to infer
  shapes from values. This is the part that resembles Arora's value-type
  system.
- **`meta`** is open-ended annotations (`unit`, `space`, `range`,
  `color_space`). It is **never read during evaluation** — purely carried
  for hosts/adapters.

### 3.2 Decision: declare Vizij's types via `arora-module-authoring`

Arora already has the type machinery and the codegen to absorb `ShapeId`
without a parallel system:

- Arora's `Value` carries `Structure(Structure)` / `Enumeration(...)`,
  each identified by a UUID; primitives have well-known UUIDs
  (`arora-types/src/ty/`).
- `arora-module-authoring` takes a declared type (YAML record) and
  *generates* the Rust bindings — including `impl Into<Value>` and
  `impl TryFrom<Value>` per type (`arora-module-authoring/rust/src/lib.rs`),
  plus the YAML structure/enumeration records that persist the schema in
  the registry.

So the strategy (per Victor's call) is:

> Keep Vizij's `Vec3` / `Quat` / `Transform` / `ColorRgba` / record types
> as **concrete Rust types** used inside the node graph. **Declare them as
> Arora types** via `arora-module-authoring`, which generates the
> `From`/`Into Value` bindings and the type records. `ShapeId` then maps
> onto Arora's type system rather than living as a second, parallel type
> language.

Concretely:

- Vizij's fixed-arity numeric types (`Vec3`, `Quat`, `Transform`,
  `ColorRgba`) become declared Arora **structure** types (e.g. `Transform`
  = `{ translation: [f32;3], rotation: [f32;4], scale: [f32;3] }`), each
  with a stable UUID and generated `Into/TryFrom<Value>`.
- Vizij's `Enum(tag, payload)` maps to Arora `Enumeration`; `Record` maps
  to `Structure`; `List`/`Array`/`Tuple` map to Arora's array value
  variants.
- `Value::Float/Bool/Text` map to Arora primitives directly.

This is "exercise `arora-module-authoring` to declare types and generate
bindings" — the tooling already does precisely this; Vizij becomes its
first cross-project consumer. A bonus: the same declarations can emit the
TypeScript/`wasm-bindgen` side, keeping the web types in lockstep.

### 3.3 Metadata (`meta`) as a sidecar

Arora has **no** metadata/annotation system — confirmed: neither `Value`,
`Structure`, `Enumeration`, nor the `Type` descriptor has any place for
units/ranges/spaces. So `Shape.meta` cannot and should not be folded into
`Value`. It becomes a **sidecar keyed by path**, e.g. a reserved metadata
namespace in the same `DataStore`:

```
/standard/semio/mouth/pos.x          -> Value (the live value)
/meta/standard/semio/mouth/pos.x     -> KeyValue { unit, space, range, color_space }
```

Properties of this choice:

- Metadata travels the same store, subscribe, and bridge machinery as
  data — no new transport.
- It is *optional* and *sparse*: only annotated paths carry a `/meta/`
  entry; evaluation never depends on it.
- A web editor reads `/meta/...` to render units/ranges; the runtime
  ignores it. This matches how Vizij already treats `meta` (host-only).

(Arora's `KeyValue` value variant is a natural carrier for the sidecar
payload. If a typed metadata record is preferred later, it too can be a
declared type.)

### 3.4 What this buys us

- One `Value` on the wire and in the store (Arora's), so Bridge, store,
  and runtime stay value-agnostic.
- Vizij code keeps ergonomic concrete types (`Vec3`, …) via generated
  conversions — no stringly-typed access.
- No graphics concepts leak into Arora's core. Units and spaces live in a
  sidecar; structural types live in the registry as ordinary declared
  types.

---

## 4. The Arora generalization: a `Behavior` trait

### 4.1 Today

`arora/src/runtime.rs` owns `trees: VecDeque<BehaviorTree>` and, each
step, pops one and calls `tick_tree()`. Behavior is a concrete type; the
runtime cannot tick anything that is not a `BehaviorTree`.

### 4.2 Proposed `Behavior` trait (sketch)

A behavior is "a thing the runtime ticks against the store each frame".
The minimal trait:

```rust
/// A unit of behavior the runtime ticks once per step.
pub trait Behavior: Send {
    /// Advance by `dt`, reading and writing `store`. Returns running status.
    fn tick(&mut self, store: &dyn DataStore, dt: f32) -> Result<Status, BehaviorError>;
}
```

- `BehaviorTree` implements it by wrapping its existing
  `BehaviorTreeRuntime::tick` (it already reads/writes the store and
  calls modules via `CallBridge`).
- The runtime holds `behaviors: VecDeque<Box<dyn Behavior>>` (or a single
  `Box<dyn Behavior>` for the common case) and ticks through the trait.

Live editing adds three optional methods (kept separate so simple
behaviors need not implement them; see §5):

```rust
pub trait LiveBehavior: Behavior {
    /// Apply a fine-grained edit; returns whether a structural reset occurred.
    fn patch(&mut self, edit: BehaviorEdit) -> Result<PatchOutcome, BehaviorError>;
    /// Replace the spec wholesale, migrating preserved runtime state where possible.
    fn load(&mut self, spec: BehaviorSpec) -> Result<(), BehaviorError>;
    /// Read current structure + live values for an editor (bidirectional sync).
    fn inspect(&self) -> BehaviorSnapshot;
}
```

### 4.3 `ProcessingGraph` (Vizij's node graph as a behavior)

A `ProcessingGraph` wraps `GraphSpec + GraphRuntime` and implements
`Behavior::tick` as the adapter between Arora's "read/write the store"
model and Vizij's "stage inputs / evaluate / read outputs" model:

```rust
impl Behavior for ProcessingGraph {
    fn tick(&mut self, store: &dyn DataStore, dt: f32) -> Result<Status, BehaviorError> {
        // 1. read this graph's subscribed input paths from the store
        for key in &self.input_keys {
            let v = store.read(std::slice::from_ref(key)).pop().flatten();
            if let Some(v) = v { self.rt.set_input(key.into(), v, None); }
        }
        // 2. evaluate one frame
        self.rt.dt = dt; self.rt.t += dt;
        evaluate_all(&mut self.rt, &self.spec)?;
        // 3. write outputs back as a StateChange
        let change = outputs_to_state_change(&self.rt, &self.spec);
        store.write(change).map_err(...)?;
        Ok(Status::Running)
    }
}
```

`LiveBehavior::patch` maps onto Vizij's existing `set_param(node, key,
value)` (param-only, keeps the plan cache + node states);
`LiveBehavior::load` maps onto `load_graph(json)`. The genuinely hard
part — preserving spring/damp/slew runtime state across a topology change
— is called out as risk R-3 (§7).

The orchestrator-as-`Behavior` from §2.5 is just another `Behavior` impl
whose `tick` runs the schedule and merge; it can compose child
`ProcessingGraph`s.

### 4.4 Runtime takes `Arc<dyn DataStore>`

To let Vizij's Blackboard back the runtime, the hard-coded
`store: SimpleDataStore` field becomes `store: Arc<dyn DataStore>` (or
`Runtime<S: DataStore>` if we prefer monomorphization). `with_io(...)`
gains a store parameter (defaulting to `SimpleDataStore` for existing
callers). The single-owner step-loop discipline is unchanged — only one
thread touches the store; the trait object does not introduce sharing.

---

## 5. Live-edit API archetype

The runtime-control surface that Vizij needs is the same surface any web
app would use to drive an Arora live. We design it generically and
surface it both over the Bridge (remote) and over `arora-web` (in-process
JS). There are **two planes**.

### 5.1 Data plane — live value updates (≈90% already present)

Writing/reading live values already exists end to end: `DataStore::write`
+ Bridge `Update`/`Get`. What is missing for an *editor* is introspection
and observation:

- `list_slots(prefix) -> [SlotInfo]` — enumerate what paths exist and
  their declared type (Vizij's `ListSlots`; Arora's Bridge lacks it).
- `subscribe(filter) -> stream<StateChange>` — observe changes so an
  editor reflects runtime state (Arora's store already has `subscribe`;
  the Bridge needs to expose a filtered feed).

In `arora-web` (JS):

```ts
runtime.write(path, value)        // or writeBatch([...])
runtime.read(paths)
runtime.listSlots(prefix)
runtime.subscribe(filter, cb)     // editor reflection
```

### 5.2 Behavior plane — live graph updates (the new archetype)

This is what Arora does *not* have today and what `LiveBehavior` (§4.2)
introduces. Three operations, at two granularities:

- **`patch(edit)`** — fine-grained: set a node param, add/remove a
  node/edge. Returns whether a structural reset was forced. Cheap path
  preserves runtime state.
- **`load(spec)`** — wholesale swap; the runtime decides patch-vs-reset
  and migrates preserved state (node-keyed buffers) where it can.
- **`inspect()`** — read current structure + live param values for the
  editor; enables bidirectional sync (runtime → editor), which Vizij
  lacks today.

In `arora-web` (JS):

```ts
runtime.patchBehavior(edit)       // { setParam: { node, key, value } } etc.
runtime.loadBehavior(spec)
runtime.inspectBehavior()         // structure + live values
runtime.listMethods(prefix)       // callable surface (Vizij's ListMethods)
```

### 5.3 Surfacing through the Bridge

The same two planes extend `BridgeOp` so a *remote* editor (Studio, a web
client) can drive live edits, not just in-process JS:

```rust
pub enum BridgeOp {
    Get(Vec<Key>),
    Update(StateChange),
    Call(Call),
    // proposed, for live edit + introspection:
    ListSlots(Option<Path>),
    ListMethods(Option<Path>),
    InspectBehavior(BehaviorRef),
    PatchBehavior(BehaviorRef, BehaviorEdit),
    LoadBehavior(BehaviorRef, BehaviorSpec),
}
```

These are additive; existing bridges ignore the new variants until they
implement them. Vizij's `WsBridge` implements them by delegating to the
runtime's `LiveBehavior`.

### 5.4 Who drives `step()`

Unchanged from Arora's model and Vizij's reality: on native, a thread
calls `step()` on its cadence with the async io-pump spawned alongside;
on web, `requestAnimationFrame` (or a worker) calls `step(dt)` — exactly
what `@vizij/runtime-react` does now. `arora-web` owns this loop and
exposes the data/behavior plane methods above to JS.

---

## 6. Phased migration plan

Ordered by risk-retired-per-step; each phase is independently valuable
and leaves both projects building.

**Phase 0 — Spike the seam (lowest risk, highest information).**
Implement the `Behavior` trait + a `ProcessingGraph` that runs *one* Vizij
graph against a `SimpleDataStore`, natively, with no orchestrator and no
bridge. Goal: prove the read-inputs / evaluate / write-outputs adapter
and surface unknowns (path mapping, value conversion gaps). No Arora
public API committed yet.

**Phase 1 — Value convergence via `arora-module-authoring`.**
Declare Vizij's structured types (`Vec3`/`Quat`/`Transform`/`ColorRgba`/
records) as Arora types; generate `Into/TryFrom<Value>` and the type
records. Land `outputs_to_state_change` / `state_change_to_inputs` on top
of the generated conversions. Decide and document the `/meta/` sidecar
convention (§3.3).

**Phase 2 — `Behavior` trait lands in Arora.**
Promote the spike: `BehaviorTree` implements `Behavior`; runtime ticks
`Box<dyn Behavior>`; `store` becomes `Arc<dyn DataStore>`. Existing Arora
behavior keeps working (regression-gated by the engine's current tests).

**Phase 3 — Blackboard as a `DataStore`.**
Implement `DataStore` for Vizij's Blackboard (provenance + conflict
logs). Run a Vizij graph against it through the Phase-2 runtime. Arora
gains conflict-log capability.

**Phase 4 — Orchestrator-as-`Behavior`, then decompose.**
Wrap `Orchestrator` as one `Behavior` to preserve merge semantics
immediately. Then split: single-graph paths tick a bare `ProcessingGraph`;
multi-writer merge becomes a `Behavior` combinator. Retire
`vizij-orchestrator-core` as a standalone crate when nothing depends on
it.

**Phase 5 — Bridge convergence.**
Reimplement `arora-websocket` as `WsBridge: arora_bridge::Bridge`; retire
the bespoke `AroraConnection` trait. Add the introspection ops
(`ListSlots`/`ListMethods`) to `arora-bridge`.

**Phase 6 — Live-edit plane + `arora-web`.**
Add `LiveBehavior` (`patch`/`load`/`inspect`) and the behavior-plane
`BridgeOp`s. Expose the data + behavior plane through `arora-web`'s
`wasm-bindgen` surface, driven by rAF/worker. This replaces
`vizij-orchestrator-wasm` and the bespoke WS client. Tackle R-3
(state-preserving reload) here.

At the end, `vizij-rs` is: a set of Arora-declared types, a
`ProcessingGraph`, a `BlackboardStore`, and a thin renderer-HAL — all
running on `arora` natively and `arora-web` in the browser.

---

## 7. Risks and open questions

- **R-1 — Value fidelity.** Vizij `Vec3`/`Quat`/`Transform` carry
  graphics semantics that Arora `Value` represents structurally. Generated
  conversions must round-trip losslessly. *Mitigation:* Phase 1 ships
  round-trip property tests per type before anything depends on them.
- **R-2 — Two path grammars.** `Key` and `TypedPath` are the same idea,
  different parsers. *Mitigation:* converge on one path type in Phase 1;
  keep a thin `From`/`Into` shim during migration.
- **R-3 — State-preserving graph reload (hard).** Live topology edits
  must migrate node-keyed runtime state (spring/damp/slew buffers).
  Param-only edits are easy (`set_param`); structural edits today reset.
  *Mitigation:* `patch` returns whether a reset occurred; full
  state-migration is a Phase-6 deliverable with its own design note.
- **R-4 — Merge semantics ownership.** If we decompose the orchestrator
  too eagerly we can lose `blend`/`add`/`namespace` conflict resolution.
  *Mitigation:* orchestrator-as-`Behavior` first (Phase 4); decompose
  only behind tests that assert merge outcomes.
- **R-5 — Async/sync seam on web.** Arora's `Hal`/`Bridge` are async with
  an io-pump; the graph evaluates synchronously in `step()`. This already
  matches Vizij's rAF loop, but the wasm single-thread + `spawn_local`
  pump must be re-validated for `arora-web` (cf. the existing
  `arora-web` wasm build notes).
- **Q-1 — Generic vs trait-object store/behavior?** `Arc<dyn DataStore>`
  and `Box<dyn Behavior>` keep the runtime simple; generics avoid dynamic
  dispatch in the hot loop. Recommend trait objects first; measure before
  optimizing.
- **Q-2 — Where does the merge combinator live** — `arora` core, a new
  `arora-compose` crate, or Vizij-side? Defer to Phase 4 evidence.

---

## 8. Summary

Vizij is an Arora that grew up separately. Bringing it home is mostly
convergence — its store, bridge, and renderer become Arora's `DataStore`,
`Bridge`, and `Hal` impls — plus **one real generalization**: a
`Behavior` trait so a node graph is a first-class behavior alongside the
behavior tree. Vizij's structured types stay concrete and convert through
Arora `Value` via `arora-module-authoring`-generated bindings; their
metadata rides a `/meta/` sidecar. The orchestrator dissolves into
`step()`, keeping only its merge semantics. And the live-edit surface we
build for Vizij — write/subscribe/list on the data plane,
patch/load/inspect on the behavior plane, over both the Bridge and
`arora-web` — is the archetypal way any web app will drive an Arora at
runtime.
