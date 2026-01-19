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
 * Includes a few common helpers while staying permissive for legacy/forward-compatible shapes.
 */
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
  epoch: number;
  dt: number;
  merged_writes: WriteOpJSON[];
  conflicts: ConflictLog[];
  timings_ms: { [k: string]: number };
  events: any[]; // controller-specific event payloads
}

/** High-level typed interface for the JS wrapper (for consumers who prefer TS types). */
export interface OrchestratorAPI {
  registerGraph(cfg: GraphRegistrationInput): string;
  registerMergedGraph(cfg: MergedGraphRegistrationConfig): string;
  registerAnimation(cfg: AnimationRegistrationConfig): string;
  prebind(resolver: (path: string) => string | number | null | undefined): void;
  setInput(path: string, value: ValueJSON, shape?: ShapeJSON): void;
  removeInput(path: string): boolean;
  step(dt: number): OrchestratorFrame;
  listControllers(): { graphs: string[]; anims: string[] };
  removeGraph(id: string): boolean;
  removeAnimation(id: string): boolean;
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
  id?: string;
  spec: GraphSpec | any;
  subs?: GraphSubscriptions;
}

/** Graph replacement config; requires an id. */
export interface GraphReplaceConfig extends GraphRegistrationConfig {
  id: string;
}

/** Config used to register multiple graphs as a merged controller. */
export interface MergedGraphRegistrationConfig {
  id?: string;
  graphs: GraphRegistrationConfig[];
  strategy?: MergeStrategyOptions;
}

/**
 * Subscription hints for graph inputs/outputs.
 *
 * These mirror the fields on the Rust GraphSubscriptions type.
 */
export interface GraphSubscriptions {
  inputs?: string[];
  outputs?: string[];
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
  outputs?: MergeConflictStrategy;
  intermediate?: MergeConflictStrategy;
}

/**
 * Registration config for an animation controller.
 *
 * Provide `setup` to seed the animation, player, and instance defaults.
 */
export interface AnimationRegistrationConfig {
  id?: string;
  setup?: AnimationSetup;
}

/** Animation setup data passed to a controller at registration. */
export interface AnimationSetup {
  animation?: any;
  player?: {
    name?: string;
    loop_mode?: "once" | "loop" | "pingpong";
    speed?: number;
  };
  instance?: {
    weight?: number;
    time_scale?: number;
    start_offset?: number;
    enabled?: boolean;
  };
}
