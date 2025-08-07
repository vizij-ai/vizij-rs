# ECS Logic and Code Structure

This document provides the proposed Rust code structure for the components, assets, resources, and systems required for the Bevy ECS migration. Each snippet includes a docstring explaining its role and responsibilities within the animation engine.

## 1. Components

Components are the fundamental data building blocks in the ECS. They are attached to entities to define their properties and state.

### `AnimationPlayer`

```rust
use bevy::prelude::*;
use bevy::reflect::Reflect;
use crate::{AnimationTime, PlaybackMode, PlaybackState};

/// Represents an animation player, acting as a timeline and container for animation instances.
/// This component is attached to a top-level Player Entity and defines the root of an
/// animation hierarchy.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationPlayer {
    /// A human-readable name for the player, useful for debugging.
    pub name: String,
    /// The playback speed multiplier. Can be negative for reverse playback.
    pub speed: f64,
    /// Governs looping behavior (`Once`, `Loop`, `PingPong`).
    pub mode: PlaybackMode,
    /// The player's current position on its timeline.
    pub current_time: AnimationTime,
    /// The current state of the player (`Playing`, `Paused`, `Stopped`).
    pub playback_state: PlaybackState,
    /// A crucial link to the root of the entity hierarchy that this player animates.
    /// This is used by the binding system to resolve track targets.
    pub target_root: Option<Entity>,
}
```

### `AnimationInstance`

```rust
use bevy::prelude::*;
use bevy::reflect::Reflect;
use crate::{AnimationData, AnimationTime};

/// Represents a single, active animation being played.
/// This component is attached to an Instance Entity, which is always a child of a
/// Player Entity. It links to an `AnimationData` asset and defines how it should be played.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationInstance {
    /// A handle to the `AnimationData` asset to be played.
    pub animation: Handle<AnimationData>,
    /// The influence of this instance when blending with others (0.0 to 1.0).
    pub weight: f32,
    /// A local time multiplier for this specific instance, allowing it to play
    /// faster or slower than the parent player's timeline.
    pub time_scale: f32,
    /// The time on the parent player's timeline when this instance begins.
    pub start_time: AnimationTime,
}
```

### `AnimationBinding`

```rust
use bevy::prelude::*;
use bevy::reflect::{Reflect, path::BevyPath};
use std::collections::HashMap;
use crate::TrackId;

/// Stores the resolved mapping from an animation track to a target entity and component property.
/// This component is the key to performance, as it is added to an `AnimationInstance` entity
/// after a one-time binding process. It eliminates the need for expensive, per-frame string
/// lookups and hierarchy searches.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimationBinding {
    /// The map from a track's unique ID to the resolved `Entity` and the specific
    /// `BevyPath` pointing to the component property it animates.
    pub bindings: HashMap<TrackId, (Entity, BevyPath)>,
}
```

### Data Components

These are examples of custom components that would be placed on `Target Entities` to hold the data that animations will modify.

```rust
use bevy::prelude::*;
use bevy::reflect::Reflect;
use crate::Color;

/// A custom component to hold an animatable `Color` value.
/// Deriving `Reflect` is essential for the animation system to apply values dynamically.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct AnimatedColor(pub Color);

/// A custom component to hold an animatable float value, for example, a light's intensity.
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct Intensity(pub f32);
```

## 2. Assets

Assets are shareable, loadable data structures managed by Bevy's `AssetServer`.

### `AnimationData`

```rust
use bevy::prelude::*;
use bevy::reflect::{Reflect, TypePath};
use serde::{Deserialize, Serialize};

/// Represents the core, shareable animation data, now as a Bevy Asset.
/// This allows for efficient, reference-counted sharing of animation data across many instances.
/// The `TypePath` trait is required for assets.
#[derive(Asset, TypePath, Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnimationData {
    // ... all existing fields from the current AnimationData struct ...
    pub id: String,
    pub name: String,
    // ... etc
}
```

### `BakedAnimationData`

```rust
use bevy::prelude::*;
use bevy::reflect::{Reflect, TypePath};
use serde::{Deserialize, Serialize};

/// Represents pre-calculated (baked) animation data as a Bevy Asset.
/// This provides a performance-optimized path for playing back baked animations, as it
/// avoids the need for real-time interpolation calculations.
#[derive(Asset, TypePath, Reflect, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BakedAnimationData {
    // ... all existing fields from the current BakedAnimationData struct ...
    pub animation_id: String,
    pub frame_rate: f64,
    // ... etc
}
```

## 3. Resources

Resources are global, singleton data structures accessible by systems.

### `AnimationOutput`

```rust
use bevy::prelude::*;
use std::collections::HashMap;
use crate::Value;

/// A global resource that stores the final computed animation values at the end of each frame.
/// This resource is the primary bridge to the external Wasm API, ensuring its stability by
/// providing a consistent, queryable output format.
#[derive(Resource, Default)]
pub struct AnimationOutput {
    /// Maps a player's ID (as a string) to its map of animated targets.
    /// The inner map maps a target's path (e.g., "LeftArm.Joint/Transform.translation.x")
    /// to its final computed `Value` for the frame.
    pub values: HashMap<String, HashMap<String, Value>>,
}
```

### `FrameBlendData`

```rust
use bevy::prelude::*;
use bevy::reflect::path::BevyPath;
use std::collections::HashMap;
use crate::Value;

/// A frame-local resource used to accumulate weighted values for blending before they are applied.
/// It is cleared at the end of each frame. This intermediate step is essential for correctly
/// layering and blending multiple animations that target the same property.
#[derive(Default)]
struct FrameBlendData {
    /// Maps a unique entity-property pair to a vector of weighted values from all
    /// contributing animation instances for the current frame.
    blended_values: HashMap<(Entity, BevyPath), Vec<(f32, Value)>>,
}
```

## 4. Systems

Systems contain the logic that operates on entities and their components.

### `bind_new_animation_instances_system`

```rust
/// This system runs once for each new `AnimationInstance` to resolve and cache its track bindings.
/// It finds the target entity and component property for each track and stores the direct
/// mapping in an `AnimationBinding` component. This one-time setup is crucial for performance,
/// as it avoids costly lookups during the main animation loop.
fn bind_new_animation_instances_system(
    mut commands: Commands,
    new_instances_query: Query<(Entity, &Parent, &AnimationInstance), Added<AnimationInstance>>,
    player_query: Query<&AnimationPlayer>,
    animations: Res<Assets<AnimationData>>,
    // Query to traverse the entity hierarchy
    children_query: Query<&Children>,
    name_query: Query<&Name>,
) {
    // Implementation details:
    // 1. For each new instance, get its parent player and the `target_root`.
    // 2. Load the `AnimationData` asset.
    // 3. For each track, parse the target string (e.g., "ObjectName/Transform.translation.x").
    // 4. Recursively search the `target_root` hierarchy for an entity with a matching `Name`.
    // 5. Construct a `BevyPath` from the property part of the string.
    // 6. Create and add the `AnimationBinding` component to the instance entity.
}
```

### `update_animation_players_system`

```rust
/// Advances the timeline for all `AnimationPlayer` components based on the elapsed frame time.
/// It is responsible for the core playback logic, including handling speed, looping, ping-pong,
/// and stopping. It also fires events (e.g., `PlaybackEnded`) to notify other parts of the engine.
fn update_animation_players_system(
    mut player_query: Query<&mut AnimationPlayer>,
    time: Res<Time>,
    mut event_writer: EventWriter<AnimationEvent>,
) {
    // Implementation details:
    // 1. Iterate through players.
    // 2. If playing, `player.current_time += time.delta_seconds_f64() * player.speed`.
    // 3. Check if `current_time` exceeds the duration and handle `PlaybackMode`.
    // 4. For ping-pong, reverse `player.speed`.
    // 5. For `Once`, set state to `Ended` and fire event.
}
```

### `accumulate_animation_values_system`

```rust
/// The core animation sampling logic. This system iterates through all active animation instances,
/// calculates their local time, samples the animation data using the `InterpolationRegistry`,
/// and accumulates the weighted results into the `FrameBlendData` resource for later blending.
fn accumulate_animation_values_system(
    instance_query: Query<(&Parent, &AnimationInstance, &AnimationBinding)>,
    player_query: Query<&AnimationPlayer>,
    animations: Res<Assets<AnimationData>>,
    baked_animations: Res<Assets<BakedAnimationData>>,
    mut interpolation_registry: ResMut<InterpolationRegistry>,
    mut blend_data: Local<FrameBlendData>,
) {
    // Implementation details:
    // 1. Iterate through instances.
    // 2. Get parent player's `current_time`.
    // 3. Calculate instance's local time: `(player_time - instance.start_time) * instance.time_scale`.
    // 4. For each track, use the `AnimationBinding` to get the target Entity and BevyPath.
    // 5. Sample the animation data at the local time to get a `Value`.
    // 6. Push `(instance.weight, value)` to the `blend_data` map.
}
```

### `blend_and_apply_animation_values_system`

```rust
use bevy::reflect::ReflectMut;

/// Processes the `FrameBlendData` resource to blend accumulated values and apply them to the
/// target components. It uses `bevy_reflect` to dynamically get a mutable reference to the
/// target component and apply the final blended value, making the system highly extensible.
fn blend_and_apply_animation_values_system(
    mut blend_data: Local<FrameBlendData>,
    world: &mut World,
) {
    // Implementation details:
    // 1. Iterate through `blend_data.blended_values`.
    // 2. For each `(entity, path)`, blend the `Vec<(weight, value)>` into a single `Value`.
    //    - Use weighted average for most types.
    //    - Use NLERP for quaternions.
    // 3. Get the target component using `world.get_entity_mut(entity).unwrap().get_mut::<C>()`.
    // 4. Use `path.apply(&mut *component, &final_value)` to set the value.
    // 5. Clear the `blend_data` map.
}
```

### `collect_animation_output_system`

```rust
/// The final system in the animation pipeline, responsible for populating the `AnimationOutput` resource.
/// It reads the final state of all animated components for the frame and structures the data
/// in the format expected by the external Wasm API, ensuring a clean separation between the
/// internal ECS state and the external interface.
fn collect_animation_output_system(
    mut output: ResMut<AnimationOutput>,
    player_query: Query<(Entity, &AnimationPlayer)>,
    // ... other queries to get final component values ...
) {
    // Implementation details:
    // 1. Clear the `output` resource.
    // 2. Iterate through all players.
    // 3. For each player, iterate through its known animated targets (could be cached on the player).
    // 4. Read the final value from the target's component (e.g., `Transform`, `AnimatedColor`).
    // 5. Populate the `output.values` map with the `player_id -> target_path -> value` structure.
}
```

## 5. Plugin

The plugin brings everything together.

### `AnimationPlayerPlugin`

```rust
/// The main Bevy plugin for the animation player.
/// It registers all necessary components, assets, and resources, and adds the animation
/// systems to the `Update` schedule in the correct order using `.chain()` to ensure
/// a deterministic and correct data flow from one system to the next.
pub struct AnimationPlayerPlugin;

impl Plugin for AnimationPlayerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AnimationOutput>()
            .init_resource::<InterpolationRegistry>()
            // Register assets and their reflection data
            .register_asset_reflect::<AnimationData>()
            .register_asset_reflect::<BakedAnimationData>()
            // Register components for reflection
            .register_type::<AnimationPlayer>()
            .register_type::<AnimationInstance>()
            .register_type::<AnimationBinding>()
            .register_type::<AnimatedColor>()
            .register_type::<Intensity>()
            // Add systems in a defined order
            .add_systems(Update,
                (
                    bind_new_animation_instances_system,
                    update_animation_players_system,
                    accumulate_animation_values_system,
                    blend_and_apply_animation_values_system,
                    collect_animation_output_system,
                ).chain()
            );
    }
}
```
