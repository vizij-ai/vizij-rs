// Stable ESM entry for @vizij/graph-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
// Adjust the import path if your pkg name differs.
let _bindings: any | null = null;

function pkgWasmJsUrl(): URL {
  // Resolve package-local pkg/ for both src/ and dist/src/ callers
  return new URL("../../pkg/vizij_graph_wasm.js", import.meta.url);
}

function defaultWasmUrl(): URL {
  return new URL("../../pkg/vizij_graph_wasm_bg.wasm", import.meta.url);
}

async function loadBindings(input?: InitInput): Promise<any> {
  if (!_bindings) {
    const mod: any = await import(/* @vite-ignore */ pkgWasmJsUrl().toString());
    let initArg: any = input ?? defaultWasmUrl();

    // In Node tests, fetch(file://) is not supported; read the wasm bytes instead.
    try {
      const isUrlObj = typeof initArg === "object" && initArg !== null && "href" in (initArg as any);
      const href = isUrlObj ? (initArg as URL).href : (typeof initArg === "string" ? (initArg as string) : "");
      const isFileUrl =
        (isUrlObj && (initArg as URL).protocol === "file:") ||
        (typeof href === "string" && href.startsWith("file:"));

      if (isFileUrl) {
        // Compute spec strings to avoid Vite pre-bundler statically analyzing node: imports in browser builds.
        const fsSpec = "node:fs/promises";
        const urlSpec = "node:url";
        const [{ readFile }, { fileURLToPath }] = await Promise.all([
          import(/* @vite-ignore */ fsSpec),
          import(/* @vite-ignore */ urlSpec),
        ]);
        const path = fileURLToPath(isUrlObj ? (initArg as URL) : new URL(href));
        const bytes = await readFile(path);
        initArg = bytes; // Uint8Array acceptable by wasm-bindgen init
      }
    } catch {
      // Fall back to default behavior; browser bundlers handle URL fetch.
    }

    await mod.default(initArg);
    _bindings = mod;
  }
  return _bindings;
}
import type {
  NodeId,
  NodeType,
  ValueJSON,
  NodeParams,
  NodeSpec,
  GraphSpec,
  GraphOutputs,
  InitInput,
  PortSnapshot,
  EvalResult,
  ShapeJSON,
  WriteOpJSON,
  ParamSpec,
  Registry,
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
  PortSnapshot,
  EvalResult,
  WriteOpJSON,
  ShapeJSON,
  ParamSpec,
  Registry,
} 

// --- init() ---

let _initPromise: Promise<void> | null = null;


/**
 * Initialize the wasm module once.
 */
export function init(input?: InitInput): Promise<void> {
  // If already started, return the same promise (never null hereafter)
  if (_initPromise) return _initPromise;

  // initWasm returns Promise<InitOutput>; coerce to Promise<void>
  _initPromise = (async () => {
    await loadBindings(input);
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
  | string
  | number[] // vector of arbitrary length; length 3 is treated as vec3
  | ValueJSON; // accept already-encoded JSON form too

export function toValueJSON(v: Value): ValueJSON {
  if (typeof v === "number") return { float: v };
  if (typeof v === "boolean") return { bool: v };
  if (typeof v === "string") return { text: v };
  if (Array.isArray(v)) {
    // Always encode JS arrays as generic vectors to avoid accidental vec3 coercion.
    // Vec3 values should be passed explicitly as { vec3: [x,y,z] } when intended.
    return { vector: v.slice() };
  }
  // assume already ValueJSON-ish (accept full ValueJSON surface)
  return v as ValueJSON;
}

// --- Public API class ---

/**
 * Ergonomic wrapper around wasm WasmGraph.
 * Always await init() once before constructing.
 */
export class Graph {
  private inner: any;

  constructor() {
    ensureInited();
    if (!_bindings) {
      throw new Error("Call init() from @vizij/node-graph-wasm before creating Graph instances.");
    }
    const WasmGraphCtor = (_bindings as any).WasmGraph;
    this.inner = new WasmGraphCtor();
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
   * Stage a host-provided input for the next evalAll() tick.
   * Values staged before evalAll will be visible in that evaluation (epoch semantics).
   */
  stageInput(path: string, value: Value, declaredShape?: ShapeJSON): void {
    const payload = JSON.stringify(toValueJSON(value));
    const shapeStr = declaredShape ? JSON.stringify(declaredShape) : undefined;
    // wasm-bindgen maps Option<String> to string | undefined
    (this.inner as any).stage_input(path, payload, shapeStr);
  }

  /**
   * Evaluate the whole graph and return a map of nodeId -> outputKey -> ValueJSON.
   * (One batched wasm call.)
   */
  evalAll(): EvalResult {
    const raw = this.inner.eval_all(); // JSON string
    const parsed = JSON.parse(raw) as EvalResult;
    return parsed;
  }

  /**
   * Update a node parameter by key (e.g., "value", "frequency", "phase", "min", "max",
   * "stiffness", "damping", "half_life", "max_rate").
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
 * Normalize a graph specification (object or JSON string) using the shared
 * Rust-side normalization logic. Helpful for persisting specs or diffing.
 */
export async function normalizeGraphSpec(
  spec: GraphSpec | string
): Promise<GraphSpec> {
  await init();
  const json = typeof spec === "string" ? spec : JSON.stringify(spec);
  const mod = await loadBindings();
  const normalizedJson = mod.normalize_graph_spec_json(json);
  return JSON.parse(normalizedJson) as GraphSpec;
}

/**
 * Fetch the node schema registry from the wasm module as a parsed object.
 * Ensures the wasm module is initialized before calling.
 */
export async function getNodeSchemas(): Promise<import("./types").Registry> {
  await init();
  const mod = await loadBindings();
  const raw = mod.get_node_schemas_json();
  return JSON.parse(raw) as import("./types").Registry;
}

// Samples re-exports
export {
  graphSamples,
  oscillatorBasics,
  vectorPlayground,
  logicGate,
  tupleSpringDampSlew,
  nestedTelemetry,
  nestedRigWeightedPose,
  selectorCascade,
  hierarchicalBlend,
  weightedAverage,
  layeredRigBlend,
} from "./samples.js";
