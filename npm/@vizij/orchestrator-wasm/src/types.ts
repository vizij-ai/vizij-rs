/**
 * Detailed TypeScript definitions for the orchestrator package.
 *
 * These aim to be expressive enough for consumers while remaining permissive
 * for legacy shapes that the Rust serde layer accepts.
 */

import type {
  Float,
  Bool,
  Text,
  Vec2,
  Vec3,
  Vec4,
  Quat,
  ColorRgba,
  Vector,
  EnumVal,
  RecordVal,
  ArrayVal,
  ListVal,
  TupleVal,
  Transform,
  ValueJSON,
  NormalizedValue,
} from "@vizij/value-json";

export type {
  Float,
  Bool,
  Text,
  Vec2,
  Vec3,
  Vec4,
  Quat,
  ColorRgba,
  Vector,
  EnumVal,
  RecordVal,
  ArrayVal,
  ListVal,
  TupleVal,
  Transform,
  ValueJSON,
  NormalizedValue,
};

/* ShapeJSON: typed metadata describing a Value's shape.
   We provide a couple of common helpers while remaining permissive. */
/** Permissive shape metadata accepted by the orchestrator wasm boundary. */
export type ShapePrimitive = { id: string } | { id: string; sizes?: number[] } | { id: string; params?: any };
export type ShapeField = { [k: string]: ShapeJSON };
/**
 * Forward-compatible shape contract.
 *
 * Stable consumers should at least provide an `id`-based shape object; broader forms remain
 * accepted so older wrappers and host-specific metadata continue to round-trip.
 */
export type ShapeJSON = ShapePrimitive | { record?: ShapeField } | { array?: ShapeJSON } | any;

/** Write operation emitted by graph or animation controllers during a frame. */
export interface WriteOpJSON {
  path: string;
  value: ValueJSON;
  /** Optional explicit shape metadata. Omitted when the producer only emitted a value. */
  shape?: ShapeJSON;
}

/** Conflict record emitted when one write overwrote an existing blackboard entry. */
export interface ConflictLog {
  path: string;
  /** Previous entry metadata, present only when a value already existed at `path`. */
  previous_value?: ValueJSON;
  previous_shape?: ShapeJSON;
  previous_epoch?: number;
  previous_source?: string;
  /** Replacement entry metadata written by the winning operation. */
  new_value: ValueJSON;
  new_shape?: ShapeJSON;
  new_epoch: number;
  new_source: string;
}

/** Deterministic frame snapshot returned after one orchestrator step. */
export interface OrchestratorFrame {
  /** Frame counter for this orchestrator instance. */
  epoch: number;
  /** Requested step duration in seconds. */
  dt: number;
  /** Published writes in scheduler/controller order. */
  merged_writes: WriteOpJSON[];
  /** Overwrite diagnostics collected while applying the frame to the blackboard. */
  conflicts: ConflictLog[];
  /** Synthetic/per-pass timing fields keyed by pass name. Values are milliseconds. */
  timings_ms: { [k: string]: number };
  /** Controller-specific event payloads forwarded from the underlying runtime. */
  events: any[];
}

/** High-level typed surface implemented by the JS `Orchestrator` wrapper. */
export interface OrchestratorAPI {
  /** Register one graph controller and return the resolved controller id. */
  registerGraph(cfg: GraphRegistrationInput): string;
  /** Register one merged graph controller assembled from multiple graph configs. */
  registerMergedGraph(cfg: MergedGraphRegistrationConfig): string;
  /** Register one animation controller and return the resolved controller id. */
  registerAnimation(cfg: AnimationRegistrationConfig): string;
  /** Prebind animation target paths to host handles before stepping. */
  prebind(resolver: (path: string) => string | number | null | undefined): void;
  /** Overwrite the blackboard input stored at `path`. */
  setInput(path: string, value: ValueJSON, shape?: ShapeJSON): void;
  /** Remove a blackboard input. Returns `true` only when an entry existed. */
  removeInput(path: string): boolean;
  /** Advance all registered controllers once and return the frame snapshot. */
  step(dt: number): OrchestratorFrame;
  /** Return the currently registered controller ids without stepping. */
  listControllers(): { graphs: string[]; anims: string[] };
  /** Remove a graph controller by id. */
  removeGraph(id: string): boolean;
  /** Remove an animation controller by id. */
  removeAnimation(id: string): boolean;
  /** Normalize a graph spec via the Rust normalizer and return the normalized JSON object. */
  normalizeGraphSpec(spec: object | string): Promise<object>;
}

/* exported helper types for consumers */
export type { NormalizedValue as ValueNormalized };
export type { ValueJSON as Value };
export type { ShapeJSON as Shape };
export type { OrchestratorFrame as Frame };

/**
 * GraphSpec is the node-graph JSON consumed by graph controllers.
 *
 * This package stays permissive for legacy/forward-compatible shapes, but we still surface the
 * cache-related fields so consumers can understand the performance model.
 */
export interface GraphSpec {
  nodes: any[];
  edges?: any[];
  /**
   * Optional plan-validity key used by the node-graph runtime to reuse cached layouts/bindings.
   *
   * - This is auto-filled/managed by the wasm layer.
   * - It should bump only for *structural* edits (those that can affect port layouts or bindings),
   *   not for ordinary param/value changes.
   */
  specVersion?: number;
  /**
   * Optional structural fingerprint used alongside `specVersion` for validation/debugging.
   * Auto-filled/managed by the wasm layer.
   */
  fingerprint?: number;
}

/** Config used when registering one graph controller. */
export interface GraphRegistrationConfig {
  /** Optional id override. Omit to let the wrapper/runtime choose a controller id. */
  id?: string;
  spec: GraphSpec | any;
  /** Optional subscription filters controlling what gets staged and published. */
  subs?: GraphSubscriptions;
}

/** Replacement payload used by `replaceGraph()`-style flows. */
export interface GraphReplaceConfig extends GraphRegistrationConfig {
  /** Existing graph controller id to replace. */
  id: string;
}

/** Config used when registering a merged graph controller. */
export interface MergedGraphRegistrationConfig {
  id?: string;
  /** Source graph configs merged into one controller in declaration order. */
  graphs: GraphRegistrationConfig[];
  /** Optional conflict policy for merged outputs and intermediate nodes. */
  strategy?: MergeStrategyOptions;
}

/** Input/output subscription filters for graph controllers. */
export interface GraphSubscriptions {
  /** Blackboard paths staged into the graph each step. Empty/omitted means no external inputs are staged. */
  inputs?: string[];
  /** Published write paths exposed in `merged_writes`. Empty/omitted means publish all graph writes. */
  outputs?: string[];
  /** Mirror all graph writes into the blackboard even when `outputs` filters publication. */
  mirrorWrites?: boolean;
}

export type GraphRegistrationInput = string | GraphRegistrationConfig;

export type MergeConflictStrategy =
  | "error"
  | "namespace"
  | "blend"
  | "blend_equal"
  | "blend_equal_weights"
  | "add"
  | "sum"
  | "blend_sum"
  | "blend-sum"
  | "additive"
  | "default_blend"
  | "default-blend"
  | "blend-default"
  | "blend_weights"
  | "blend-weights"
  | "weights";

/** Conflict policy for merged graph registration. */
export interface MergeStrategyOptions {
  /** Resolution strategy for collisions between final output paths. */
  outputs?: MergeConflictStrategy;
  /** Resolution strategy for collisions between intermediate/internal nodes. */
  intermediate?: MergeConflictStrategy;
}

/** Config used when registering an animation controller. */
export interface AnimationRegistrationConfig {
  id?: string;
  /** Optional setup blob forwarded to the animation controller constructor. */
  setup?: AnimationSetup;
}

/** Initial animation/player/instance setup accepted by the wasm wrapper. */
export interface AnimationSetup {
  /** Animation payload consumed by the animation controller. Shape is intentionally permissive. */
  animation?: any;
  player?: {
    name?: string;
    loop_mode?: "once" | "loop" | "pingpong";
    speed?: number;
  };
  instance?: {
    /** Blend weight for the registered instance. */
    weight?: number;
    /** Playback scaling factor applied by the animation engine. */
    time_scale?: number;
    /** Start offset in seconds on the player timeline. */
    start_offset?: number;
    enabled?: boolean;
  };
}
