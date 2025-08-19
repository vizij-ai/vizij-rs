# ECS Animation Player Implementation Plan

This document provides a detailed, step-by-step plan for building the `ecs-animation-player`. It expands on the architecture proposed in `ECS.md` and `ECS_logic.md`, focusing on how to implement the Wasm-compatible API using Bevy's ECS.

## Phase 1: Core Setup and Asset Integration

**Goal:** Establish the Bevy application, define core data structures as ECS-native types, and handle animation data loading.

1.  **Project Scaffolding:**
    *   Initialize a new Rust library project: `cargo new --lib ecs-animation-player`.
    *   Add dependencies to `Cargo.toml`: `bevy` (with minimal features), `serde`, `serde_json`, `wasm-bindgen`, `uuid`.

2.  **Port Core Data Structures:**
    *   Copy the `animation`, `value`, `time`, `error`, etc. modules from the original `animation-player`.
    *   These structs define the fundamental data and will be reused.

3.  **Bevy Asset Integration:**
    *   In `animation/data.rs`, make `AnimationData` a Bevy asset by adding `#[derive(Asset, TypePath, Reflect)]`.
    *   Do the same for `BakedAnimationData` in `animation/baking.rs`.
    *   This allows Bevy's `AssetServer` to manage loading and sharing of animation data.

4.  **Wasm Engine Boilerplate:**
    *   Create `src/wasm/engine.rs`.
    *   Define `WasmAnimationEngine` to hold a Bevy `App`: `pub struct WasmAnimationEngine { app: App }`.
    *   Implement the constructor `WasmAnimationEngine::new()`:
        *   It should create a new `App::new()`.
        *   Add the `MinimalPlugins` group and the `AssetPlugin`.
        *   Add our custom `AnimationPlayerPlugin` (to be created next).
        *   The `update()` method will simply call `self.app.update()`.

## Phase 2: ECS Components, Resources, and Plugin

**Goal:** Define all necessary ECS constructs and wire them together in a Bevy plugin.

1.  **Create `src/ecs` module:**
    *   `components.rs`: Define `AnimationPlayer`, `AnimationInstance`, `AnimationBinding`, and animatable data components (`AnimatedColor`, `Intensity`) as laid out in `ECS_logic.md`.
    *   `resources.rs`: Define `AnimationOutput` and `FrameBlendData`.
    *   `systems.rs`: Create empty stubs for all the systems described in `ECS.md`.
    *   `plugin.rs`: Create the `AnimationPlayerPlugin`. It should register all components, assets, and resources, and add the (currently empty) systems to the `Update` schedule in the correct, chained order.

2.  **Resource for ID Mapping:**
    *   To bridge the gap between Wasm string IDs and Bevy `Entity` IDs, create a resource:
        ```rust
        #[derive(Resource, Default)]
        pub struct IdMapping {
            pub players: HashMap<String, Entity>,
            pub instances: HashMap<String, Entity>,
            pub animations: HashMap<String, Handle<AnimationData>>,
        }
        ```
    *   Initialize this resource in the `AnimationPlayerPlugin`.

## Phase 3: Implementing the Wasm API

**Goal:** Make the Wasm bindings functional by mapping them to operations on the Bevy `World`. The `WasmAnimationEngine` methods will interact with the `app.world`.

1.  **Engine Management:**
    *   `load_animation(json)`:
        1.  Deserialize the JSON into `AnimationData`.
        2.  Get mutable access to `world.resource_mut::<Assets<AnimationData>>()`.
        3.  Add the data to the asset collection to get a `Handle<AnimationData>`.
        4.  Generate a new string ID, and store the mapping in the `IdMapping` resource.
        5.  Return the string ID.
    *   `update(delta)`:
        1.  Update Bevy's `Time` resource with the `delta`.
        2.  Call `self.app.update()`.
        3.  Access `world.resource::<AnimationOutput>()`.
        4.  Serialize the `output.values` map to `JsValue` and return it.

2.  **Player Management:**
    *   `create_player()`:
        1.  Generate a new string ID for the player.
        2.  Spawn a new entity: `let entity = world.spawn(AnimationPlayer::default()).id();`.
        3.  Store the mapping in `world.resource_mut::<IdMapping>().players`.
        4.  Return the string ID.
    *   `play(player_id)`, `pause(player_id)`, `stop(player_id)`:
        1.  Look up the `Entity` from the `player_id` in the `IdMapping` resource.
        2.  Get the `AnimationPlayer` component: `world.get_mut::<AnimationPlayer>(entity)`.
        3.  Modify the `playback_state` field.
    *   `seek(player_id, time)`:
        1.  Look up the `Entity`.
        2.  Get the `AnimationPlayer` component.
        3.  Set `current_time = time`.

3.  **Instance Management:**
    *   `add_instance(player_id, animation_id, config)`:
        1.  Look up the player `Entity` and animation `Handle` from the `IdMapping` resource.
        2.  Deserialize the config into `AnimationInstanceSettings`.
        3.  Create the `AnimationInstance` component.
        4.  Spawn a new entity with this component: `let instance_entity = world.spawn(instance_component).id();`.
        5.  Use `world.entity_mut(player_entity).add_child(instance_entity);` to create the parent-child relationship.
        6.  Generate and return a string ID for the instance, storing it in the `IdMapping`.
    *   `update_instance_config(player_id, instance_id, config)`:
        1.  Look up the instance `Entity` from the `IdMapping`.
        2.  Get the `AnimationInstance` component: `world.get_mut::<AnimationInstance>(entity)`.
        3.  Update its fields (`weight`, `time_scale`, etc.) from the parsed config.

## Phase 4: Implementing Core Animation Systems

**Goal:** Implement the logic inside the system functions.

1.  **`update_animation_players_system`:**
    *   Implement the logic as described in `ECS_logic.md`. This is a straightforward system that modifies `AnimationPlayer.current_time`.

2.  **`bind_new_animation_instances_system`:**
    *   This is the most complex system.
    *   **Query:** `Query<(Entity, &Parent, &AnimationInstance), Added<AnimationInstance>>`.
    *   **Logic per instance:**
        1.  Get player `Entity` from `&Parent`. Get `target_root` from `player_query.get(player_entity)`.
        2.  Get `AnimationData` from `Res<Assets<AnimationData>>` using the handle on `AnimationInstance`.
        3.  Iterate `animation_data.tracks`.
        4.  For each track, parse `track.target` string (e.g., `"Robot/Arm/Transform.translation.x"`). Split it into an entity path (`"Robot/Arm"`) and a property path (`"Transform.translation.x"`).
        5.  Implement a recursive helper function `find_entity_by_path(start_entity, path_parts, &children_query, &name_query)` to search the hierarchy.
        6.  Once the target entity is found, use `BevyPath::parse()` on the property path.
        7.  Store `(target_entity, bevy_path)` in a `HashMap`.
        8.  When all tracks are bound, add the `AnimationBinding` component to the instance entity using `commands.entity(instance_entity).insert(binding)`.

3.  **`accumulate_animation_values_system`:**
    *   Clear the `FrameBlendData` local resource at the start.
    *   Iterate through all `(AnimationInstance, AnimationBinding)` pairs.
    *   For each track binding, sample the value from the `AnimationData` asset using the `InterpolationRegistry`.
    *   Push the `(weight, value)` tuple into the `FrameBlendData` map for the corresponding `(Entity, BevyPath)`.

4.  **`blend_and_apply_animation_values_system`:**
    *   Iterate through the `FrameBlendData` map.
    *   For each `(entity, path)`, blend the list of weighted values.
        *   Implement blending logic for each `Value` type. For `Transform`, blend position/scale with weighted average and rotation with NLERP.
    *   Use `world.get_entity_mut(entity)` to access the entity.
    *   Use `entity_mut.get_mut::<T>()` to get the component. This will require a large `match` on the component name from the `BevyPath`.
    *   Apply the blended value using `path.apply(&mut *component, &final_value)`.

5.  **`collect_animation_output_system`:**
    *   Clear the `AnimationOutput` resource.
    *   This system needs a way to know which players are animating which targets to reconstruct the output map. The `AnimationBinding` contains this info.
    *   Iterate through all players. For each player, iterate through its instance children. For each instance, iterate its `AnimationBinding`.
    *   For each binding, read the final value from the component on the target entity.
    *   Reconstruct the string path and populate the `AnimationOutput` resource.

## Phase 5: Testing and Refinement

**Goal:** Ensure correctness, performance, and API compatibility.

1.  **Unit Tests:** Write unit tests for individual systems where possible, especially for blending and binding logic.
2.  **Integration Tests:** Create Bevy apps in `tests/` that build scenes, run the `AnimationPlayerPlugin`, and assert that component values are updated correctly.
3.  **Wasm Compatibility Tests:** Reuse or adapt the existing `npm-vite-ts` example to run against the new `ecs-animation-player`. The visual output and console logs should be identical to the original player, confirming API compatibility.
4.  **Benchmarking:** Create benchmarks to compare the performance of the new ECS-based player against the original, especially with a large number of animated objects.
