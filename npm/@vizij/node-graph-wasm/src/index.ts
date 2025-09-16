// Stable ESM entry for @vizij/graph-wasm
// Wraps the wasm-pack output in ../pkg (built with `--target web`).
// Adjust the import path if your pkg name differs.
import initWasm, { WasmGraph, get_node_schemas_json } from "../pkg/vizij_graph_wasm.js";
import type {
  NodeId,
  NodeType,
  ValueJSON,
  NodeParams,
  NodeSpec,
  GraphSpec,
  GraphOutputs,
  InitInput,
} from "./types";

export type {
  NodeId,
  NodeType,
  ValueJSON,
  NodeParams,
  NodeSpec,
  GraphSpec,
  GraphOutputs,
  InitInput,
} 

// --- init() ---

let _initPromise: Promise<void> | null = null;

function defaultWasmUrl(): URL {
  return new URL("../pkg/vizij_graph_wasm_bg.wasm", import.meta.url);
}

/**
 * Initialize the wasm module once.
 */
export function init(input?: InitInput): Promise<void> {
  // If already started, return the same promise (never null hereafter)
  if (_initPromise) return _initPromise;

  // initWasm returns Promise<InitOutput>; coerce to Promise<void>
  _initPromise = (async () => {
    await initWasm(input ?? defaultWasmUrl());
  })();

  return _initPromise;
}

// Don’t try to assert on an outer variable — just check & throw.
function ensureInited(): void {
  if (!_initPromise) {
    throw new Error(
      "Call init() from @vizij/node-graph-wasm before creating Graph instances."
    );
  }
}


// --- Value helpers ---

export type Value =
  | number
  | boolean
  | number[] // vector of arbitrary length; length 3 is treated as vec3
  | ValueJSON; // accept already-encoded JSON form too

export function toValueJSON(v: Value): ValueJSON {
  if (typeof v === "number") return { float: v };
  if (typeof v === "boolean") return { bool: v };
  if (Array.isArray(v)) {
    // Always encode JS arrays as generic vectors to avoid accidental vec3 coercion.
    // Vec3 values should be passed explicitly as { vec3: [x,y,z] } when intended.
    return { vector: v.slice() };
  }
  // assume already ValueJSON-ish
  if ("float" in v || "bool" in v || "vec3" in v || "vector" in v) return v as ValueJSON;
  throw new Error("Unsupported Value shape");
}

// --- Public API class ---

/**
 * Ergonomic wrapper around wasm WasmGraph.
 * Always await init() once before constructing.
 */
export class Graph {
  private inner: WasmGraph;

  constructor() {
    ensureInited();
    this.inner = new WasmGraph();
  }

  /**
   * Load a graph spec (object or JSON string).
   */
  loadGraph(spec: GraphSpec | string): void {
    const json = typeof spec === "string" ? spec : JSON.stringify(spec);
    this.inner.load_graph(json);
  }

  /** Set absolute time (seconds). */
  setTime(t: number): void {
    this.inner.set_time(t);
  }

  /** Increment time (seconds). */
  step(dt: number): void {
    this.inner.step(dt);
  }

  /**
   * Evaluate the whole graph and return a map of nodeId -> outputKey -> ValueJSON.
   * (One batched wasm call.)
   */
  evalAll(): Record<string, Record<string, ValueJSON>> {
    const raw = this.inner.eval_all(); // JSON string
    const parsed = JSON.parse(raw) as {
      nodes: Record<string, Record<string, ValueJSON>>;
      writes: unknown;
    };
    return parsed.nodes;
  }

  /**
   * Update a node parameter by key (e.g., "value", "frequency", "phase", "min", "max", "x", "y", "z").
   * Value may be number | boolean | vec3 or a pre-encoded ValueJSON.
   */
  setParam(nodeId: string, key: string, value: Value): void {
    const payload = JSON.stringify(toValueJSON(value));
    this.inner.set_param(nodeId, key, payload);
  }
}

// Convenience re-exports for consumers who prefer a function-style API
export async function createGraph(
  spec?: GraphSpec | string
): Promise<Graph> {
  await init();
  const g = new Graph();
  if (spec) g.loadGraph(spec);
  return g;
}

/**
 * Fetch the node schema registry from the wasm module as a parsed object.
 * Ensures the wasm module is initialized before calling.
 */
export async function getNodeSchemas(): Promise<import("./types").Registry> {
  await init();
  const raw = get_node_schemas_json();
  return JSON.parse(raw) as import("./types").Registry;
}
