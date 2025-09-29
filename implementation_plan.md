# Implementation Plan

[Overview]
Integrate vizij-graph-core and vizij-animation-core via a shared contract (TypedPath + Value + Shape + WriteBatch), exposing two complementary integration modes: a feature-gated AnimationPlayer node inside the graph and a decoupled orchestrator crate for scalable multi-graph/multi-player coordination.

This implementation preserves engine agnosticism and keeps the animation and graph crates decoupled by default. All cross-crate coupling lives behind explicit Cargo features or within a new orchestrator crate. The runtime remains deterministic and inspectable: explicit passes, conflict policies, and per-frame diagnostics make behavior predictable and debuggable. The plan leverages the existing vizij-api-core primitives as the single source of truth for data movement across systems.

[Types]
Minimal additions to the shared API; new graph types and orchestrator structs to formalize contracts between systems.

- Shared API (vizij-api-core):
  - No changes to existing types (Value, Shape, TypedPath, WriteBatch). These remain the lingua franca.
  - Conventions (for node I/O encoding):
    - “Values map” port: Value::Record { key: Text → Value } for exporting Engine changes keyed by canonical handles/paths.
    - “Events” port: Value::List of Value::Record { "kind": Text, "data": Any-like payload }, mirroring CoreEvent at a high level (string kind + JSON-like data using Value).
    - “WriteBatch” port: Value::List of Value::Record { "path": Text, "value": Value, "shape"?: Shape as Value::Record schema-like } when exported as a graph value.
- Graph core (vizij-graph-core) additions:
  - enum NodeType:
    - + AnimationPlayer (behind feature "animation_player")
  - struct NodeParams additions (all Option<...>):
    - animation_json: Option<String> // required to bootstrap player engine
    - player_name: Option<String> // default “player-<node_id>”
    - output_mode: Option<String> // "record" (values/events) | "writebatch"
  - #[cfg(feature = "animation_player")] enum AnimationPlayerOutputMode { Record, WriteBatch } (internal; params use string for schema stability)
- Graph runtime state (vizij-graph-core/src/eval/graph_runtime.rs):
  - enum NodeRuntimeState:
    - + AnimationPlayer(AnimationPlayerState) #[cfg(feature = "animation_player")]
  - struct AnimationPlayerState #[cfg(feature = "animation_player")]:
    - engine: vizij_animation_core::Engine
    - player: vizij_animation_core::PlayerId
    - insts: Vec<vizij_animation_core::InstId>
    - last_json_hash: u64
    - output_mode: AnimationPlayerOutputMode
- Orchestrator (new crate):
  - struct BlackboardEntry {
      value: Value,
      shape: Option<Shape>,
      epoch: u64,
      source: SourceId,
      priority: u8, // reserved for future; default 0
    }
  - type Blackboard = HashMap<TypedPath, BlackboardEntry>
  - struct GraphController { id: GraphId, spec: GraphSpec, rt: GraphRuntime, subscriptions: Subscriptions }
  - struct AnimationController { id: AnimId, engine: Engine }
  - struct Orchestrator {
      blackboard: Blackboard,
      graphs: HashMap<GraphId, GraphController>,
      anims: HashMap<AnimId, AnimationController>,
      schedule: Schedule, // pass ordering configuration
      epoch: u64,
      diagnostics: DiagnosticsCfg,
    }
  - struct OrchestratorFrame {
      epoch: u64,
      dt: f32,
      merged_writes: WriteBatch,
      conflicts: Vec<ConflictLog>,
      timings_ms: HashMap<String, f32>,
      events: Vec<Value>, // high-level diagnostic events (optional)
    }
  - enum Schedule { SinglePass, TwoPass, RateDecoupled { graphs_hz: f32, anims_hz: f32 } }

Validation rules:
- Last-writer-wins by default per pass; “writer” is defined by pass order and controller evaluation order. Optional u8 priorities reserved but not enforced in v1.

[Files]
Introduce a new orchestrator crate and feature-gated changes in graph core; no changes to vizij-animation-core APIs required.

- New files:
  - crates/orchestrator/vizij-orchestrator/Cargo.toml
  - crates/orchestrator/vizij-orchestrator/src/lib.rs
  - crates/orchestrator/vizij-orchestrator/src/blackboard.rs
  - crates/orchestrator/vizij-orchestrator/src/scheduler.rs
  - crates/orchestrator/vizij-orchestrator/src/controllers/graph.rs
  - crates/orchestrator/vizij-orchestrator/src/controllers/animation.rs
  - crates/orchestrator/vizij-orchestrator/src/diagnostics.rs
  - crates/orchestrator/vizij-orchestrator/tests/integration_smoke.rs
- Modified files (vizij-graph-core):
  - Cargo.toml:
    - [features] add feature "animation_player"
    - [dependencies] add optional vizij-animation-core = { path = "../../animation/vizij-animation-core", optional = true }
  - src/types.rs:
    - Add NodeType::AnimationPlayer (gated)
    - Add NodeParams fields: animation_json, player_name, output_mode
  - src/schema.rs:
    - Register AnimationPlayer node (gated) with ports/params as below.
  - src/eval/graph_runtime.rs:
    - Add NodeRuntimeState::AnimationPlayer and AnimationPlayerState
    - Add helper: fn anim_player_state_mut(&mut self, node_id: &NodeId, init: &InitArgs) -> &mut AnimationPlayerState (gated)
  - src/eval/eval_node.rs:
    - Add match arm for NodeType::AnimationPlayer
    - Implement eval_animation_player(...) (gated)
- No changes required in vizij-animation-core; integration occurs via Engine and Inputs/Outputs already present.

[Functions]
New orchestrator API and feature-gated node evaluation logic.

- Orchestrator public API (crates/orchestrator/vizij-orchestrator/src/lib.rs):
  - fn with_graph(self, id: GraphId, spec: GraphSpec, subs: Subscriptions) -> Self
  - fn with_animation(self, id: AnimId, setup: AnimSetup) -> Self
  - fn set_input(&mut self, path: TypedPath, value: Value, shape: Option<Shape>)
  - fn step(&mut self, dt: f32) -> OrchestratorFrame
- GraphController (controllers/graph.rs):
  - fn new(id: GraphId, spec: GraphSpec, subs: Subscriptions) -> Self
  - fn evaluate(&mut self, bb: &mut Blackboard, epoch: u64, dt: f32) -> EvalResult
    - Stages bb inputs into rt.staged_inputs based on Subscriptions, advances epoch, calls evaluate_all, collects rt.writes, and projects selected outputs back into bb if configured.
- AnimationController (controllers/animation.rs):
  - fn new(id: AnimId, engine: Engine) -> Self
  - fn update(&mut self, dt: f32, bb: &mut Blackboard) -> (WriteBatch, Vec<Value>)
    - Constructs Inputs from bb (optional), calls engine.update_writebatch(dt, inputs), returns batch and high-level events.
- Scheduler (scheduler.rs):
  - fn run_single_pass(&mut self, dt: f32) -> OrchestratorFrame
  - fn run_two_pass(&mut self, dt: f32) -> OrchestratorFrame
  - fn maybe_rate_decouple(&mut self, elapsed: f32) // tick anims/graphs at configured rates
- Graph AnimationPlayer node (vizij-graph-core/src/eval/eval_node.rs):
  - #[cfg(feature = "animation_player")] fn eval_animation_player(rt: &mut GraphRuntime, spec: &NodeSpec, inputs: &HashMap<String, PortValue>) -> Result<OutputMap, String>
    - Bootstraps/updates AnimationPlayerState, advances engine with rt.dt, returns according to output_mode:
      - "record": outputs:
        - values: Value::Record { key(Text) → Value }
        - events: Value::List of Value::Record { kind, data }
        - write_batch (optional): Value-encoded list as described
        - Optionally mirror batch to rt.writes when enabled via a boolean param (default true)
      - "writebatch": outputs:
        - write_batch only; optionally mirror into rt.writes.

[Classes]
New Rust structs/enums across orchestrator and graph core.

- New (orchestrator):
  - struct Orchestrator { blackboard, graphs, anims, schedule, epoch, diagnostics }
  - struct GraphController { id, spec, rt, subscriptions }
  - struct AnimationController { id, engine }
  - struct OrchestratorFrame { epoch, dt, merged_writes, conflicts, timings_ms, events }
  - struct BlackboardEntry { value, shape, epoch, source, priority }
  - enum Schedule { SinglePass, TwoPass, RateDecoupled { graphs_hz, anims_hz } }
  - struct Subscriptions { // binds bb paths to graph inputs/outputs
      inputs: Vec<TypedPath>,   // staged into Input nodes
      outputs: Vec<TypedPath>,  // read from graph rt.outputs and published into bb
      writes: bool,             // mirror graph rt.writes to bb
    }
  - struct AnimSetup { players: Vec<PlayerSetup>, prebind: bool, /* future: anim library ingestion */ }
- Modified (vizij-graph-core):
  - enum NodeType { ..., AnimationPlayer } #[cfg(feature = "animation_player")]
  - struct NodeParams { animation_json, player_name, output_mode, ... }
  - enum NodeRuntimeState { ..., AnimationPlayer(AnimationPlayerState) } #[cfg(feature = "animation_player")]
  - struct AnimationPlayerState { engine, player, insts, last_json_hash, output_mode } #[cfg(feature = "animation_player")]

[Dependencies]
New crate and feature, no default cross-crate coupling without opt-in.

- Add new crate: vizij-orchestrator
  - Dependencies:
    - vizij-api-core
    - vizij-graph-core
    - vizij-animation-core
    - hashbrown = { workspace = true }
    - anyhow = { workspace = true }
    - serde = { workspace = true }, serde_json = { workspace = true }
- Update vizij-graph-core/Cargo.toml:
  - features:
    - animation_player = ["vizij-animation-core"]
  - dependencies:
    - vizij-animation-core = { path = "../../animation/vizij-animation-core", optional = true }
- No changes to vizij-animation-core dependency graph.

[Testing]
Layered tests: unit, integration, and end-to-end harnesses.

- Unit tests:
  - vizij-orchestrator:
    - Blackboard conflict resolution (last-writer-wins in-pass), deterministic merges, epoch behavior
    - Scheduler pass ordering and timing counters
  - vizij-graph-core (gated tests):
    - AnimationPlayer node JSON reload (hash changes), engine lifetime, default behaviors for outputs
- Integration tests:
  - Orchestrator SinglePass: anims → graphs; verify writes and values materialize in Blackboard with expected determinism
  - Orchestrator TwoPass: graphs → anims → graphs; verify “feedback” behavior
  - Hybrid: Graphs with internal AnimationPlayer nodes + standalone animations; shared Blackboard
- Diagnostics:
  - Time per pass, number of conflicts, number of writes, per-source write counts; emit as events for dev tooling
- Samples:
  - Include JSON fixtures for simple clips and simple graphs (Input/Output only) to validate cross-system paths

[Implementation Order]
Implement incrementally to preserve binary compatibility and keep changes reviewable.

1) Orchestrator crate scaffold
   - Create vizij-orchestrator with lib, blackboard, scheduler, controllers, diagnostics modules
   - Stub public API: with_graph, with_animation, set_input, step (no-op passes returning empty frame)
2) Blackboard + merge policy
   - Implement BlackboardEntry and merge semantics (last-writer-wins) with conflict logging
   - Add provenance (source id) and epoch
3) GraphController
   - Stage Blackboard inputs into GraphRuntime (rt.set_input for subscribed TypedPath)
   - evaluate_all; collect rt.writes; publish selected outputs back into Blackboard
4) AnimationController
   - Wrap Engine; route Inputs (default empty); call engine.update_writebatch(dt, inputs)
   - Export Outputs changes as Value::Record and events as Value::List of Records; return WriteBatch
5) Scheduler passes
   - SinglePass: Animations → merge → Graphs → merge → frame
   - TwoPass: Graphs → merge → Animations → merge → Graphs → merge → frame
   - RateDecoupled (optional pass frequency controls): tick whichever systems are due; accumulate dt remainder
6) Feature-gated AnimationPlayer node
   - Add Cargo feature "animation_player" to vizij-graph-core
   - Add NodeType::AnimationPlayer and NodeParams fields
   - Add schema entry (ports/params below)
   - Add runtime state variant and eval_animation_player
     - Inputs: "command" (Any/Text), "speed"(Float, optional), "time_scale"(Float, optional), "weights"(Vector, optional)
       "overrides"(Any, optional map for per-handle overrides)
     - Outputs: "values"(Any Record), "events"(Any List of Records), "write_batch"(Any List of Records; optional)
       Params: animation_json (Text; required), player_name (Text; optional), output_mode (Text; "record" | "writebatch")
     - Behavior: advance engine with rt.dt; map optional inputs to Engine::Inputs; when output_mode == "writebatch" and mirror=true, append to rt.writes
7) Diagnostics and logging
   - Add counters and conflict/event logs in orchestrator frame; log AnimationPlayer conflicts when mirroring into rt.writes (if enabled) to help debug overlaps
8) Tests & fixtures
   - Unit/integration as listed; CI wiring
9) Examples
   - Minimal examples demonstrating: graph-only, orchestrated single-pass, orchestrated two-pass, and hybrid (AnimationPlayer node + external Engine)
