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
  | "vec3"
  | "vec3split"
  | "vec3add"
  | "vec3subtract"
  | "vec3multiply"
  | "vec3scale"
  | "vec3normalize"
  | "vec3dot"
  | "vec3cross"
  | "vec3length"
  | "output";

/**
 * JSON form used at the wasm boundary.
 * (Core Values are encoded as one-of objects.)
 */
export type ValueJSON =
  | { float: number }
  | { bool: boolean }
  | { vec3: [number, number, number] };

export interface NodeParams {
  value?: ValueJSON | number | boolean | [number, number, number];
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
