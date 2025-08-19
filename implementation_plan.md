# Implementation Plan

[Overview]
Add missing wasm bindings to the ECS animation player so its public WebAssembly API is drop-in compatible with the non-ECS animation player. The goal is to mirror function names, parameter shapes, and return payloads, while keeping ECS-specific APIs available but optional.

This plan analyzes both wasm interfaces and identifies gaps in the ECS version. The non-ECS bindings expose a complete set of engine configuration, animation asset management, player control, per-instance control, baking, and derivative calculation APIs. The ECS bindings currently provide a subset: player lifecycle and control, engine update, animation load, and ECS-specific set_player_root and drain_events. Missing parity includes: engine config getters/setters, unload_animation, animation_ids, full per-instance APIs (add/remove/update/get config), export_animation, get_derivatives, and bake_animation. Naming differences also exist (camelCase in ECS vs snake_case in non-ECS).

This implementation will:
- Add all missing wasm methods to ECS and ensure identical input/output shapes as non-ECS.
- Provide snake_case exports matching non-ECS, while retaining existing ECS camelCase names as optional aliases.
- Add minimal type support in ECS to fully represent and mutate instance and player configuration, including “enabled” and per-instance start time.
- Wire baking and derivatives to existing ECS logic, returning JSON payloads identical to non-ECS.

[Types]  
The ECS wasm type surface will be extended to align with non-ECS shapes.

Detailed type definitions, interfaces, enums, or data structures:

- EngineConfig (JSON)
  - timeStep: number (float, seconds) — Optional. Stored but not enforced by schedule; affects default delta if needed.
  - maxUpdatesPerFrame: number (integer) — Optional. For parity only; not strictly used in ECS schedule but preserved in resource.
  - Validation: timeStep > 0; maxUpdatesPerFrame >= 1.

- BakingConfig (JSON)
  - frameRate: number (float, Hz) — Default 60.0 if absent.
  - startTime: number (float, seconds) — Default 0.0 if absent.
  - endTime: number (float, seconds) — Optional; if present must be > startTime.
  - Validation mirrors non-ECS.

- InstanceConfig (JSON) — used by add_instance/update_instance_config/get_instance_config
  - weight: number (float) — Optional, default 1.0.
  - timeScale: number (float) — Optional, default 1.0.
  - instanceStartTime: number (float, seconds) — Optional, default 0.0.
  - enabled: boolean — Optional, default true.
  - Validation: timeScale can be any finite; weight finite; instanceStartTime >= 0.

- PlayerSettings (JSON return type of get_player_settings)
  - name: string
  - speed: number (float)
  - mode: "once" | "loop" | "ping_pong"
  - loop_until_target: number | null (retain ECS field; fine to keep as null)
  - offset: number — return 0.0 for parity if not used
  - start_time: number — persisted field (see modifications)
  - end_time: number | null — persisted field (see modifications)
  - instance_ids: string[] — child instance IDs

- PlayerState (JSON return type of get_player_state)
  - playback_state: string ("Playing" | "Paused" | "Stopped" or equivalent naming used in non-ECS)
  - last_update_time: number (float, seconds)
  - current_loop_count: number (u32) — 0 if not tracked
  - is_playing_forward: boolean — speed >= 0

- ECS Components (new/updated)
  - AnimationInstance (ecs-animation-player/src/ecs/components.rs)
    - weight: f32
    - time_scale: f32
    - start_time: AnimationTime
    - enabled: bool (new; default true) — systems must skip disabled instances
  - AnimationPlayer (ecs-animation-player/src/ecs/components.rs)
    - name: String
    - speed: f64
    - mode: PlaybackMode
    - current_time: AnimationTime
    - duration: AnimationTime
    - playback_state: PlaybackState
    - target_root: Option<Entity>
    - start_time: AnimationTime (new)
    - end_time: Option<AnimationTime> (new)

- Resources
  - IdMapping (existing)
    - players: Map<String, Entity>
    - instances: Map<String, Entity>
    - animations: Map<String, Handle<AnimationData>>
  - EngineTime (existing)
    - delta_seconds: f64
    - elapsed_seconds: f64
  - EngineConfigEcs (new)
    - time_step: f64
    - max_updates_per_frame: u32

[Files]
We will modify ECS wasm and ECS engine/components to add parity; no changes to non-ECS code.

Detailed breakdown:
- New files to be created
  - None required. (Optionally a helper module for derivatives if desired.)

- Existing files to be modified
  - ecs-animation-player/src/wasm/engine.rs
    - Change constructor to accept Option<String> (config_json?: string) for parity; parse EngineConfigEcs, store as resource.
    - Add get_engine_config() -> JsValue and set_engine_config(config_json: &str) -> Result<(), JsValue>.
    - Extend load_animation to add the same fallback loading path as non-ECS:
      - If serde_json parse fails, call load_test_animation_from_json_wasm(json_str) then parse again.
    - Add unload_animation(animation_id: &str) -> Result<(), JsValue>: remove from IdMapping.animations and from Assets.
    - Add animation_ids() -> Vec<String>.
    - Export names in snake_case to match non-ECS and keep existing camelCase (via duplicated bindings using js_name).
  - ecs-animation-player/src/wasm/animation.rs
    - Add impl WasmAnimationEngine methods:
      - add_instance(player_id, animation_id, config_json?: string) -> Result<String, JsValue>
      - remove_instance(player_id, instance_id) -> Result<(), JsValue>
      - update_instance_config(player_id, instance_id, config_json: &str) -> Result<(), JsValue>
      - get_instance_config(player_id, instance_id) -> Result<String, JsValue>
      - export_animation(animation_id: &str) -> Result<String, JsValue>
      - get_derivatives(player_id: &str, derivative_width_ms?: f64) -> Result<JsValue, JsValue>
      - bake_animation(animation_id: &str, config_json?: string) -> Result<String, JsValue>
    - Add free function load_test_animation_from_json_wasm(json_str: &str) -> Result<String, JsValue> that proxies to loaders (parity with non-ECS).
    - Ensure snake_case exports exist; keep any ECS camelCase aliases as optional.
  - ecs-animation-player/src/wasm/player.rs
    - Add snake_case export aliases for all existing camelCase functions (create_player, remove_player, get_player_settings, get_player_state, get_player_duration, get_player_time, get_player_progress, get_player_ids, update_player_config, set_player_root, play, pause, stop, seek) so both forms exist. The snake_case names must match non-ECS.
    - Update get_player_settings to output start_time and end_time from component (after component changes).
  - ecs-animation-player/src/ecs/components.rs
    - Add enabled: bool to AnimationInstance (default true).
    - Add start_time: AnimationTime and end_time: Option<AnimationTime> to AnimationPlayer for parity with non-ECS player settings.
    - Update Default impl accordingly.
  - ecs-animation-player/src/ecs/systems.rs
    - Respect AnimationInstance.enabled during accumulation/blending.
    - Clamp/limit playback based on AnimationPlayer.start_time/end_time where applicable (e.g., seeking, updating current_time, progress computation).
    - Ensure systems that compute duration/progress account for start/end windows consistently.
  - ecs-animation-player/src/ecs/resources.rs
    - Add EngineConfigEcs Resource and register it in the App on engine construction; expose to get/set functions.

- Files to be deleted or moved
  - None.

- Configuration file updates
  - None (no new external dependencies).

[Functions]
Add all missing wasm interface functions to mirror non-ECS; preserve existing ECS-only functions.

Detailed breakdown:
- New functions (name, signature, file path, purpose)
  - WasmAnimationEngine::get_engine_config(&self) -> JsValue
    - Path: ecs-animation-player/src/wasm/engine.rs
    - Purpose: Return EngineConfigEcs as JSON; parity with non-ECS get_engine_config.
  - WasmAnimationEngine::set_engine_config(&mut self, config_json: &str) -> Result<(), JsValue>
    - Path: ecs-animation-player/src/wasm/engine.rs
    - Purpose: Parse and store EngineConfigEcs resource; parity with non-ECS set_engine_config.
  - WasmAnimationEngine::unload_animation(&mut self, animation_id: &str) -> Result<(), JsValue>
    - Path: ecs-animation-player/src/wasm/engine.rs
    - Purpose: Remove asset handle from IdMapping and Assets; parity with non-ECS.
  - WasmAnimationEngine::animation_ids(&mut self) -> Vec<String>
    - Path: ecs-animation-player/src/wasm/engine.rs
    - Purpose: Return currently loaded animation IDs; parity with non-ECS.
  - WasmAnimationEngine::add_instance(&mut self, player_id: &str, animation_id: &str, config_json: Option<String>) -> Result<String, JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Create child entity with AnimationInstance component; store in IdMapping.instances; parse InstanceConfig defaults and apply.
  - WasmAnimationEngine::remove_instance(&mut self, player_id: &str, instance_id: &str) -> Result<(), JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Despawn instance entity and remove mapping; parity with non-ECS remove_instance.
  - WasmAnimationEngine::update_instance_config(&mut self, player_id: &str, instance_id: &str, config_json: &str) -> Result<(), JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Update AnimationInstance.weight, time_scale, start_time, enabled.
  - WasmAnimationEngine::get_instance_config(&self, player_id: &str, instance_id: &str) -> Result<String, JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Serialize AnimationInstance to InstanceConfig JSON; parity with non-ECS get_instance_config.
  - WasmAnimationEngine::export_animation(&self, animation_id: &str) -> Result<String, JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Fetch AnimationData by handle and serde_json::to_string; parity with non-ECS export_animation.
  - WasmAnimationEngine::get_derivatives(&mut self, player_id: &str, derivative_width_ms: Option<f64>) -> Result<JsValue, JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Mirror non-ECS behavior: validate width>0 if provided; compute derivatives over window centered at current time (or forward difference with width) and return { "property.path": float }.
    - Implementation detail: Use existing derivative functions (animation/baking.rs, track.rs) to sample and compute rates for the player’s bound properties. If necessary, add an internal helper to compute per-track derivative via finite differences using ECS interpolation at t and t+dt.
  - WasmAnimationEngine::bake_animation(&mut self, animation_id: &str, config_json: Option<String>) -> Result<String, JsValue>
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Invoke ECS baking path to create BakedAnimationData using BakingConfig; return baked JSON like non-ECS (do not only store in Assets).
  - load_test_animation_from_json_wasm(json_str: &str) -> Result<String, JsValue> (free function)
    - Path: ecs-animation-player/src/wasm/animation.rs
    - Purpose: Provide the same fallback test loader entrypoint that non-ECS exports (wrap crate::loaders::load_test_animation_from_json).

- Modified functions (exact name, current file path, required changes)
  - WasmAnimationEngine::new (ecs-animation-player/src/wasm/engine.rs)
    - Change signature to new(config_json: Option<String>), keep #[wasm_bindgen(constructor)].
    - Initialize App + plugins, register resources (EngineTime default, EngineConfigEcs parsed or default).
  - WasmAnimationEngine::load_animation (ecs-animation-player/src/wasm/engine.rs)
    - Add fallback parse logic identical to non-ECS: on parse error, call load_test_animation_from_json_wasm then parse.
    - Keep returning new UUID id mapped to Handle<AnimationData>.
  - Player-related getters (ecs-animation-player/src/wasm/player.rs)
    - get_player_settings: Include start_time and end_time from component (new fields), keep instance_ids aggregation.

- Removed functions (name, file path, reason, migration strategy)
  - None.

[Classes]
WasmAnimationEngine gains configuration parity and additional methods; ECS components gain fields for parity.

Detailed breakdown:
- New classes
  - EngineConfigEcs Resource (name: EngineConfigEcs; file: ecs-animation-player/src/ecs/resources.rs or a new config.rs). Fields:
    - time_step: f64 (default 0.016)
    - max_updates_per_frame: u32 (default 10)
- Modified classes
  - WasmAnimationEngine (ecs-animation-player/src/wasm/engine.rs):
    - Constructor signature change, additional exported methods, name aliases for snake_case/camelCase where needed.
  - AnimationInstance (ecs-animation-player/src/ecs/components.rs):
    - Add enabled: bool default true.
  - AnimationPlayer (ecs-animation-player/src/ecs/components.rs):
    - Add start_time: AnimationTime; end_time: Option<AnimationTime>.
- Removed classes
  - None.

[Dependencies]
No external dependencies are added. Existing crates (uuid, serde, bevy, wasm-bindgen) remain unchanged. All changes are internal to the ECS crate and wasm surface.

[Testing]
Use integration tests and targeted unit tests to validate parity.

- New/updated tests:
  - ecs-animation-player/tests/integration_test.rs
    - New cases for:
      - Engine: get_engine_config/set_engine_config roundtrip (serde of JSON).
      - Assets: load_animation fallback path from test JSON; unload_animation removes id; animation_ids reflects state.
      - Instances: add_instance/update_instance_config/get_instance_config/remove_instance; enabled toggling affects output (e.g., weight zero when disabled or systems skip).
      - Bake: bake_animation returns JSON string with expected schema.
      - Derivatives: get_derivatives returns mapping; validate error on non-positive width.
      - Player: update_player_config with startTime/endTime persists and seeking respects clamp; get_player_settings includes new fields.
  - wasm-level smoke tests (if applicable via web tests) to verify snake_case exports exist and match non-ECS function names.

Validation strategies:
- Compare outputs from ECS API with those from non-ECS for the same inputs where deterministic (e.g., export_animation on the same JSON).
- Ensure calling only snake_case methods works fully (drop-in compatibility).
- Ensure ECS-specific methods (set_player_root, drain_events, camelCase aliases) remain functional but optional.

[Implementation Order]
Implement parity incrementally to minimize conflicts and verify along the way.

1) Engine constructor and config
   - Change WasmAnimationEngine::new to accept Option<String>, add EngineConfigEcs resource.
   - Implement get_engine_config/set_engine_config and tests.

2) Animation asset management
   - Add fallback loader, unload_animation, animation_ids.
   - Tests for load/unload/ids.

3) Player settings fields parity
   - Add start_time/end_time to AnimationPlayer; wire get_player_settings; update update_player_config to persist properly.
   - Clamp logic in systems for start/end time.
   - Tests for persistence and clamping.

4) Instance lifecycle parity
   - Add enabled to AnimationInstance; implement add_instance/remove_instance/update_instance_config/get_instance_config.
   - Modify systems to skip disabled instances.
   - Tests for instance config roundtrip and enabled behavior.

5) Baking parity
   - Implement bake_animation that returns JSON (do not only store handle).
   - Tests comparing baked JSON schema (optionally spot-check values).

6) Derivatives parity
   - Implement get_derivatives with optional derivative_width_ms mirror, error handling, and output shape.
   - Tests for shape and validation errors.

7) Naming parity
   - Add snake_case exports for all existing wasm ECS methods; retain camelCase via js_name aliases.
   - Add snake_case variants for load_animation (and keep loadAnimation), player methods, etc.
   - Web-level smoke tests to ensure both naming styles are exported.

8) Documentation and cleanup
   - Update any Wasm docs/comments to clarify parity and optional ECS-specific functions.
   - Ensure no breaking changes for current ECS consumers due to added optional parameters or fields.

[Status]
- [x] Step 1: Engine constructor/config parity (new(config_json?), EngineConfigEcs resource, get_engine_config/set_engine_config)
- [x] Step 2: Animation asset parity (fallback loader, unload_animation, animation_ids)
- [x] Step 3: Player settings parity (add start_time/end_time to component, update getters, clamp logic in systems)
- [x] Step 4: Instance lifecycle parity (add enabled flag, add_instance/remove_instance/update_instance_config/get_instance_config, systems honor enabled)
- [x] Step 5: Baking parity (bake_animation returns baked JSON identical to non-ECS)
- [x] Step 6: Derivatives parity (get_derivatives with identical semantics and output shape)
- [x] Step 7: Naming parity (add snake_case wasm exports; keep camelCase js_name aliases)
- [ ] Step 8: Tests and docs (integration tests for new APIs; update comments to state ECS-only methods are optional)

[Progress]
- 2025-08-19:
  - Added EngineConfigEcs resource (ecs-animation-player/src/ecs/resources.rs) wrapping AnimationEngineConfig with Default::web_optimized().
  - Updated WasmAnimationEngine constructor to accept Option<String> config JSON and insert EngineConfigEcs into the Bevy App (ecs-animation-player/src/wasm/engine.rs).
  - Added get_engine_config / set_engine_config wasm bindings with snake_case and camelCase aliases (getEngineConfig / setEngineConfig).
  - Animation assets parity: load_animation now supports native JSON + test_animation fallback; unload_animation and animation_ids implemented with snake_case and camelCase aliases (ecs-animation-player/src/wasm/engine.rs).
  - Player parity: Added start_time/end_time to AnimationPlayer; get_player_settings returns them; seek and progress respect window; systems clamp/reflect within [start,end] (ecs-animation-player/src/ecs/components.rs, ecs-animation-player/src/ecs/systems.rs, ecs-animation-player/src/wasm/player.rs).
  - Instance lifecycle: AnimationInstance gained enabled; wasm methods add_instance, remove_instance, update_instance_config, get_instance_config implemented (ecs-animation-player/src/wasm/animation.rs); systems skip disabled instances.
  - Baking: bake_animation implemented to return baked JSON consistent with non-ECS (ecs-animation-player/src/wasm/animation.rs).
  - Derivatives: get_derivatives implemented with optional width and camelCase alias (getDerivatives); blends per-target derivatives across instances (ecs-animation-player/src/wasm/animation.rs).
  - Fixed borrow checker errors (E0502) in collect_animation_output_system by avoiding simultaneous mutable/immutable borrows of World; replaced resource_mut InterpolationRegistry with a local instance. Also fixed E0614 by removing dereferences of Entity. All tests now pass.

[Findings: Output parity and front-end current values]
- Risk identified: setPlayerRoot is ECS-only and intended to be optional, but current output collection relies on resolved bindings (via setPlayerRoot) to reflect component values. If a consumer does not call setPlayerRoot, no bindings are created and collect_animation_output_system produces empty outputs. This breaks drop-in usage where non-ECS returned evaluated values without a scene graph.
- Impact: Front ends expecting update() to return current values (by track target path) see empty objects when setPlayerRoot is not used.
- Remediation plan:
  1) Add a binding-less fallback in collect_animation_output_system:
     - When no AnimationBinding exists for an instance (or player has no target_root), evaluate raw track values directly at the instance’s local time and emit them keyed by track.target, blended by instance weights (just like non-ECS).
     - Preserve existing binding-based path (preferred when bindings exist) but ensure the fallback always fills outputs for tracks with no bindings.
  2) Instrument debug logs to surface misconfiguration:
     - Warn once per player per few seconds if target_root is None and there are instances (“player has no target_root; using binding-less fallback output”).
     - Warn if a player has instances but both raw and baked bindings are empty (“no bindings created; output is produced via fallback sampling”).
     - Info when output map is empty after both binding and fallback paths.
  3) Document in README/DeveloperFeedback that setPlayerRoot is optional; when omitted, outputs are computed directly from track data (matching non-ECS behavior).
  4) Provide a config flag (future) to force binding-less output mode (useful for headless/data use).

[Next Steps]
- Step 8: Tests and docs
  - Add/extend tests to cover: no set_player_root fallback returns current values; update_player_config rootEntity sets/clears target_root; derivatives width validation and shapes; bake_animation JSON schema; instance enabled behavior.
  - Update DeveloperFeedback/README to document optional ECS-only APIs, fallback behavior, and snake_case/camelCase parity.
- Cleanup
  - Remove or #[allow(dead_code)] reflect_component_mut to silence warnings.
  - Quick audit of exported wasm names to confirm final parity list and optional aliases.
