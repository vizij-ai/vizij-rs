// Stable ESM entry for @vizij/graph-wasm
// Wraps the wasm-pack output in ../pkg (built with `--target web`).
// Adjust the import path if your pkg name differs.
import initWasm, { WasmGraph } from "../pkg/vizij_graph_wasm.js";
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
  | [number, number, number]
  | ValueJSON; // accept already-encoded JSON form too

export function toValueJSON(v: Value): ValueJSON {
  if (typeof v === "number") return { float: v };
  if (typeof v === "boolean") return { bool: v };
  if (Array.isArray(v)) {
    if (v.length !== 3) throw new Error("vec3 must be [x,y,z]");
    return { vec3: [v[0] ?? 0, v[1] ?? 0, v[2] ?? 0] };
  }
  // assume already ValueJSON-ish
  if ("float" in v || "bool" in v || "vec3" in v) return v as ValueJSON;
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
   * Evaluate the whole graph and return a map of nodeId -> ValueJSON.
   * (One batched wasm call.)
   */
  evalAll(): Record<string, ValueJSON> {
    const raw = this.inner.eval_all(); // JSON string
    return JSON.parse(raw) as Record<string, ValueJSON>;
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
