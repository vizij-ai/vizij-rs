# Developer Feedback and Update Notes

This document summarizes the current ECS implementation state, the latest changes merged to support external tick-based timing, and recommendations moving forward.

## Review Scope

- Compared planned ECS design in `ECS.md` and `ECS_logic.md` against implementation in:
  - `ecs-animation-player/src/ecs/{components.rs, resources.rs, systems.rs, plugin.rs, path.rs}`
  - `ecs-animation-player/src/wasm/engine.rs`
  - `ecs-animation-player/tests/{time_update_test.rs, integration_test.rs}`
  - `Cargo.toml`
- Assessed Rust/Bevy best practices around scheduling, data flow, reflection, assets, and performance.

## Recent Changes: External Tick-Based Time via EngineTime (Aug 8, 2025)

Goal: Ensure the ECS animation player does not rely on internal realtime/virtual time and advances strictly by an externally provided delta, allowing ticks to represent any duration.

Summary of changes:
- Introduced EngineTime resource:
  - File: `src/ecs/resources.rs`
  - Type: `#[derive(Resource, Debug, Clone, Copy)] pub struct EngineTime { pub delta_seconds: f64, pub elapsed_seconds: f64 }`
  - Initialized in Plugin: `AnimationPlayerPlugin` now calls `.init_resource::<EngineTime>()`.
- Systems now tick using EngineTime:
  - File: `src/ecs/systems.rs`
  - `update_animation_players_system` uses `engine_time.delta_seconds * player.speed` instead of `Time` to advance `AnimationPlayer.current_time`.
  - Event timestamps use `engine_time.elapsed_seconds`.
  - Existing tracing logs remain to validate ticks:
    - `update: player='...' delta=... before=... tentative=... duration=... speed=...`
    - `update: player='...' final_time=... state=...`
    - `accumulate: instance ... local_time=... weight=...`
    - `blend_apply: entity=... path=... applied`
- Wasm engine wiring:
  - File: `src/wasm/engine.rs`
  - `update(frame_delta_seconds: f64)` writes:
    - `engine_time.delta_seconds = frame_delta_seconds;`
    - `engine_time.elapsed_seconds += frame_delta_seconds;`
    - Calls `app.update()`, then clears `delta_seconds` to avoid reuse.
- Tests updated to validate explicit ticking and value updates:
  - File: `tests/time_update_test.rs`
    - Drives time exclusively via `EngineTime` (0.5s, 0.5s, 1.0s).
    - Adds a linear transition between keypoints for deterministic interpolation.
    - Sets `PlaybackMode::Once` to ensure playhead reaches the end instead of looping.
    - Adds a player `name` to improve log clarity.
  - File: `tests/integration_test.rs` now advances using `EngineTime` as well.
- Result: `cargo test -p ecs-animation-player --tests` passes. The time update test now asserts the player advances strictly by provided deltas and reaches the correct values:
  - 0.5s → x ≈ 5.0
  - 1.0s → x ≈ 10.0
  - 2.0s → x ≈ 20.0 and playback ends (Once mode)

How to observe logs:
- Set `RUST_LOG="ecs_animation_player=debug,ecs_animation_player::ecs=debug"`.

Impact and API compatibility:
- ECS systems are now decoupled from Bevy `Time` and are driven by `EngineTime`.
- The WebAssembly `WasmAnimationEngine::update(delta)` is the single place where deltas enter the system.
- No breaking changes to the external serialized `AnimationOutput` shape.

## High-Level Alignment with the Plan

- Components:
  - `AnimationPlayer`, `AnimationInstance`, and `AnimationBinding` exist and match intent.
  - `AnimationPlayer` includes a cached `duration` (useful).
  - `AnimationBinding` stores two maps:
    - `raw_track_bindings: HashMap<TrackId, (Entity, BevyPath)>`
    - `baked_track_bindings: HashMap<String, (Entity, BevyPath)>`
- Resources:
  - `AnimationOutput` exists and is produced at end of frame.
  - `FrameBlendData` exists and is currently a `Resource` with single-writer/consumer semantics.
  - `IdMapping` bridges string IDs to `Entity`/`Handle`.
  - NEW: `EngineTime` for external tick-based updates.
- Systems:
  - `bind_new_animation_instances_system`: Resolves and caches track → (Entity, BevyPath); also resolves baked targets.
  - `update_player_durations_system`: Recomputes player duration on child changes.
  - `update_animation_players_system`: Advances time; handles `Loop`, `PingPong`, `Once`; uses EngineTime and emits events on end.
  - `accumulate_animation_values_system`: Samples (raw/baked) and writes to `FrameBlendData` using `InterpolationRegistry`.
  - `blend_and_apply_animation_values_system`: Blends values and applies via reflection.
  - `collect_animation_output_system`: Produces `player_id -> target_path -> Value`.
  - `cleanup_id_mapping_on_despawned_system`: Keeps `IdMapping` synchronized.
- Plugin:
  - `AnimationPlayerPlugin` registers assets/resources/events/types and wires system sets with `.chain()` to enforce order.

## Rust and Bevy Best Practices Assessment

What’s good:
- Deterministic schedule with chained system sets: Bind → UpdatePlayers → Accumulate → BlendApply → Output.
- One-time binding removes per-frame string/path lookups.
- Reflection helpers (`apply_value_to_reflect`, `get_component_value`) centralize type handling, including `bevy::Transform`.
- Clear separation of responsibilities; stable output resource for API boundary.
- Robustness via `warn!` and skip paths for invalid data.
- External tick-based time is now explicit and test-validated.

Gaps and Risks (still applicable or newly observed):
1. FrameBlendData lifecycle and locality
   - Currently a Resource with write/consume within the chained frame.
   - This serializes Accumulate and BlendApply and can reduce parallelism. Consider making it frame-local (`Local`) with a handoff mechanism or document single-writer/consumer semantics.

2. Quaternion blending correctness
   - Current Transform blending averages quaternion components and normalizes. Add hemisphere alignment:
     - For each sample q_i, if dot(q_i, q_ref) < 0 → negate q_i before accumulation.
     - Use NLERP for N-way blends; SLERP for 2-way if precision is critical.

3. Parent/Children API usage consistency
   - Use canonical `Parent`/`Children` consistently (Bevy 0.16) and avoid confusing aliases.
   - Ensure `bevy_hierarchy` is included through the selected Bevy dependency strategy.

4. Player duration maintenance triggers
   - `update_player_durations_system` only reacts to Added/Changed children.
   - Consider recalculating when `AnimationInstance` fields change (`start_time`, `time_scale`, `animation`), or opportunistically (guarded by a dirty flag). For baked data timing, asset events can drive recomputation.

5. Asset lookups per frame
   - Avoid O(N) baked lookups via `.iter().find(...)`. Maintain an index `HashMap<String, Handle<BakedAnimationData>>` keyed by `animation_id` via `AssetEvent` to get O(1) baked retrievals.

6. Event payload fidelity
   - Do not format handles via `{:?}` for IDs. Emit stable identifiers, e.g., `AnimationData.id`, instance ID, and/or player ID.

7. Reflection hot path caching
   - Cache resolved `TypeId` and parsed property paths in `AnimationBinding` during bind. Then use `get_reflect_mut` with `TypeId` directly to avoid registry string lookups on hot paths.

8. Defensive programming and logging in hot paths
   - Replace `unwrap()` on time conversions with `expect(...)` or clamp to safe domain.
   - Avoid `warn!` in hot loops; prefer `debug!` unless it truly indicates a data problem.

9. Scheduling and `World` exclusivity
   - `blend_and_apply_animation_values_system(World)` requires `&mut World` and serializes the set. If performance becomes a concern, consider switching to typed queries per component to reduce exclusive access.

10. Cargo feature hygiene
    - Ensure Bevy features include hierarchy/core/reflect/time/etc. Consolidate to the monolithic `bevy` crate with minimal features or use modular crates consistently to prevent drift.

11. Minor issues noticed
    - `unused variable: baked_data` warning in `systems.rs` when baked data is compiled but not used in a specific branch; consider prefixing with `_` or adjusting logic.

## Test Status

- All ECS tests pass with the new timing model:
  - `time_update_test.rs`: Passes with EngineTime-driven ticks, linear interpolation, Once mode, and detailed logs confirming expected stepping.
  - `integration_test.rs`: Passes using EngineTime for progression.
- Enables more deterministic test scenarios (e.g., large/small arbitrary deltas) and removes reliance on Bevy’s internal time for correctness.

## Suggested Next Steps (Prioritized)

2. Quaternion hemisphere correction
   - Implement hemisphere alignment and add tests to cover opposite quaternion blends and boundary rotations.

3. Duration recomputation coverage
   - Add triggers for instance property changes and animation handle changes; consider asset events for late asset availability; optional dirty-flag recomputation.

4. Baked animation index
   - Introduce a `BakedIndex` resource keyed by `animation_id`. Replace `.iter().find(...)` with index lookups in `accumulate`/`output`.

5. FrameBlendData lifecycle
   - Either keep as Resource but document constraints and single-writer assumptions, or convert to `Local` with a structured handoff to maximize parallelism.

6. Event payloads
   - Emit stable IDs (e.g., `AnimationData.id`, player ID from `IdMapping`) rather than handle debug strings.

7. Reflection metadata caching
   - Extend `AnimationBinding` entries with cached `TypeId` and reuse in blend/apply to reduce registry lookups.

8. Hardening and logging hygiene
   - Replace `unwrap()` with `expect`/clamp where appropriate.
   - Reduce `warn!` spam in expected control paths; keep debug logs for step validation.

9. Cargo dependency alignment
    - Consolidate Bevy dependency approach and ensure all required features (hierarchy, reflect, time, core, transform) are enabled.

## API Parity Summary (Wasm)

- snake_case exports (matching non-ECS):
  - Engine: new, update, get_engine_config, set_engine_config, drain_events
  - Animation assets: load_animation, unload_animation, animation_ids, export_animation, bake_animation
  - Player lifecycle/control: create_player, remove_player, play, pause, stop, seek
  - Player info: get_player_settings, get_player_state, get_player_duration, get_player_time, get_player_progress, get_player_ids
  - Player config: update_player_config, set_player_root (ECS-only, optional)
  - Instances: add_instance, remove_instance, update_instance_config, get_instance_config
  - Analysis/Utilities: get_derivatives, load_test_animation_from_json_wasm, value_to_js
- camelCase aliases are retained via wasm_bindgen(js_name) for existing ECS consumers (e.g., loadAnimation, createPlayer, getPlayerSettings, getDerivatives, etc.).

## Binding-less Fallback Behavior (Drop-in Parity)

- set_player_root is OPTIONAL. When no AnimationBinding exists (no set_player_root or unresolved target), collect_animation_output_system now computes current values directly from AnimationData:
  - Samples each instance’s tracks at local_time = (player.current_time - instance.start_time) * time_scale
  - Blends values by instance.weight per target, and populates AnimationOutput as: player_id -> { target_path -> Value }
  - If bindings exist, binding-based values take precedence and fallback will not overwrite them
- Diagnostics:
  - Warn when a player has instances but no target_root and no bindings (outputs may be empty)
  - Warn when a player produced an empty output map (for quick troubleshooting)
- Tests:
  - ecs-animation-player/tests/output_fallback_test.rs:
    - fallback_sampling_produces_output_without_bindings validates non-empty output without set_player_root
    - disabled_instance_is_skipped_in_fallback_output ensures disabled instances do not contribute

## Root Entity Convenience

- Wasm update_player_config accepts an optional rootEntity to set/clear target_root ergonomically
- In ECS integration tests, setting AnimationPlayer.target_root at spawn time and adding instances ensures bind_new_animation_instances_system resolves bindings on the next update

## Derivatives and Baking Parity

- get_derivatives mirrors non-ECS semantics and shape (width validation, per-target derivative computation, optional derivative_width_ms)
- bake_animation produces baked JSON like the non-ECS engine (not just storing an asset), enabling drop-in usage patterns

## Test Status (Step 8)

- Added output_fallback_test.rs covering binding-less fallback and instance.enabled behavior
- Existing tests (integration_test.rs, time_update_test.rs) validate binding path, EngineTime-driven updates, and deterministic interpolation
- All tests pass via cargo test across crates
