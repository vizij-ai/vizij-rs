// Stable ESM entry for @vizij/node-graph-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
// Adjust the import path if your pkg name differs.
import { loadBindings as loadWasmBindings, type InitInput as LoaderInitInput } from "@vizij/wasm-loader";
import { toValueJSON, type ValueJSON, type ValueInput } from "@vizij/value-json";
import type {
  NodeId,
  NodeType,
  NodeParams,
  NodeSpec,
  GraphSpec,
  GraphOutputs,
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
  PortSnapshot,
  EvalResult,
  WriteOpJSON,
  ShapeJSON,
  ParamSpec,
  Registry,
};

export {
  toValueJSON,
  isNormalizedValue,
  valueAsNumber,
  valueAsNumericArray,
  valueAsTransform,
  valueAsVec3,
  valueAsVector,
  valueAsBool,
  valueAsQuat,
  valueAsColorRgba,
  valueAsText,
} from "@vizij/value-json";

// --- wasm loader ---

type WasmGraphCtor = new () => any;

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  WasmGraph: WasmGraphCtor;
  normalize_graph_spec_json: (json: string) => string;
  get_node_schemas_json: () => string;
  abi_version: () => number;
}

const bindingCache: { current: WasmBindings | null } = { current: null };

function pkgWasmJsUrl(): URL {
  // Resolve package-local pkg/ for both src/ and dist/src/ callers
  return new URL("../../pkg/vizij_graph_wasm.js", import.meta.url);
}

function defaultWasmUrl(): URL {
  return new URL("../../pkg/vizij_graph_wasm_bg.wasm", import.meta.url);
}

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await loadWasmBindings<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => import(/* @vite-ignore */ pkgWasmJsUrl().toString()),
      defaultWasmUrl,
      init: async (module: unknown, initArg: unknown) => {
        const typed = module as WasmBindings;
        await typed.default(initArg);
      },
      getBindings: (module: unknown) => module as WasmBindings,
      expectedAbi: 2,
      getAbiVersion: (bindings) => Number(bindings.abi_version()),
    },
    input
  );

  return bindingCache.current!;
}

export type InitInput = LoaderInitInput;

export function abi_version(): number {
  if (!bindingCache.current) {
    throw new Error("Call init() from @vizij/node-graph-wasm before reading abi_version().");
  }
  return Number(bindingCache.current.abi_version());
}

// --- init() ---

let _initPromise: Promise<void> | null = null;

/**
 * Initialize the wasm module once.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;

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
  if (!bindingCache.current) {
    throw new Error("WASM bindings were not initialized correctly.");
  }
}

// --- Value helpers ---

export type Value = ValueInput;

// --- Public API class ---

/**
 * Ergonomic wrapper around wasm WasmGraph.
 * Always await init() once before constructing.
 */
export class Graph {
  private inner: any;

  constructor() {
    ensureInited();
    const bindings = bindingCache.current!;
    const WasmGraph = bindings.WasmGraph;
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
export async function getNodeSchemas(): Promise<Registry> {
  await init();
  const mod = await loadBindings();
  const raw = mod.get_node_schemas_json();
  return JSON.parse(raw) as Registry;
}

// Samples re-exports
import {
  graphSamples as baseGraphSamples,
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
import { urdfGraphSamples, urdfIkPosition } from "./samples_extra.js";

export const graphSamples: Record<string, GraphSpec> = {
  ...baseGraphSamples,
  ...urdfGraphSamples,
};

export {
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
  urdfIkPosition,
};
