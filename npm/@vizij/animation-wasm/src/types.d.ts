import type { NormalizedValue, ValueJSON as SharedValueJSON } from "@vizij/value-json";

/**
 * Public TypeScript types for @vizij/animation-wasm
 * Mirrors vizij-animation-core JSON contracts exposed via wasm-bindgen.
 */

/* -----------------------------------------------------------
   WASM init input (parity with node-graph)
----------------------------------------------------------- */
/**
 * Source accepted by the wasm-pack generated initializer.
 *
 * In browsers this is typically a URL, RequestInfo, or Response. Bundled/server-side hosts may
 * also provide a `BufferSource` or precompiled `WebAssembly.Module`.
 */
export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

/* -----------------------------------------------------------
   Basic IDs
----------------------------------------------------------- */
export type AnimId = number;
export type PlayerId = number;
export type InstId = number;

/* -----------------------------------------------------------
   Engine Config (vizij-animation-core/src/config.rs)
----------------------------------------------------------- */
export interface Features {
  /** Reserved for future toggles (SIMD, parallel, etc.) */
  reserved0?: boolean;
}

/** Engine construction options forwarded to the wasm animation engine. */
export interface Config {
  /** Initial capacity hints for scratch/sample buffers */
  scratch_samples?: number;
  scratch_values_scalar?: number;
  scratch_values_vec?: number;
  scratch_values_quat?: number;

  /** Maximum events to retain per tick before backpressure policy applies */
  max_events_per_tick?: number;

  /** Feature flags */
  features?: Features;
}

/* -----------------------------------------------------------
   Baking (vizij-animation-core/src/baking.rs)
----------------------------------------------------------- */
/** Options controlling baked sample extraction from an animation clip. */
export interface BakingConfig {
  /** Target frame rate (Hz) for baked samples */
  frame_rate?: number;
  /** Start time (seconds) in clip space */
  start_time?: number;
  /** End time (seconds) in clip space; omit or null to use full duration */
  end_time?: number | null;
  /** Optional finite-difference epsilon override for derivative estimation */
  derivative_epsilon?: number;
}

export interface BakedTrack {
  target_path: string;
  values: Value[];
}

export interface BakedAnimationData {
  anim: AnimId;
  frame_rate: number;
  start_time: number;
  end_time: number;
  tracks: BakedTrack[];
}

/* -----------------------------------------------------------
   Inputs (vizij-animation-core/src/inputs.rs)
   serde default represents enums as { "Variant": { ... } }
----------------------------------------------------------- */
export type LoopMode = "Once" | "Loop" | "PingPong";

/** Player-level command bag applied before the engine advances time for a tick. */
export type PlayerCommand =
  | { Play: { player: PlayerId } }
  | { Pause: { player: PlayerId } }
  | { Stop: { player: PlayerId } }
  | { SetSpeed: { player: PlayerId; speed: number } }
  | { Seek: { player: PlayerId; time: number } }
  | { SetLoopMode: { player: PlayerId; mode: LoopMode } }
  /** `end_time: null` clears the explicit end bound and reuses the player's natural end. */
  | { SetWindow: { player: PlayerId; start_time: number; end_time?: number | null } };

/** Partial instance update applied before stepping. Omitted fields leave the current value unchanged. */
export interface InstanceUpdate {
  player: PlayerId;
  inst: InstId;
  weight?: number;
  time_scale?: number;
  start_offset?: number;
  enabled?: boolean;
}

/** Input payload accepted by `Engine.update*()` for one tick. */
export interface Inputs {
  /** Player-level commands applied first, before instance updates and time advancement. */
  player_cmds?: PlayerCommand[];
  /** Instance-level updates applied after `player_cmds` and before sampling. */
  instance_updates?: InstanceUpdate[];
}

/* -----------------------------------------------------------
   Values (vizij-animation-core/src/value.rs)
   Tagged union: { type: "...", data: ... }
----------------------------------------------------------- */
export type ValueJSON = SharedValueJSON;
export type Value = NormalizedValue;

/* -----------------------------------------------------------
   Outputs (vizij-animation-core/src/outputs.rs)
----------------------------------------------------------- */
/** One emitted target change for a player during a tick. */
export interface Change {
  player: PlayerId;
  /** Opaque key (resolved via prebind or canonical path when unresolved) */
  key: string;
  value: Value;
}

/** Change paired with an optional derivative sample. */
export interface ChangeWithDerivative extends Change {
  /** Present only when derivative-aware sampling/output is requested. */
  derivative?: Value | null;
}

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

export interface Outputs {
  /** Value changes produced during this tick. */
  changes: Change[];
  /** Semantic playback/events emitted during this tick. */
  events: CoreEvent[];
}

export interface OutputsWithDerivatives {
  changes: ChangeWithDerivative[];
  events: CoreEvent[];
}

/* -----------------------------------------------------------
   StoredAnimation (new JSON format) — minimal typing
   See vizij-spec/Animation.md and fixtures/animations/vector-pose-combo.json
----------------------------------------------------------- */
export type StoredScalarVec2 = { x: number; y: number };
export type StoredScalarVec3 = { x: number; y: number; z: number };
export type StoredEulerRPY = { r: number; p: number; y: number };
export type StoredColorRGB = { r: number; g: number; b: number };
export type StoredColorHSL = { h: number; s: number; l: number };

export type StoredValue =
  | number
  | StoredScalarVec2
  | StoredScalarVec3
  | StoredEulerRPY
  | StoredColorRGB
  | StoredColorHSL
  | boolean
  | string;

export interface BezierCP {
  x: number;
  y: number;
}

export interface Keypoint {
  id: string;
  /** Normalized stamp [0..1] within the track */
  stamp: number;
  value: StoredValue;
  transitions?: {
    in?: BezierCP;
    out?: BezierCP;
  };
}

export interface Track {
  id: string;
  name?: string;
  /** Canonical target path (e.g., "node/Transform.translation") */
  animatableId: string;
  /** Keypoints ordered in normalized clip space. */
  points: Keypoint[];
  settings?: { color?: string };
}

export interface StoredAnimation {
  id?: string;
  name?: string;
  /** Duration in milliseconds */
  duration: number;
  /** Tracks sampled across the normalized `[0, 1]` clip domain. */
  tracks: Track[];
  /** Optional grouping metadata */
  groups?: Record<string, unknown>;
}

/* -----------------------------------------------------------
   AnimationData (engine-internal JSON format)
   Left intentionally broad; use when supplying core-format clips.
----------------------------------------------------------- */
export type AnimationData = unknown;

/* -----------------------------------------------------------
   Baked animation data
----------------------------------------------------------- */
export interface BakedTrack {
  target_path: string;
  values: Value[];
}

export interface BakedDerivativeTrack {
  target_path: string;
  /** Null means no stable derivative sample was emitted for that frame. */
  values: Array<Value | null>;
}

export interface BakedAnimationData {
  anim: AnimId;
  frame_rate: number;
  /** Clip-space start time in seconds. */
  start_time: number;
  /** Clip-space end time in seconds. */
  end_time: number;
  tracks: BakedTrack[];
}

export interface BakedDerivativeAnimationData {
  anim: AnimId;
  frame_rate: number;
  /** Clip-space start time in seconds. */
  start_time: number;
  /** Clip-space end time in seconds. */
  end_time: number;
  tracks: BakedDerivativeTrack[];
}

export interface BakedAnimationBundle {
  values: BakedAnimationData;
  derivatives: BakedDerivativeAnimationData;
}

/* -----------------------------------------------------------
   Engine inspection (authoritative state from core)
----------------------------------------------------------- */

export interface AnimationInfo {
  id: number;
  name?: string;
  /** Source animation duration in milliseconds. */
  duration_ms: number;
  track_count: number;
}

export type PlaybackState = "Playing" | "Paused" | "Stopped";

export interface InstanceCfg {
  /** Blend weight applied to the instance contribution. */
  weight: number;
  /** Playback scaling factor used by the engine's local-time mapping. */
  time_scale: number;
  /** Start offset in seconds on the player timeline. */
  start_offset: number;
  enabled: boolean;
}

/** Snapshot of one registered animation instance. */
export interface InstanceInfo {
  id: number;
  animation: number;
  cfg: InstanceCfg;
}

/** Snapshot of one player after stepping or inspection. */
export interface PlayerInfo {
  id: number;
  name: string;
  state: PlaybackState;
  time: number; // seconds
  speed: number;
  loop_mode: LoopMode;
  start_time: number; // seconds
  end_time?: number | null; // seconds or null/undefined
  length: number; // seconds (computed: max over instances of start_offset + anim_duration/|time_scale|)
}
import type { NormalizedValue, ValueJSON as SharedValueJSON } from "@vizij/value-json";
