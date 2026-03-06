// Public TypeScript types for @vizij/graph-wasm

import type { ValueJSON as BaseValueJSON, NormalizedValue } from "@vizij/value-json";

export type ValueJSON = BaseValueJSON;
export type { NormalizedValue };

/**
 * Convenient authoring forms accepted by wrapper helpers and inline defaults.
 *
 * The runtime normalizes these into canonical `ValueJSON` before evaluation.
 */
export type ValueLike =
  | ValueJSON
  | number
  | boolean
  | [number, number, number]
  | number[];

/** Unique node identifier within a single `GraphSpec.nodes` array. */
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

/**
 * Shared parameter bag for graph nodes.
 *
 * Individual node kinds only read a subset of these fields; unused fields are ignored so specs
 * can stay forward-compatible across hosts.
 */
export interface NodeParams {
  value?: ValueJSON | number | boolean | [number, number, number] | number[];
  /** Segment sizes for `split`; fractional values are floored by the Rust runtime. */
  sizes?: number[];
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
  /** Optional seed vector passed into IK solvers. */
  seed?: number[];
  /** Optional per-joint weights passed into IK solvers. */
  weights?: number[];
  max_iters?: number;
  tol_pos?: number;
  tol_rot?: number;
  joint_defaults?: [string, number][];
  /** Branch labels matched by `case` routing nodes. */
  case_labels?: string[];
}

/** One step in a selector path applied to a structured output value. */
export type SelectorSegmentJSON =
  /** Traverse into a record field by name. */
  | { field: string }
  /** Traverse into an array/vector/list element by zero-based index. */
  | { index: number };

/** One node declaration within a graph spec. */
export interface NodeSpec {
  id: NodeId;
  /** Rust field name is `type`, but `type` is a TS keyword; JSON still uses `"type"`. */
  type: NodeType;
  params?: NodeParams;
  output_shapes?: Record<string, ShapeJSON>;
  /**
   * Optional map of input names to inline default values. Each entry mirrors the effect of wiring a
   * Constant node but can be overridden by an explicit edge targeting the same input. Bare values
   * are normalized as `{ value }`; the optional `shape` applies only to that inline default.
   */
  input_defaults?: Record<
    string,
    ValueLike | { value: ValueLike; shape?: ShapeJSON }
  >;
}

/** Source endpoint for an edge. */
export interface EdgeOutputEndpoint {
  node_id: NodeId;
  /** Output port name. Defaults to `"out"` when omitted in JSON. */
  output?: string;
}

/** Destination endpoint for an edge. */
export interface EdgeInputEndpoint {
  node_id: NodeId;
  input: string;
}

/** Directed connection between a source output and destination input. */
export interface EdgeSpec {
  from: EdgeOutputEndpoint;
  to: EdgeInputEndpoint;
  /** Optional field/index traversal applied to the source value before delivery. */
  selector?: SelectorSegmentJSON[];
}

/** JSON graph contract accepted by the wasm graph runtime. */
export interface GraphSpec {
  nodes: NodeSpec[];
  /** Directed edges between node ports. Omit or pass `[]` for disconnected/default-only graphs. */
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

/** Snapshot of one output port after a completed evaluation step. */
export interface PortSnapshot {
  value: ValueJSON;
  shape: ShapeJSON;
}

/** Output snapshot keyed as `node_id -> output_port -> snapshot`. */
export type GraphOutputs = Record<NodeId, Record<string, PortSnapshot>>;

/** External write emitted by output/sink nodes during one evaluation. */
export interface WriteOpJSON {
  path: string;
  value: ValueJSON;
  shape: ShapeJSON;
}

/** Full evaluation result returned by wrapper helpers such as `evalAll()`. */
export interface EvalResult {
  nodes: Record<NodeId, Record<string, PortSnapshot>>;
  /** Ordered writes emitted during this evaluation. They are not conflict-resolved or deduplicated. */
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
export type ParamType = "float" | "bool" | "vec3" | "vector" | "any";

/** Schema description for one fixed input or output port. */
export interface PortSpec {
  /** Canonical port id (for example `"out"`, `"in"`, `"lhs"`, `"rhs"`). */
  id: string;
  ty: PortType;
  /** Human-friendly label suitable for UI palettes and inspectors. */
  label: string;
  /** Optional help text emitted by the Rust registry. */
  doc?: string;
  /** Whether the port may be omitted by callers. */
  optional?: boolean;
}

/** Schema for a variadic group of ports. */
export interface VariadicSpec {
  /** Canonical group id for the repeated port family (for example `"operands"`). */
  id: string;
  ty: PortType;
  label: string;
  doc?: string;
  /** Minimum number of ports the caller must provide. */
  min: number;
  /** Maximum number of accepted ports, when finite. */
  max?: number;
}

/** Schema for one configurable node parameter. */
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

/** Registry entry describing one node type exposed by the wasm module. */
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

/** Top-level schema registry returned by `getNodeSchemas()`. */
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
 * You must have called `init()` before using this (or call `getNodeSchemas` from the JS wrapper).
 */
export function getNodeSchemas(): Promise<Registry>;
