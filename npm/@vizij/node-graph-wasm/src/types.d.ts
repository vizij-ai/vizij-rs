// Public TypeScript types for @vizij/graph-wasm

import type { ValueJSON as BaseValueJSON, NormalizedValue } from "@vizij/value-json";

export type ValueJSON = BaseValueJSON;
export type { NormalizedValue };

export type ValueLike =
  | ValueJSON
  | number
  | boolean
  | [number, number, number]
  | number[];

export type NodeId = string;

/**
 * Matches Rust enum `NodeType` with `#[serde(rename_all = "lowercase")]`.
 */
export type NodeType =
  | "constant"
  | "slider"
  | "multislider"
  | "add"
  | "subtract"
  | "multiply"
  | "divide"
  | "power"
  | "log"
  | "abs"
  | "modulo"
  | "sqrt"
  | "sign"
  | "min"
  | "max"
  | "round"
  | "sin"
  | "cos"
  | "tan"
  | "time"
  | "oscillator"
  | "spring"
  | "damp"
  | "slew"
  | "and"
  | "or"
  | "not"
  | "xor"
  | "greaterthan"
  | "lessthan"
  | "equal"
  | "notequal"
  | "if"
  | "case"
  | "clamp"
  | "remap"
  | "centered_remap"
  | "piecewise_remap"
  | "vec3cross"
  | "vectorconstant"
  | "vectoradd"
  | "vectorsubtract"
  | "vectormultiply"
  | "vectorscale"
  | "vectornormalize"
  | "vectordot"
  | "vectorlength"
  | "vectorindex"
  | "join"
  | "split"
  | "vectormin"
  | "vectormax"
  | "vectormean"
  | "vectormedian"
  | "vectormode"
  | "tovector"
  | "fromvector"
  | "simplenoise"
  | "perlinnoise"
  | "simplexnoise"
  | "weightedsumvector"
  | "default-blend"
  | "blendweightedaverage"
  | "blendadditive"
  | "blendmultiply"
  | "blendweightedoverlay"
  | "blendweightedaverageoverlay"
  | "blendmax"
  | "inversekinematics"
  | "urdfikposition"
  | "urdfikpose"
  | "urdffk"
  | "buildrecord"
  | "readrecord"
  | "switchrecord"
  | "mergerecord"
  | "splitrecord"
  | "mathmultrecord"
  | "mathaddrecord"
  | "mathdivrecord"
  | "mathsubrecord"
  | "input"
  | "output";

export type ShapeJSON =
  | { id: "Scalar"; meta?: Record<string, string> }
  | { id: "Bool"; meta?: Record<string, string> }
  | { id: "Vec2"; meta?: Record<string, string> }
  | { id: "Vec3"; meta?: Record<string, string> }
  | { id: "Vec4"; meta?: Record<string, string> }
  | { id: "Quat"; meta?: Record<string, string> }
  | { id: "ColorRgba"; meta?: Record<string, string> }
  | { id: "Transform"; meta?: Record<string, string> }
  | { id: "Text"; meta?: Record<string, string> }
  | { id: "Vector"; data?: { len?: number }; meta?: Record<string, string> }
  | { id: "Record"; data: { name: string; shape: ShapeJSON }[]; meta?: Record<string, string> }
  | { id: "Array"; data: [ShapeJSON, number]; meta?: Record<string, string> }
  | { id: "List"; data: ShapeJSON; meta?: Record<string, string> }
  | { id: "Tuple"; data: ShapeJSON[]; meta?: Record<string, string> }
  | { id: "Enum"; data: [string, ShapeJSON][]; meta?: Record<string, string> };

export interface NodeParams {
  value?: ValueJSON | number | boolean | [number, number, number] | number[];
  sizes?: number[]; // for Split
  frequency?: number;
  noise_seed?: number;
  octaves?: number;
  lacunarity?: number;
  persistence?: number;
  phase?: number;
  min?: number;
  max?: number;
  x?: number;
  y?: number;
  z?: number;
  in_min?: number;
  in_max?: number;
  out_min?: number;
  out_max?: number;
  /** Optional typed-path target for sinks (validated in Rust). */
  path?: string;
  stiffness?: number;
  damping?: number;
  mass?: number;
  half_life?: number;
  max_rate?: number;
  urdf_xml?: string;
  root_link?: string;
  tip_link?: string;
  seed?: number[];
  weights?: number[];
  max_iters?: number;
  tol_pos?: number;
  tol_rot?: number;
  joint_defaults?: [string, number][];
  case_labels?: string[];
  record_keys?: string[]; // for BuildRecord/ReadRecord
  keys?: string; // for SplitRecord (comma-separated field names)
}

export type SelectorSegmentJSON =
  | { field: string }
  | { index: number };

export interface NodeSpec {
  id: NodeId;
  /** Rust field name is `type`, but `type` is a TS keyword; JSON still uses `"type"`. */
  type: NodeType;
  params?: NodeParams;
  output_shapes?: Record<string, ShapeJSON>;
  /**
   * Optional map of input names to inline default values. Each entry mirrors the effect of wiring a
   * Constant node but can be overridden by an explicit link targeting the same input.
   */
  input_defaults?: Record<
    string,
    ValueLike | { value: ValueLike; shape?: ShapeJSON }
  >;
}

export interface EdgeOutputEndpoint {
  node_id: NodeId;
  output?: string;
}

export interface EdgeInputEndpoint {
  node_id: NodeId;
  input: string;
}

export interface EdgeSpec {
  from: EdgeOutputEndpoint;
  to: EdgeInputEndpoint;
  selector?: SelectorSegmentJSON[];
}

export interface GraphSpec {
  nodes: NodeSpec[];
  edges?: EdgeSpec[];
  /**
   * Optional caller-managed cache version used as a *plan-validity key*.
   *
   * - The wasm loader auto-fills this when omitted.
   * - It should only bump when the graph's *layout/bindings* change (structural edits), not for
   *   ordinary param/value tweaks.
   */
  specVersion?: number;
  /**
   * Optional structural fingerprint used alongside `specVersion` as a debug/validation aid.
   * Auto-filled alongside `specVersion` when omitted.
   */
  fingerprint?: number;
}

export interface PortSnapshot {
  value: ValueJSON;
  shape: ShapeJSON;
}

export type GraphOutputs = Record<NodeId, Record<string, PortSnapshot>>;

export interface WriteOpJSON {
  path: string;
  value: ValueJSON;
  shape: ShapeJSON;
}

export interface EvalResult {
  nodes: Record<NodeId, Record<string, PortSnapshot>>;
  writes: WriteOpJSON[];
}

/**
 * Input accepted by wasm-bindgen init; mirrors what wasm-pack generated init()
 * accepts (URL/Request/Response/BufferSource/WebAssembly.Module).
 */
export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

/* --------------------------------------------------------------------
   Node Schema Registry (exported from wasm via get_node_schemas_json)
-------------------------------------------------------------------- */

export type PortType =
  | "float"
  | "bool"
  | "vec3"
  | "quat"
  | "transform"
  | "vector"
  | "any";
export type ParamType = "float" | "bool" | "vec3" | "vector" | "any" | "text";

export interface PortSpec {
  id: string;            // canonical port id (e.g., "out", "in", "lhs", "rhs", "x", "y", "z")
  ty: PortType;
  label: string;         // human-friendly label for UI
  doc?: string;          // optional help text
  optional?: boolean;    // missing by default
}

export interface VariadicSpec {
  id: string;            // group id for variadic inputs (e.g., "operands")
  ty: PortType;
  label: string;
  keyed?: boolean;       // true when each slot has a user-editable string key
  doc?: string;
  min: number;
  max?: number;
}

export interface ParamSpec {
  id: string;
  ty: ParamType;
  label: string;
  doc?: string;
  /**
   * Raw JSON default value as emitted by the Rust registry.
   * Usually encoded as ValueJSON but may also be primitive types or arrays.
   */
  default_json?: unknown;
  min?: number;
  max?: number;
}

export interface NodeSignature {
  type_id: NodeType;
  name: string;
  category: string;
  doc?: string;
  inputs: PortSpec[];
  variadic_inputs?: VariadicSpec;
  outputs: PortSpec[];
  variadic_outputs?: VariadicSpec;
  params: ParamSpec[];
}

export interface Registry {
  version: string;
  nodes: NodeSignature[];
}

/* --------------------------------------------------------------------
   Samples (exported from wasm via src/index.ts)
-------------------------------------------------------------------- */
export const oscillatorBasics: GraphSpec;
export const vectorPlayground: GraphSpec;
export const logicGate: GraphSpec;
export const tupleSpringDampSlew: GraphSpec;
export const nestedRigWeightedPose: GraphSpec;
export const selectorCascade: GraphSpec;
export const graphSamples: Record<string, GraphSpec>;

/**
 * Fetch the node schema registry from the wasm module.
 * You must have called init() before using this (or call getNodeSchemas from the JS wrapper).
 */
export function getNodeSchemas(): Promise<Registry>;
