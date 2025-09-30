/**
 * Detailed TypeScript definitions for the orchestrator package.
 *
 * These aim to be expressive enough for consumers while remaining permissive
 * for legacy shapes that the Rust serde layer accepts.
 */

/* Core primitive / legacy representations */
export type Float = { float: number };
export type Bool = { bool: boolean };
export type Text = { text: string };

export type Vec2 = { vec2: [number, number] };
export type Vec3 = { vec3: [number, number, number] };
export type Vec4 = { vec4: [number, number, number, number] };
export type Quat = { quat: [number, number, number, number] };
export type ColorRgba = { color: [number, number, number, number] };

/* Generic containers */
export type Vector = { vector: number[] };
export type ArrayVal = { array: ValueJSON[] };
export type ListVal = { list: ValueJSON[] };
export type TupleVal = { tuple: ValueJSON[] };
export type RecordVal = { record: { [k: string]: ValueJSON } };

/* Enum and transform */
export type EnumVal = { enum: { tag: string; value: ValueJSON } };
export type Transform = { transform: { pos: ValueJSON; rot: ValueJSON; scale: ValueJSON } };

/* Normalized (tooling) representation produced by node-graph normalizer:
   { type: "Float"|"Vec3"|..., data: ... } */
export type NormalizedValue =
  | { type: "Float"; data: number }
  | { type: "Bool"; data: boolean }
  | { type: "Text"; data: string }
  | { type: "Vec2"; data: [number, number] }
  | { type: "Vec3"; data: [number, number, number] }
  | { type: "Vec4"; data: [number, number, number, number] }
  | { type: "Quat"; data: [number, number, number, number] }
  | { type: "ColorRgba"; data: [number, number, number, number] }
  | { type: "Vector"; data: number[] }
  | { type: "List"; data: NormalizedValue[] }
  | { type: "Array"; data: NormalizedValue[] }
  | { type: "Tuple"; data: NormalizedValue[] }
  | { type: "Enum"; data: [string, NormalizedValue] }
  | { type: "Record"; data: { [k: string]: NormalizedValue } }
  | { type: "Transform"; data: { pos: NormalizedValue; rot: NormalizedValue; scale: NormalizedValue } }
  | { type: string; data: any }; // forward compatible

/* The ValueJSON union accepts either the legacy/compact forms, normalized objects,
   or pragmatic JS-friendly primitives that are commonly used by consumers. */
export type ValueJSON =
  | Float
  | Bool
  | Text
  | Vec2
  | Vec3
  | Vec4
  | Quat
  | ColorRgba
  | Vector
  | EnumVal
  | RecordVal
  | ArrayVal
  | ListVal
  | TupleVal
  | Transform
  | NormalizedValue
  | number
  | string
  | boolean
  | any; // allow custom shapes for forward compatibility

/* ShapeJSON: typed metadata describing a Value's shape.
   We provide a couple of common helpers while remaining permissive. */
export type ShapePrimitive = { id: string } | { id: string; sizes?: number[] } | { id: string; params?: any };
export type ShapeField = { [k: string]: ShapeJSON };
export type ShapeJSON = ShapePrimitive | { record?: ShapeField } | { array?: ShapeJSON } | any;

/* Write operation emitted by controllers */
export interface WriteOpJSON {
  path: string;
  value: ValueJSON;
  shape?: ShapeJSON;
}

/* Conflict log emitted when a write overwrote an existing entry */
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

/* The orchestrator frame returned after each step */
export interface OrchestratorFrame {
  epoch: number;
  dt: number;
  merged_writes: WriteOpJSON[];
  conflicts: ConflictLog[];
  timings_ms: { [k: string]: number };
  events: any[]; // controller-specific event payloads
}

/* High-level typed interface for the wrapper (for consumers who prefer TS types) */
export interface OrchestratorAPI {
  registerGraph(cfg: GraphRegistrationInput): string;
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

/* exported helper types for consumers */
export type { NormalizedValue as ValueNormalized };
export type { ValueJSON as Value };
export type { ShapeJSON as Shape };
export type { OrchestratorFrame as Frame };

/* Helper config types used by the JS wrapper */
export interface GraphRegistrationConfig {
  id?: string;
  spec: any;
  subs?: GraphSubscriptions;
}

export interface GraphSubscriptions {
  inputs?: string[];
  outputs?: string[];
  mirrorWrites?: boolean;
}

export type GraphRegistrationInput = string | GraphRegistrationConfig;

export interface AnimationRegistrationConfig {
  id?: string;
  setup?: AnimationSetup;
}

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
