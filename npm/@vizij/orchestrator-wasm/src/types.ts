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

/**
 * Typed metadata describing a Value's shape.
 *
 * Includes common helpers while staying permissive for legacy/forward-compatible shapes.
 */
/** Shape primitive payload accepted by the orchestrator. */
export type ShapePrimitive = { id: string } | { id: string; sizes?: number[] } | { id: string; params?: any };
/** Nested field map for record-shaped values. */
export type ShapeField = { [k: string]: ShapeJSON };
/** Union of supported shape representations accepted by the orchestrator. */
export type ShapeJSON = ShapePrimitive | { record?: ShapeField } | { array?: ShapeJSON } | any;

/** Write operation emitted by controllers during a step. */
export interface WriteOpJSON {
  path: string;
  value: ValueJSON;
  shape?: ShapeJSON;
}

/** Conflict log emitted when a write overwrote an existing entry. */
export interface ConflictLog {
  path: string;
  previous_value?: ValueJSON;
  previous_shape?: ShapeJSON;
  previous_epoch?: number;
  previous_source?: string;
  new_value: ValueJSON;
  new_shape?: ShapeJSON;
  new_epoch: number;
  new_source: string;
}

/** The orchestrator frame returned after each step. */
export interface OrchestratorFrame {
  /** Monotonic step counter (epoch). */
  epoch: number;
  /** Delta time in seconds passed to the step call. */
  dt: number;
  /** Merged writes from all controllers this step. */
  merged_writes: WriteOpJSON[];
  /** Conflicts captured while merging controller writes. */
  conflicts: ConflictLog[];
  /** Per-controller timings in milliseconds. */
  timings_ms: { [k: string]: number };
  /** Controller-specific event payloads (opaque). */
  events: any[];
}

/** High-level typed interface for the JS wrapper (for consumers who prefer TS types). */
export interface OrchestratorAPI {
  /** Register a new graph controller and return its id. */
  registerGraph(cfg: GraphRegistrationInput): string;
  /** Register a merged graph controller and return its id. */
  registerMergedGraph(cfg: MergedGraphRegistrationConfig): string;
  /** Register an animation controller and return its id. */
  registerAnimation(cfg: AnimationRegistrationConfig): string;
  /** Provide a resolver that maps typed paths to host-specific keys. */
  prebind(resolver: (path: string) => string | number | null | undefined): void;
  /** Set or update a blackboard input value. */
  setInput(path: string, value: ValueJSON, shape?: ShapeJSON): void;
  /** Remove an input value from the blackboard. */
  removeInput(path: string): boolean;
  /** Step all controllers and return the merged frame output. */
  step(dt: number): OrchestratorFrame;
  /** List controller ids grouped by type. */
  listControllers(): { graphs: string[]; anims: string[] };
  /** Remove a graph controller by id. */
  removeGraph(id: string): boolean;
  /** Remove an animation controller by id. */
  removeAnimation(id: string): boolean;
  /** Normalize a GraphSpec to match wasm expectations. */
  normalizeGraphSpec(spec: object | string): Promise<object>;
}

/** Re-exported helper types for consumers. */
export type { NormalizedValue as ValueNormalized };
/** Alias for the ValueJSON union from @vizij/value-json. */
export type { ValueJSON as Value };
/** Alias for the ShapeJSON type. */
export type { ShapeJSON as Shape };
/** Alias for OrchestratorFrame. */
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

/** Config used to register a node-graph controller. */
export interface GraphRegistrationConfig {
  /** Optional controller id (auto-generated when omitted). */
  id?: string;
  /** Graph spec JSON (object form). */
  spec: GraphSpec | any;
  /** Optional input/output subscription hints. */
  subs?: GraphSubscriptions;
}

/** Graph replacement config; requires an id. */
export interface GraphReplaceConfig extends GraphRegistrationConfig {
  /** Existing controller id to replace. */
  id: string;
}

/** Config used to register multiple graphs as a merged controller. */
export interface MergedGraphRegistrationConfig {
  /** Optional controller id (auto-generated when omitted). */
  id?: string;
  /** Graphs to merge into a single controller. */
  graphs: GraphRegistrationConfig[];
  /** Optional conflict resolution strategy. */
  strategy?: MergeStrategyOptions;
}

/**
 * Subscription hints for graph inputs/outputs.
 *
 * These mirror the fields on the Rust GraphSubscriptions type.
 */
export interface GraphSubscriptions {
  /** Inputs to read from the blackboard each step. */
  inputs?: string[];
  /** Outputs to write back to the blackboard each step. */
  outputs?: string[];
  /** Echo graph writes back to the blackboard even if not subscribed. */
  mirrorWrites?: boolean;
}

/** Graph registration inputs accepted by the JS wrapper. */
export type GraphRegistrationInput = string | GraphRegistrationConfig;

/** Conflict strategies supported by graph merge options. */
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

/** Merge strategy options for graph registration. */
export interface MergeStrategyOptions {
  /** How to resolve output name conflicts. */
  outputs?: MergeConflictStrategy;
  /** How to resolve conflicts within intermediate write paths. */
  intermediate?: MergeConflictStrategy;
}

/**
 * Registration config for an animation controller.
 *
 * Provide `setup` to seed the animation, player, and instance defaults.
 */
export interface AnimationRegistrationConfig {
  /** Optional controller id (auto-generated when omitted). */
  id?: string;
  /** Optional animation setup data used at registration. */
  setup?: AnimationSetup;
}

/** Animation setup data passed to a controller at registration. */
export interface AnimationSetup {
  /** Animation payload consumed by the animation controller. */
  animation?: any;
  player?: {
    /** Friendly label for the player. */
    name?: string;
    /** Loop mode for the player (default set by Rust). */
    loop_mode?: "once" | "loop" | "pingpong";
    /** Playback speed multiplier. */
    speed?: number;
  };
  instance?: {
    /** Blend weight for the instance. */
    weight?: number;
    /** Time scale multiplier for the instance. */
    time_scale?: number;
    /** Offset into the animation timeline, in seconds. */
    start_offset?: number;
    /** Whether the instance starts enabled. */
    enabled?: boolean;
  };
}
