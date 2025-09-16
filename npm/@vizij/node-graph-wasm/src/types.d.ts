// Public TypeScript types for @vizij/graph-wasm

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
  | "sin"
  | "cos"
  | "tan"
  | "time"
  | "oscillator"
  | "and"
  | "or"
  | "not"
  | "xor"
  | "greaterthan"
  | "lessthan"
  | "equal"
  | "notequal"
  | "if"
  | "clamp"
  | "remap"
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
  | "inversekinematics"
  | "output";

/**
 * JSON form used at the wasm boundary.
 * (Core Values are encoded as one-of objects.)
 */
export type ValueJSON =
  | { float: number }
  | { bool: boolean }
  | { vec2: [number, number] }
  | { vec3: [number, number, number] }
  | { vec4: [number, number, number, number] }
  | { quat: [number, number, number, number] }
  | { color: [number, number, number, number] }    // ColorRgba
  | { transform: { pos: [number, number, number]; rot: [number, number, number, number]; scale: [number, number, number] } }
  | { vector: number[] }
  | { text: string }
  | { enum: { tag: string; value: ValueJSON } };

export interface NodeParams {
  value?: ValueJSON | number | boolean | [number, number, number] | number[];
  sizes?: number[]; // for Split
  frequency?: number;
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
}

export interface NodeSpec {
  id: NodeId;
  /** Rust field name is `type`, but `type` is a TS keyword; JSON still uses `"type"`. */
  type: NodeType;
  params?: NodeParams;
  /**
   * Map of input name to a connection specifying the source node and which output key to read.
   * Matches Rust: HashMap<String, InputConnection> where InputConnection { node_id, output_key }.
   */
  inputs?: Record<string, { node_id: string; output_key: string }>;
}

export interface GraphSpec {
  nodes: NodeSpec[];
}

export type GraphOutputs = Record<NodeId, Record<string, ValueJSON>>;

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

export type PortType = "float" | "bool" | "vec3" | "vector";
export type ParamType = "float" | "bool" | "vec3" | "vector" | "any";

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
  doc?: string;
  min: number;
  max?: number;
}

export interface ParamSpec {
  id: string;
  ty: ParamType;
  label: string;
  doc?: string;
  default_json?: ValueJSON;  // default value encoded as ValueJSON when applicable
  min?: number;
  max?: number;
}

export interface NodeSignature {
  type_id: NodeType;
  name: string;
  category: string;
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

/**
 * Fetch the node schema registry from the wasm module.
 * You must have called init() before using this (or call getNodeSchemas from the JS wrapper).
 */
export function getNodeSchemas(): Promise<Registry>;
