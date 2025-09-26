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
export interface BakingConfig {
  /** Target frame rate (Hz) for baked samples */
  frame_rate?: number;
  /** Start time (seconds) in clip space */
  start_time?: number;
  /** End time (seconds) in clip space; omit or null to use full duration */
  end_time?: number | null;
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

export type PlayerCommand =
  | { Play: { player: PlayerId } }
  | { Pause: { player: PlayerId } }
  | { Stop: { player: PlayerId } }
  | { SetSpeed: { player: PlayerId; speed: number } }
  | { Seek: { player: PlayerId; time: number } }
  | { SetLoopMode: { player: PlayerId; mode: LoopMode } }
  | { SetWindow: { player: PlayerId; start_time: number; end_time?: number | null } };

export interface InstanceUpdate {
  player: PlayerId;
  inst: InstId;
  weight?: number;
  time_scale?: number;
  start_offset?: number;
  enabled?: boolean;
}

export interface Inputs {
  /** Player-level commands applied before stepping */
  player_cmds?: PlayerCommand[];
  /** Instance-level updates applied before stepping */
  instance_updates?: InstanceUpdate[];
}

/* -----------------------------------------------------------
   Values (vizij-animation-core/src/value.rs)
   Tagged union: { type: "...", data: ... }
----------------------------------------------------------- */
export type Value =
  | { type: "Float"; data: number }
  | { type: "Vec2"; data: [number, number] }
  | { type: "Vec3"; data: [number, number, number] }
  | { type: "Vec4"; data: [number, number, number, number] }
  | { type: "Quat"; data: [number, number, number, number] } // (x, y, z, w)
  | { type: "ColorRgba"; data: [number, number, number, number] } // RGBA
  | {
      type: "Transform";
      data: {
        pos: [number, number, number];
        rot: [number, number, number, number]; // quat (x,y,z,w)
        scale: [number, number, number];
      };
    }
  | { type: "Bool"; data: boolean }
  | { type: "Text"; data: string };

/* -----------------------------------------------------------
   Outputs (vizij-animation-core/src/outputs.rs)
----------------------------------------------------------- */
export interface Change {
  player: PlayerId;
  /** Opaque key (resolved via prebind or canonical path when unresolved) */
  key: string;
  value: Value;
  derivative: Value;
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
  changes: Change[];
  events: CoreEvent[];
}

/* -----------------------------------------------------------
   StoredAnimation (new JSON format) — minimal typing
   See vizij-spec/Animation.md and test_fixtures/new_format.json
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
  points: Keypoint[];
  settings?: { color?: string };
}

export interface StoredAnimation {
  id?: string;
  name?: string;
  /** Duration in milliseconds */
  duration: number;
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
   Engine inspection (authoritative state from core)
----------------------------------------------------------- */

export interface AnimationInfo {
  id: number;
  name?: string;
  duration_ms: number;
  track_count: number;
}

export type PlaybackState = "Playing" | "Paused" | "Stopped";

export interface InstanceCfg {
  weight: number;
  time_scale: number;
  start_offset: number;
  enabled: boolean;
}

export interface InstanceInfo {
  id: number;
  animation: number;
  cfg: InstanceCfg;
}

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
