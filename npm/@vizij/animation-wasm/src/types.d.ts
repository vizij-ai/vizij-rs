import type { NormalizedValue, ValueJSON as SharedValueJSON } from "@vizij/value-json";

/**
 * Public TypeScript types for @vizij/animation-wasm
 * Mirrors vizij-animation-core JSON contracts exposed via wasm-bindgen.
 */

/* -----------------------------------------------------------
   WASM init input (parity with node-graph)
----------------------------------------------------------- */
export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

/* -----------------------------------------------------------
   Basic IDs
----------------------------------------------------------- */
/** Engine-managed animation identifier. */
export type AnimId = number;
/** Engine-managed player identifier. */
export type PlayerId = number;
/** Engine-managed instance identifier. */
export type InstId = number;

/* -----------------------------------------------------------
   Engine Config (vizij-animation-core/src/config.rs)
----------------------------------------------------------- */
export interface Features {
  /** Reserved for future toggles (SIMD, parallel, etc.). */
  reserved0?: boolean;
}

export interface Config {
  /** Initial capacity hints for scratch/sample buffers. */
  scratch_samples?: number;
  scratch_values_scalar?: number;
  scratch_values_vec?: number;
  scratch_values_quat?: number;

  /** Maximum events to retain per tick before backpressure policy applies. */
  max_events_per_tick?: number;

  /** Feature flags. */
  features?: Features;
}

/* -----------------------------------------------------------
   Baking (vizij-animation-core/src/baking.rs)
----------------------------------------------------------- */
export interface BakingConfig {
  /** Target frame rate (Hz) for baked samples. */
  frame_rate?: number;
  /** Start time (seconds) in clip space. */
  start_time?: number;
  /** End time (seconds) in clip space; omit or null to use full duration. */
  end_time?: number | null;
  /** Optional finite-difference epsilon override for derivative estimation. */
  derivative_epsilon?: number;
}

/** Baked values for a single target path. */
export interface BakedTrack {
  /** Typed path for the track being sampled. */
  target_path: string;
  /** Sampled values in time order. */
  values: Value[];
}

/** Baked clip values at a uniform sampling rate. */
export interface BakedAnimationData {
  /** Animation id that was baked. */
  anim: AnimId;
  /** Frame rate used for baking (Hz). */
  frame_rate: number;
  /** Clip start time. */
  start_time: number;
  /** Clip end time. */
  end_time: number;
  /** Tracks emitted for each target path. */
  tracks: BakedTrack[];
}

/* -----------------------------------------------------------
   Inputs (vizij-animation-core/src/inputs.rs)
   serde default represents enums as { "Variant": { ... } }
----------------------------------------------------------- */
/** Playback looping mode for a player. */
export type LoopMode = "Once" | "Loop" | "PingPong";

/** Command applied to a player before stepping. */
export type PlayerCommand =
  | { Play: { player: PlayerId } }
  | { Pause: { player: PlayerId } }
  | { Stop: { player: PlayerId } }
  | { SetSpeed: { player: PlayerId; speed: number } }
  | { Seek: { player: PlayerId; time: number } }
  | { SetLoopMode: { player: PlayerId; mode: LoopMode } }
  | { SetWindow: { player: PlayerId; start_time: number; end_time?: number | null } };

/** Per-instance configuration updates applied before stepping. */
export interface InstanceUpdate {
  /** Target player id. */
  player: PlayerId;
  /** Target instance id. */
  inst: InstId;
  /** Blend weight for the instance (0..1). */
  weight?: number;
  /** Playback time scaling factor. */
  time_scale?: number;
  /** Clip start offset applied to instance time. */
  start_offset?: number;
  /** Enable/disable the instance. */
  enabled?: boolean;
}

/** Inputs batch applied on the next engine tick. */
export interface Inputs {
  /** Player-level commands applied before stepping. */
  player_cmds?: PlayerCommand[];
  /** Instance-level updates applied before stepping. */
  instance_updates?: InstanceUpdate[];
}

/* -----------------------------------------------------------
   Values (vizij-animation-core/src/value.rs)
   Tagged union: { type: "...", data: ... }
----------------------------------------------------------- */
/** Value JSON union accepted by the engine wrapper. */
export type ValueJSON = SharedValueJSON;
/** Normalized Value payloads returned by the engine. */
export type Value = NormalizedValue;

/* -----------------------------------------------------------
   Outputs (vizij-animation-core/src/outputs.rs)
----------------------------------------------------------- */
/** A resolved output change emitted by the engine. */
export interface Change {
  /** Player that emitted the change. */
  player: PlayerId;
  /** Opaque key (resolved via prebind or canonical path when unresolved). */
  key: string;
  /** Value payload after sampling. */
  value: Value;
}

/** Change record including optional finite-difference derivative. */
export interface ChangeWithDerivative extends Change {
  derivative?: Value | null;
}

/** Playback and diagnostic events emitted by the core engine. */
export type CoreEvent =
  | { PlaybackStarted: { player: PlayerId; animation?: string | null } }
  | { PlaybackPaused: { player: PlayerId } }
  | { PlaybackStopped: { player: PlayerId } }
  | { PlaybackResumed: { player: PlayerId } }
  | { PlaybackEnded: { player: PlayerId; animation_time: number } }
  | { TimeChanged: { player: PlayerId; old_time: number; new_time: number } }
  | {
      KeypointReached: {
        player: PlayerId;
        track_path: string;
        key_index: number;
        value: Value;
        animation_time: number;
      };
    }
  | {
      PerformanceWarning: {
        metric: string;
        value: number;
        threshold: number;
      };
    }
  | { Error: { message: string } }
  | { Custom: { kind: string; data: unknown } };

/** Output payload returned by updateValues(). */
export interface Outputs {
  /** All resolved output changes for this tick. */
  changes: Change[];
  /** Playback and diagnostic events emitted by the engine. */
  events: CoreEvent[];
}

/** Output payload returned by updateValuesAndDerivatives(). */
export interface OutputsWithDerivatives {
  /** Output changes with derivative estimates (if enabled). */
  changes: ChangeWithDerivative[];
  /** Playback and diagnostic events emitted by the engine. */
  events: CoreEvent[];
}

/* -----------------------------------------------------------
   StoredAnimation (new JSON format) — minimal typing
   See vizij-spec/Animation.md and fixtures/animations/vector-pose-combo.json
----------------------------------------------------------- */
/** Stored animation vector value. */
export type StoredScalarVec2 = { x: number; y: number };
/** Stored animation vector value. */
export type StoredScalarVec3 = { x: number; y: number; z: number };
/** Stored animation Euler rotation (roll/pitch/yaw). */
export type StoredEulerRPY = { r: number; p: number; y: number };
/** Stored animation color in RGB. */
export type StoredColorRGB = { r: number; g: number; b: number };
/** Stored animation color in HSL. */
export type StoredColorHSL = { h: number; s: number; l: number };

/** Union of stored value payloads accepted by StoredAnimation. */
export type StoredValue =
  | number
  | StoredScalarVec2
  | StoredScalarVec3
  | StoredEulerRPY
  | StoredColorRGB
  | StoredColorHSL
  | boolean
  | string;

/** Cubic-bezier control point (0..1 range). */
export interface BezierCP {
  /** Normalized time coordinate (0..1) for the control point. */
  x: number;
  /** Normalized value coordinate (0..1) for the control point. */
  y: number;
}

/** Keypoint sample within a track. */
export interface Keypoint {
  id: string;
  /** Normalized stamp [0..1] within the track. */
  stamp: number;
  /** Stored value at this keyframe. */
  value: StoredValue;
  /** Optional per-keyframe transition handles. */
  transitions?: {
    in?: BezierCP;
    out?: BezierCP;
  };
}

/** Animation track targeting a canonical path. */
export interface Track {
  id: string;
  name?: string;
  /** Canonical target path (e.g., "node/Transform.translation"). */
  animatableId: string;
  /** Keyframes for this track. */
  points: Keypoint[];
  /** Optional UI hints for editors. */
  settings?: { color?: string };
}

/** Stored animation clip data. */
export interface StoredAnimation {
  id?: string;
  name?: string;
  /** Duration in milliseconds. */
  duration: number;
  /** Track list with paths, interpolation modes, and values. */
  tracks: Track[];
  /** Optional grouping metadata. */
  groups?: Record<string, unknown>;
}

/* -----------------------------------------------------------
   AnimationData (engine-internal JSON format)
   Left intentionally broad; use when supplying core-format clips.
----------------------------------------------------------- */
/** Core-format animation payload; leave as `unknown` for loose typing. */
export type AnimationData = unknown;

/* -----------------------------------------------------------
   Baked animation data
----------------------------------------------------------- */
/** Baked values for a single target path. */
export interface BakedTrack {
  /** Typed path for the baked output track. */
  target_path: string;
  /** Sampled values in time order. */
  values: Value[];
}

/** Baked derivatives for a single target path. */
export interface BakedDerivativeTrack {
  /** Typed path for the baked output track. */
  target_path: string;
  /** Sampled values in time order. */
  values: Array<Value | null>;
}

/** Baked clip values at a uniform sampling rate. */
export interface BakedAnimationData {
  /** Animation id that was baked. */
  anim: AnimId;
  /** Frame rate used for baking (Hz). */
  frame_rate: number;
  /** Clip start time. */
  start_time: number;
  /** Clip end time. */
  end_time: number;
  /** Tracks emitted for each target path. */
  tracks: BakedTrack[];
}

/** Baked clip derivatives at a uniform sampling rate. */
export interface BakedDerivativeAnimationData {
  /** Animation id that was baked. */
  anim: AnimId;
  /** Frame rate used for baking (Hz). */
  frame_rate: number;
  /** Clip start time. */
  start_time: number;
  /** Clip end time. */
  end_time: number;
  /** Tracks emitted for each target path. */
  tracks: BakedDerivativeTrack[];
}

/** Pair of baked values and derivatives produced by the engine. */
export interface BakedAnimationBundle {
  /** Baked values without derivatives. */
  values: BakedAnimationData;
  /** Baked values and derivatives. */
  derivatives: BakedDerivativeAnimationData;
}

/* -----------------------------------------------------------
   Engine inspection (authoritative state from core)
----------------------------------------------------------- */

/** Metadata describing a loaded animation. */
export interface AnimationInfo {
  /** Animation id. */
  id: number;
  /** Optional display name. */
  name?: string;
  /** Duration in milliseconds. */
  duration_ms: number;
  /** Number of tracks in the animation. */
  track_count: number;
}

/** Current playback state for a player. */
export type PlaybackState = "Playing" | "Paused" | "Stopped";

/** Default instance configuration used when attaching animations. */
export interface InstanceCfg {
  /** Blend weight for the instance (0..1). */
  weight: number;
  /** Playback time scaling factor. */
  time_scale: number;
  /** Clip start offset applied to instance time. */
  start_offset: number;
  /** Enable/disable the instance. */
  enabled: boolean;
}

/** Snapshot of a live animation instance. */
export interface InstanceInfo {
  /** Instance id. */
  id: number;
  /** Animation id assigned to the instance. */
  animation: number;
  /** Current instance configuration. */
  cfg: InstanceCfg;
}

/** Snapshot of a player, including current timing. */
export interface PlayerInfo {
  /** Player id. */
  id: number;
  /** Human-readable name. */
  name: string;
  /** Current playback state. */
  state: PlaybackState;
  /** Current playback time in seconds. */
  time: number; // seconds
  /** Playback speed multiplier. */
  speed: number;
  /** Current looping mode. */
  loop_mode: LoopMode;
  /** Playback window start in seconds. */
  start_time: number; // seconds
  /** Playback window end in seconds (optional). */
  end_time?: number | null; // seconds or null/undefined
  /** Total playback window length in seconds. */
  length: number; // seconds (computed: max over instances of start_offset + anim_duration/|time_scale|)
}
