# WasmAnimationEngine Documentation

## Overview

The `WasmAnimationEngine` is a WebAssembly (WASM) module that provides a high-performance animation system for web applications. It wraps the core Rust-based animation engine, offering a rich feature set for loading, controlling, and querying animations through a clean JavaScript interface.

## Architecture

The module exposes the `WasmAnimationEngine` class in JavaScript, which serves as the primary entry point for all animation-related operations.

### Key Components

1.  **Core Animation Engine**: The underlying Rust engine that handles data structures, state management, interpolation, and the main update loop.
2.  **WASM Bindings**: A JavaScript-compatible API generated using `wasm-bindgen`, which marshals data and calls between JS and Rust.
3.  **JSON Interface**: Data, configurations, and results are serialized to and from JSON for easy interoperability.
4.  **Hierarchical State Model**:
    -   **Engine**: Manages global resources (`AnimationData`) and `AnimationPlayer`s.
    -   **Player**: Manages a timeline and a set of `AnimationInstance`s.
    -   **Instance**: Represents a single animation clip with its own playback settings.

## Core API

### 1. Engine Management

#### `new(config_json?: string): WasmAnimationEngine`

Creates a new animation engine instance.

-   **`config_json`** (optional): A JSON string with `AnimationEngineConfig` to customize engine behavior (e.g., memory limits, performance thresholds). If omitted, web-optimized defaults are used.
-   **Features**: Automatically sets up a panic hook for clear error reporting in the browser console.

### 2. Animation Data Management

#### `load_animation(animation_json: string): string`

Loads animation data from a JSON string into the engine.

-   **Returns**: A unique `animation_id` (string) used to reference this data later.
-   **Features**: Supports complex animation structures with tracks, keypoints, transitions, and metadata.

#### `unload_animation(animation_id: string): void`

Removes previously loaded animation data from the engine to free up memory.

#### `get_animation_data(animation_id: string): string`

Exports a loaded animation's data as a JSON string.

### 3. Player Management

#### `create_player(): string`

Creates a new `AnimationPlayer` instance.

-   **Returns**: A unique `player_id` (string) for managing the player.

#### `remove_player(player_id: string): void`

Removes a player and all its associated instances.

#### `get_player_ids(): string[]`

Returns an array of all active player IDs.

### 4. Instance Management

#### `add_instance(player_id: string, animation_id: string, settings_json?: string): string`

Adds an animation instance to a player. This is the primary way to make an animation "playable."

-   **`settings_json`** (optional): A JSON string with `AnimationInstanceSettings` to control the instance's behavior (timescale, looping, etc.).
-   **Returns**: A unique `instance_id` (string) for this specific instance.

#### `remove_instance(player_id: string, instance_id: string): void`

Removes a specific animation instance from a player.

#### `get_instance_ids(player_id: string): string[]`

Returns an array of all instance IDs associated with a player.

### 5. Playback Control

#### `play(player_id: string): void`

Starts or resumes playback for the specified player.

#### `pause(player_id: string): void`

Pauses playback, maintaining the player's current time.

#### `stop(player_id: string): void`

Stops playback and resets the player's time to its start time.

#### `seek(player_id: string, time_seconds: f64): void`

Jumps the player's timeline to a specific time.

### 6. Real-time Updates

#### `update(frame_delta_seconds: f64): JsValue`

Advances the animation engine by a time delta (typically the time since the last frame). This is the core method that drives all animations.

-   **Returns**: A `JsValue` (JavaScript object) containing the current interpolated values for all active players, structured as:
    `{ [player_id]: { [track_target]: Value } }`

### 7. State & Configuration Queries

#### `get_player_state(player_id: string): string`

Returns a JSON string of the player's current `PlayerState`, including its playback status (`playing`, `paused`, etc.).

#### `get_player_time(player_id: string): f64`

Gets the player's current time in seconds.

#### `get_player_progress(player_id: string): f64`

Returns the player's progress as a value from `0.0` to `1.0`.

#### `update_player_config(player_id: string, config_json: string): void`

Updates the runtime configuration of a player. See the `PlayerSettings` data structure for available fields.

#### `update_instance_config(player_id: string, instance_id: string, settings_json: string): void`

Updates the settings for a specific animation instance at runtime. See the `AnimationInstanceSettings` data structure for available fields.

### 8. Advanced Features

#### `get_derivatives(player_id: string, derivative_width_ms?: f64): JsValue`

Calculates the rate of change (derivative) for all animated values in a player. Useful for effects like motion blur or matching velocity.

-   **Returns**: A `JsValue` object mapping track targets to their derivative `Value`.

#### `bake_animation(animation_id: string, config_json?: string): string`

Pre-calculates animation values at a fixed frame rate ("bakes" the animation). This can improve performance for complex animations by replacing real-time interpolation with a simple lookup.

-   **Returns**: A JSON string of the `BakedAnimationData`.

## Data Structures

### AnimationData

The core data model for an animation clip.

```json
{
  "id": "animation_id",
  "name": "Animation Name",
  "metadata": {
    "duration": 5.0,
    "author": "Author Name",
    "version": "1.0.0"
  },
  "tracks": {
    "track_uuid": {
      "id": "track_uuid",
      "name": "track_name",
      "target": "property.path",
      "enabled": true,
      "weight": 1.0,
      "keypoints": [
        {
          "id": "keypoint_uuid",
          "time": 0.0,
          "value": { "type": "Float", "value": 1.0 }
        }
      ]
    }
  },
  "transitions": [
    {
      "id": "transition_uuid",
      "keypoints": ["keypoint_uuid_1", "keypoint_uuid_2"],
      "variant": "Cubic",
      "parameters": { "tension": 0.5 }
    }
  ]
}
```

### PlayerSettings (Player Configuration)

Used with `update_player_config`.

```json
{
  "speed": 1.0,
  "mode": "Loop", // "Once", "Loop", "PingPong"
  "loop_until_target": null,
  "start_time": 0.0,
  "end_time": null // or a time in seconds
}
```

### AnimationInstanceSettings

Used with `add_instance` and `update_instance_config`.

```json
{
  "instance_start_time": 0.0, // Offset on the player's timeline
  "timescale": 1.0,
  "enabled": true
}
```

### Value Types

The engine supports multiple value types for animation:

-   `Float`: Single floating-point number.
-   `Vector2`, `Vector3`, `Vector4`: For positions, scales, quaternions, etc.
-   `Color`: RGBA color values.
-   `Transform`: A combination of position (`Vector3`), rotation (`Vector4`), and scale (`Vector3`).
