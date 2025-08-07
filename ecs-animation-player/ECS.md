# Animation Player ECS Migration Proposal

This document outlines a proposal for migrating the existing animation player to an Entity-Component-System (ECS) architecture using the [Bevy](https://bevyengine.org/) engine. The primary goal is to enhance modularity, performance, and extensibility while retaining the existing WebAssembly (Wasm) bindings and data-driven output.

This document is a comprehensive plan for the migration, intended to be a self-contained guide for implementation.

## 1. Introduction & Motivation

The current animation player is a monolithic engine that manages players, animations, and interpolation in a tightly coupled, object-oriented manner. While functional, this design presents challenges for future development, performance optimization, and maintainability.

Migrating to Bevy's data-oriented ECS paradigm offers several key advantages:

- **Modularity:** By separating data (Components) from logic (Systems), the codebase becomes easier to understand, maintain, and extend. New features can be added with minimal impact on existing logic.
- **Performance:** Bevy's ECS is designed for high performance through cache-friendly data layouts and automatic parallel execution of systems. This will allow us to handle more complex animations and more numerous instances simultaneously.
- **Extensibility:** The Bevy ecosystem is rich with plugins for physics, UI, rendering, and more. An ECS-based player can more easily integrate with these features in the future.
- **Clarity:** An ECS approach provides a clearer, more declarative structure for representing animated scenes, making the state of the system more transparent and easier to debug.

The migration will be designed to **keep the public Wasm API stable**. The engine will not be responsible for rendering; it will continue to output the calculated animation values, which can be consumed by an external renderer or other logic, preserving its role as a headless animation computer.

## 2. Proposed Architecture

The core of the migration involves mapping the current concepts to Bevy's ECS paradigm. We will create a Bevy `App` that runs within our existing `WasmAnimationEngine` and drives all the animation logic.

### 2.1. Core ECS Structures

#### **Entities**

Entities are simple identifiers that "own" a set of components. We will model our animation hierarchy using entities:

-   **Player Entity:** An entity that represents an animation player. It acts as a timeline and a container for animation instances. It is the root of an animation hierarchy.
-   **Instance Entity:** An entity representing a single, active animation instance. It will be a **child** of a `Player Entity`.
-   **Target Entity:** An entity in the scene that is being animated (e.g., a 3D model, a light, a UI element). These entities will have standard Bevy components like `Name` and `Transform`, as well as custom components to hold animatable data.

#### **Components**

Components are the raw data associated with an entity.

-   **`AnimationPlayer` (Component):** Attached to a `Player Entity`. Manages the overall playback state and timeline.
    -   `name: String`: A human-readable name for the player.
    -   `speed: f64`: The playback speed multiplier.
    -   `mode: PlaybackMode`: Governs looping behavior (`Once`, `Loop`, `PingPong`).
    -   `current_time: AnimationTime`: The player's current position on its timeline.
    -   `playback_state: PlaybackState`: The current state (`Playing`, `Paused`, `Stopped`).
    -   `target_root: Option<Entity>`: **Crucially, this links the player to the root of the entity hierarchy it animates.** This is used for binding.

-   **`AnimationInstance` (Component):** Attached to an `Instance Entity`. Links to an `AnimationData` asset and defines how it should be played.
    -   `animation: Handle<AnimationData>`: A handle to the animation asset to be played.
    -   `weight: f32`: The influence of this instance when blending (0.0 to 1.0).
    -   `time_scale: f32`: A local time multiplier for this specific instance.
    -   `start_time: AnimationTime`: The time on the parent player's timeline when this instance begins.

-   **`AnimationBinding` (Component):** This is the key to performance. It is added to an `AnimationInstance` entity after a one-time binding process. It stores a direct mapping from a track's ID to the target entity and component property that will be animated.
    -   `bindings: HashMap<TrackId, (Entity, BevyPath)>`: Maps a track to the entity and the specific component property (via `bevy_reflect::path::BevyPath`) it animates. This eliminates all per-frame string lookups.

-   **Data Components:** Standard Bevy components on `Target Entities` that hold the data to be modified by animations. These must derive `Component`, `Reflect`, and `Default`.
    -   `bevy::transform::components::Transform`
    -   `bevy::core::Name`: Used to identify targets by name during the binding process.
    -   `AnimatedColor(pub Color)`: A custom component to hold an animatable `Color` value.
    -   `Intensity(pub f32)`: A custom component to hold an animatable float value.
    -   *(Other custom components as needed)*

#### **Assets**

Assets are shareable, loadable data structures.

-   **`AnimationData` (Asset):** The core `AnimationData` struct will become a Bevy `Asset`, loaded via the `AssetServer`. This allows for efficient, reference-counted sharing of animation data across many instances. It will derive `bevy::asset::Asset` and `bevy::reflect::TypePath`.
-   **`BakedAnimationData` (Asset):** Baked animations will also be treated as assets, allowing for a unified loading and usage approach.

#### **Resources**

Resources are global, singleton data structures.

-   **`AnimationOutput` (Resource):** A resource to store the final computed animation values at the end of each frame. This is the primary mechanism for maintaining a stable Wasm API.
    -   `values: HashMap<String, HashMap<String, Value>>`: Maps `player_id -> target_path -> value`.

-   **`FrameBlendData` (Local Resource):** A frame-local resource used to accumulate values for blending before they are applied. It is cleared at the end of each frame.
    -   `blended_values: HashMap<(Entity, BevyPath), Vec<(f32, Value)>>`: Accumulates weighted values for each unique property on each entity.

-   **`InterpolationRegistry` (Resource):** The existing `InterpolationRegistry` will be managed as a Bevy resource, accessible by the animation systems.

#### **Systems**

Systems contain the logic that operates on components. They will be ordered to ensure a correct and predictable data flow.

1.  **`bind_new_animation_instances_system`:**
    -   **Trigger:** Runs once for any new `AnimationInstance` entity that does not yet have an `AnimationBinding` component.
    -   **Query:** `Query<(Entity, &Parent, &AnimationInstance), Added<AnimationInstance>>`
    -   **Logic:**
        1.  For each new instance, get the parent `Player Entity` and read its `AnimationPlayer` component to find the `target_root`.
        2.  Access the `AnimationData` asset using the `Handle` from the `AnimationInstance`.
        3.  For each track in the `AnimationData`, parse its `target` string (e.g., `"LeftArm.Joint/Transform.translation.x"`).
        4.  Search the entity hierarchy under `target_root` for a child with a matching `Name` (e.g., "LeftArm.Joint").
        5.  Once the entity is found, construct a `bevy_reflect::path::BevyPath` for the component property (e.g., `"Transform.translation.x"`).
        6.  Populate a `HashMap<TrackId, (Entity, BevyPath)>`.
        7.  Add an `AnimationBinding` component to the instance entity with the resolved map.

2.  **`update_animation_players_system`:**
    -   **Query:** `Query<&mut AnimationPlayer>`
    -   **Resources:** `Res<Time>`, `EventWriter<AnimationEvent>`
    -   **Logic:**
        1.  Iterate through all `AnimationPlayer` components.
        2.  If `playback_state` is `Playing`, advance `current_time` based on `Time::delta()` and the player's `speed`.
        3.  Handle timeline logic: looping, ping-pong (by reversing `speed`), and stopping.
        4.  If playback ends, send a `PlaybackEnded` event via the `EventWriter`.

3.  **`accumulate_animation_values_system`:**
    -   **Query:** `Query<(&Parent, &AnimationInstance, &AnimationBinding)>`
    -   **Resources:** `Res<AssetServer>`, `Res<Assets<AnimationData>>`, `Res<Assets<BakedAnimationData>>`, `Res<InterpolationRegistry>`, `Local<FrameBlendData>`
    -   **Logic:**
        1.  Iterate through each `AnimationInstance` and its `AnimationBinding`.
        2.  Get the parent `AnimationPlayer` to find the global `current_time`.
        3.  Calculate the instance's local time using its `start_time` and `time_scale`.
        4.  Access the `AnimationData` (or `BakedAnimationData`) asset.
        5.  For each track in the animation, use the `AnimationBinding` to get the target `Entity` and `BevyPath` instantly.
        6.  Sample the animation track at the local time to get the raw `Value`.
        7.  Populate the `FrameBlendData` resource, adding the `(instance.weight, value)` pair to the list for the correct `(Entity, BevyPath)` key.

4.  **`blend_and_apply_animation_values_system`:**
    -   **Resources:** `Local<FrameBlendData>`, `Query<(&mut Transform, &mut AnimatedColor, ...)>` (via reflection)
    -   **Logic:**
        1.  Iterate through the populated `FrameBlendData`.
        2.  For each `(entity, path)` key, process the `Vec<(f32, Value)>`.
        3.  Blend the values into a single final `Value`.
            -   For floats, vectors, etc., this is a weighted average.
            -   For quaternions (`Value::Transform`), use normalized linear interpolation (NLERP) for performance and stability.
        4.  Use `bevy_reflect` to get a mutable reference to the target component on the `entity`.
        5.  Apply the final blended value to the component property using the `BevyPath`.
        6.  Clear `FrameBlendData` for the next frame.

5.  **`collect_animation_output_system`:**
    -   **Query:** `Query<(&Name, &Transform, &AnimatedColor, ...)>` on all entities that could be animated.
    -   **Resources:** `ResMut<AnimationOutput>`
    -   **Logic:**
        1.  This system acts as the bridge to the Wasm API.
        2.  It populates the `AnimationOutput` resource with the final, applied values for the frame.
        3.  It iterates through players and their known targets to reconstruct the `player_id -> target_path -> value` map required by the external API.

#### **Plugin**

-   **`AnimationPlayerPlugin`:** A Bevy `Plugin` that registers all components, assets, resources, and adds the systems to the `Update` schedule in the correct order using `add_systems(Update, (...).chain())`.

### 2.2. Data Flow

The data flow for a single frame is as follows:

1.  Bevy's `Time` resource is updated.
2.  `update_animation_players_system` advances the `current_time` of each `AnimationPlayer`.
3.  `accumulate_animation_values_system` samples all active animations and populates the `FrameBlendData` resource with weighted values for each targeted property.
4.  `blend_and_apply_animation_values_system` processes `FrameBlendData`, calculates the final blended value for each property, and applies it directly to the target components via reflection.
5.  `collect_animation_output_system` reads the final state of the animated components and populates the `AnimationOutput` resource.
6.  The Wasm `update` function, after calling `app.update()`, reads the `AnimationOutput` resource and returns its contents to the JavaScript caller.

## 3. Migration Guide

This section provides a step-by-step plan for refactoring the codebase.

### Step 1: Project Setup & Bevy Integration

1.  **Add Bevy Dependency:** Add `bevy` to `Cargo.toml` with minimal features.
    ```toml
    [dependencies]
    bevy = { version = "0.13", default-features = false, features = ["bevy_asset", "bevy_core", "bevy_ecs", "bevy_reflect", "bevy_hierarchy", "bevy_time"] }
    ```
2.  **Create ECS Module:** Create a new `src/ecs` module to house all Bevy-related code (`plugin.rs`, `components.rs`, `systems.rs`, etc.).
3.  **Integrate Bevy App:** Modify `WasmAnimationEngine` to hold a Bevy `App`. Its constructor will initialize the app, add our `AnimationPlayerPlugin`, and register the necessary resources. The `WasmAnimationEngine::update` method will now simply call `app.update()`.

### Step 2: Define Core ECS Structures

1.  **Define Components:** In `src/ecs/components.rs`, define `AnimationPlayer`, `AnimationInstance`, and `AnimationBinding`. Derive `Component` and `Reflect`.
2.  **Implement `Asset`:** Add `#[derive(Asset, TypePath, Reflect)]` to the existing `AnimationData` and `BakedAnimationData` structs.
3.  **Define Resources:** Define the `AnimationOutput` and `FrameBlendData` resources.

### Step 3: Implement Systems

1.  Implement the systems in `src/ecs/systems.rs` as described in section 2.1.
2.  Pay close attention to the queries and the use of reflection for applying values.
3.  Add the systems to the `AnimationPlayerPlugin`, ensuring they are chained in the correct execution order.

### Step 4: Refactor Wasm Bindings

The `WasmAnimationEngine` will become a thin wrapper around the Bevy `App`, dispatching commands to the `World`.

-   **`create_player()`:** Spawns a new entity with the `AnimationPlayer` component and returns its `Entity` ID, converted to a string.
-   **`add_instance()`:** Spawns a new child entity for the player with an `AnimationInstance` component. The `bind_new_animation_instances_system` will handle the rest automatically.
-   **`update()`:** Calls `app.update()`, then accesses the `AnimationOutput` resource from the `World`, serializes it, and returns it.
-   **Other API calls** (e.g., `play`, `pause`, `seek`) will now query for the relevant `AnimationPlayer` component and modify its state directly.

### Step 5: Testing and Verification

1.  Create test scenes within Bevy that spawn entities with `Name` and `Transform` components.
2.  Write Bevy-native integration tests to:
    -   Verify that `bind_new_animation_instances_system` correctly creates `AnimationBinding` components.
    -   Verify that `update_animation_players_system` advances time correctly.
    -   Verify that `blend_and_apply_animation_values_system` correctly modifies component values.
    -   Verify that blending multiple instances with different weights produces the correct output.
3.  Ensure the output from the Wasm `update` function matches the output of the old engine for the same inputs to guarantee API compatibility.

## 4. Conclusion

Migrating to a Bevy-based ECS architecture is a significant but beneficial undertaking. This plan provides a robust foundation for a more performant, modular, and extensible animation engine. It addresses the key challenges of target binding, blending, and API stability from the outset. By following this detailed proposal, we can modernize the animation player's core, align it with industry best practices, and unlock new possibilities for future features, all while ensuring a smooth transition for existing applications.