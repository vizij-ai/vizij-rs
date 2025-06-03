# WasmAnimationEngine Documentation

## Overview

The WasmAnimationEngine is a WebAssembly (WASM) module that provides high-performance animation playback and interpolation capabilities for web applications. It wraps the core Rust animation engine and exposes a JavaScript-friendly API for real-time animation processing.

## Architecture

### Core Structure

```
WasmAnimationEngine
├── AnimationEngine (Rust core)
├── JavaScript Bindings (wasm-bindgen)
├── Memory Management (Arc<Mutex<>>)
└── JSON Serialization Interface
```

The module is built around a thread-safe wrapper (`Arc<Mutex<AnimationEngine>>`) that allows safe concurrent access to the animation engine from JavaScript.

### Key Components

1. **Animation Engine**: Core Rust engine handling animation data, players, and interpolation
2. **WASM Bindings**: JavaScript-compatible interface using wasm-bindgen
3. **JSON Interface**: Serialization/deserialization for data exchange
4. **Console Integration**: Logging capabilities for debugging
5. **Performance Monitoring**: Built-in metrics and performance tracking

## Core Functionality

### 1. Engine Management

#### `new(config_json?: string): WasmAnimationEngine`
Creates a new animation engine instance with optional configuration.

**Parameters:**
- `config_json` (optional): JSON string containing AnimationConfig
- If not provided, uses web-optimized default configuration

**Features:**
- Automatic panic hook setup for better error reporting
- Web-optimized default settings (32MB memory, 60 FPS target)
- Configurable performance thresholds and limits

### 2. Animation Data Management

#### `load_animation(animation_json: string): void`
Loads animation data from JSON format with fallback parsing.

**Features:**
- Direct JSON parsing with fallback to test animation loader
- Comprehensive error handling and logging
- Support for complex animation structures with tracks, keypoints, and transitions

#### `export_animation(animation_id: string): string`
Exports animation data as JSON string for persistence or transfer.

### 3. Player Management

#### `create_player(player_id: string): void`
Creates a new animation player instance.

#### `add_instance(player_id: string, animation_id: string): instance_id: string`
Adds an animation instance to a player with automatic duration calculation.

### 4. Playback Control

#### `play(player_id: string): void`
Starts animation playback for the specified player.

#### `pause(player_id: string): void`
Pauses animation playback, maintaining current position.

#### `stop(player_id: string): void`
Stops animation playback and resets to beginning.

#### `seek(player_id: string, time_seconds: f64): void`
Seeks to a specific time position in the animation.

### 5. Real-time Updates

#### `update(frame_delta_seconds: f64): JsValue`
Updates the animation engine and returns current interpolated values.

**Returns:**
- Nested HashMap structure: `{player_id: {instance_id: {track_target: value}}}`
- JSON-serialized for JavaScript consumption
- Includes all active animations and their current interpolated values

### 6. State Queries

#### `get_player_state(player_id: string): string`
Returns the current playback state as JSON.

#### `get_player_time(player_id: string): f64`
Gets the current animation time in seconds.

#### `get_player_progress(player_id: string): f64`
Returns playback progress as a value between 0.0 and 1.0.

#### `get_player_ids(): string[]`
Lists all available player IDs.

### 7. Configuration Management

#### `update_player_config(player_id: string, config_json: string): void`
Updates player configuration with validation.

**Supported Settings:**
- `speed`: Playback speed multiplier (-5.0 to 5.0)
- `mode`: Playback mode ("once", "loop", "ping_pong")
- `start_time`: Animation start time (seconds, ≥ 0.0)
- `end_time`: Animation end time (seconds, ≥ 0.0 or null)
- Legacy support for boolean `loop` and `ping_pong` flags

### 8. Advanced Features

#### `get_derivatives(player_id: string, derivative_width_ms?: f64): JsValue`
Calculates rates of change for all animation tracks.

**Parameters:**
- `derivative_width_ms` (optional): Time window for derivative calculation
- Returns derivatives as JSON-serialized values

#### `bake_animation(animation_id: string, config_json?: string): string`
Pre-calculates animation values at specified frame rates.

**Features:**
- Configurable frame rate and time ranges
- Optional derivative calculation
- Memory usage estimation
- Statistics about baked data

### 9. Performance Monitoring

#### `get_metrics(): JsValue`
Returns comprehensive performance metrics including:
- Frame rate statistics
- Memory usage
- Interpolation performance
- Player statistics

## Data Structures

### Animation Data Format

```json
{
  "id": "animation_id",
  "name": "Animation Name",
  "tracks": {
    "track_id": {
      "name": "track_name",
      "target": "property.path",
      "enabled": true,
      "weight": 1.0,
      "keypoints": [
        {
          "time": 0.0,
          "value": { "type": "Float", "value": 1.0 }
        }
      ]
    }
  },
  "transitions": [
    {
      "from_keypoint": "keypoint_id_1",
      "to_keypoint": "keypoint_id_2",
      "variant": "Linear"
    }
  ],
  "metadata": {
    "duration": 5.0,
    "author": "Author Name",
    "description": "Animation description"
  }
}
```

### Configuration Format

```json
{
  "speed": 1.0,
  "mode": "loop",
  "start_time": 0.0,
  "end_time": null
}
```

### Value Types

The engine supports multiple value types:
- `Float`: Single floating-point numbers
- `Vector3`: 3D vectors (position, scale)
- `Vector4`: 4D vectors (quaternion rotation)
- `Color`: RGBA color values
- `Transform`: Combined position, rotation, scale

## Performance Characteristics

### Memory Management

- Web-optimized default: 32MB limit
- Automatic memory tracking and limits
- Efficient Arc<Mutex<>> for thread safety
- Optional wee_alloc for smaller WASM size

### Interpolation Performance

- Cached interpolation functions
- Configurable performance thresholds
- Real-time performance monitoring
- Optimized for 60 FPS target on web

### Concurrency

- Thread-safe design with Mutex protection
- Non-blocking updates where possible
- Error isolation prevents engine corruption

## Testing and Development

### Built-in Test Functions

#### `create_test_animation(): string`
Creates a comprehensive test animation with multiple transition types.

#### `create_animation_test_type(): string`
Creates an animation showcasing different value types.

#### `greet(name: string): string`
Simple greeting function for WASM connectivity testing.

### Console Integration

- `console_log(message: string)`: Direct browser console logging
- Automatic error logging with context
- Performance warning notifications


## Conclusion

The WasmAnimationEngine provides a robust foundation for web-based animation systems with excellent performance characteristics and a comprehensive feature set. 



