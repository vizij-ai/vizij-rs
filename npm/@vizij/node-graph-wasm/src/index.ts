// Stable ESM entry for @vizij/node-graph-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
// Adjust the import path if your pkg name differs.
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";
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
  PortSpec,
  VariadicSpec,
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
let wasmModulePromise: Promise<WasmBindings | unknown> | null = null;
let wasmUrlCache: string | null = null;

function pkgWasmJsUrl(): URL {
  // Resolve package-local pkg/ for both src/ and dist/src/ callers
  return new URL("../../pkg/vizij_graph_wasm.js", import.meta.url);
}

function importStaticWasmModule(): Promise<unknown> {
  return import("../../pkg/vizij_graph_wasm.js");
}

function importDynamicWasmModule(): Promise<unknown> {
  return import(/* @vite-ignore */ pkgWasmJsUrl().toString());
}

async function importWasmModule(): Promise<unknown> {
  if (!wasmModulePromise) {
    wasmModulePromise = importStaticWasmModule().catch((err) => {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn(
          "@vizij/node-graph-wasm: static wasm import failed, falling back to runtime URL import.",
          err
        );
      }
      return importDynamicWasmModule();
    });
  }
  return wasmModulePromise;
}

function defaultWasmUrl(): string {
  if (!wasmUrlCache) {
    wasmUrlCache = new URL("../../pkg/vizij_graph_wasm_bg.wasm", import.meta.url).toString();
  }
  return wasmUrlCache;
}

const loadBindingsImpl =
  typeof window === "undefined"
    ? loadWasmBindings
    : (loadWasmBindingsBrowser as typeof loadWasmBindings);

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await loadBindingsImpl<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => importWasmModule(),
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
  private lastEvalResult: EvalResult | null = null;

  private invalidateCachedOutputs(): void {
    this.lastEvalResult = null;
  }

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
    this.invalidateCachedOutputs();
    const json = typeof spec === "string" ? spec : JSON.stringify(spec);
    this.inner.load_graph(json);
  }

  /** Set absolute time (seconds). */
  setTime(t: number): void {
    this.invalidateCachedOutputs();
    this.inner.set_time(t);
  }

  /** Increment time (seconds). */
  step(dt: number): void {
    this.invalidateCachedOutputs();
    this.inner.step(dt);
  }

  /**
   * Stage a host-provided input for the next evalAll() tick.
   * Values staged before evalAll will be visible in that evaluation (epoch semantics).
   */
  stageInput(path: string, value: Value, declaredShape?: ShapeJSON): void {
    this.invalidateCachedOutputs();
    const payload = toValueJSON(value);
    const target = this.inner as any;
    if (typeof target.stage_input_value === "function") {
      target.stage_input_value(path, payload, declaredShape);
    } else {
      const shapeStr = declaredShape ? JSON.stringify(declaredShape) : undefined;
      target.stage_input(path, JSON.stringify(payload), shapeStr);
    }
  }

  /**
   * Evaluate the whole graph and return a map of nodeId -> outputKey -> ValueJSON.
   * (One batched wasm call.)
   */
  evalAll(): EvalResult {
    const target = this.inner as any;
    const result =
      typeof target.eval_all_js === "function"
        ? target.eval_all_js()
        : target.eval_all();
    const parsed =
      typeof result === "string"
        ? (JSON.parse(result) as EvalResult)
        : (result as EvalResult);
    this.lastEvalResult = parsed;
    return parsed;
  }

  /**
   * Batch-stage numeric inputs (paths[i] -> values[i]) in one wasm call.
   * Paths length must match values length.
   */
  stageInputs(paths: string[], values: Float32Array): void {
    this.invalidateCachedOutputs();
    const target = this.inner as any;
    if (typeof target.stage_inputs_batch === "function") {
      target.stage_inputs_batch(paths, values);
    } else {
      // Fallback: per-input staging if older wasm is loaded
      for (let i = 0; i < paths.length; i += 1) {
        this.stageInput(paths[i], values[i]);
      }
    }
  }

  /**
   * Register paths once and reuse their indices for faster staging.
   */
  registerInputPaths(paths: string[]): Uint32Array {
    const target = this.inner as any;
    if (typeof target.register_input_paths !== "function") {
      throw new Error("register_input_paths not available on wasm binding");
    }
    return target.register_input_paths(paths);
  }

  prepareInputSlots(indices: Uint32Array, declaredShapes?: Array<ShapeJSON | null>): void {
    const target = this.inner as any;
    if (typeof target.prepare_input_slots !== "function") {
      throw new Error("prepare_input_slots not available on wasm binding");
    }
    // declaredShapes may be undefined => treat as nulls
    const payload =
      declaredShapes ?? new Array(indices.length).fill(null);
    target.prepare_input_slots(indices, payload);
  }

  /**
   * Stage inputs using indices returned by registerInputPaths.
   * Length of indices must match values length.
   */
  stageInputsByIndex(indices: Uint32Array, values: Float32Array): void {
    this.invalidateCachedOutputs();
    const target = this.inner as any;
    if (typeof target.stage_inputs_indices === "function") {
      target.stage_inputs_indices(indices, values);
    } else {
      throw new Error("stage_inputs_indices not available on wasm binding");
    }
  }

  /**
   * Stage inputs using pre-prepared slots (no path parse, reuse declared shapes).
   */
  stageInputsBySlot(indices: Uint32Array, values: Float32Array): void {
    this.invalidateCachedOutputs();
    const target = this.inner as any;
    if (typeof target.stage_inputs_slots === "function") {
      target.stage_inputs_slots(indices, values);
    } else {
      throw new Error("stage_inputs_slots not available on wasm binding");
    }
  }

  /**
   * Fetch the default 'out' port for many nodes as a Float32Array.
   * Non-scalar outputs return NaN for that entry.
   */
  getOutputsBatch(nodeIds: string[]): Float32Array {
    const target = this.inner as any;
    if (typeof target.get_outputs_batch === "function") {
      return target.get_outputs_batch(nodeIds);
    }
    // Fallback: map over last eval result without re-running the graph
    const res = this.lastEvalResult ?? this.evalAll();
    const out = new Float32Array(nodeIds.length);
    for (let i = 0; i < nodeIds.length; i += 1) {
      const nid = nodeIds[i];
      const port = (res.nodes as any)?.[nid]?.out?.value;
      const val =
        typeof port?.data === "number"
          ? port.data
          : Array.isArray(port?.data) && port.data.length > 0
            ? port.data[0]
            : Number.NaN;
      out[i] = val;
    }
    return out;
  }

  /**
   * Run N steps of fixed dt inside WASM, returning the final frame.
   * This amortizes JS/WASM crossings for tight loops.
   */
  evalSteps(steps: number, dt: number): EvalResult {
    const target = this.inner as any;
    const result =
      typeof target.eval_steps_js === "function"
        ? target.eval_steps_js(steps, dt)
        : (() => {
            let last: EvalResult | undefined;
            for (let i = 0; i < Math.max(1, steps); i += 1) {
              this.step(dt);
              last = this.evalAll();
            }
            return last!;
          })();
    const parsed =
      typeof result === "string"
        ? (JSON.parse(result) as EvalResult)
        : (result as EvalResult);
    this.lastEvalResult = parsed;
    return parsed;
  }

  /**
   * Update a node parameter by key (e.g., "value", "frequency", "phase", "min", "max",
   * "stiffness", "damping", "half_life", "max_rate").
   * Value may be number | boolean | vec3 or a pre-encoded ValueJSON.
   */
  setParam(nodeId: string, key: string, value: Value): void {
    this.invalidateCachedOutputs();
    const payload = toValueJSON(value);
    const target = this.inner as any;
    if (typeof target.set_param_value === "function") {
      target.set_param_value(nodeId, key, payload);
    } else {
      target.set_param(nodeId, key, JSON.stringify(payload));
    }
  }
}

export {
  listNodeGraphFixtures,
  loadNodeGraphBundle,
  loadNodeGraphSpec,
  loadNodeGraphSpecJson,
  loadNodeGraphStage,
} from "./fixtures.js";

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

function describePort(port: PortSpec): string {
  const status = port.optional ? "optional" : "required";
  const doc = port.doc && port.doc.trim().length > 0 ? ` — ${port.doc}` : "";
  return `    • ${port.label} [${port.id}:${port.ty}] (${status})${doc}`;
}

function describeParam(param: ParamSpec): string {
  const bounds: string[] = [];
  if (typeof param.min === "number") bounds.push(`min ${param.min}`);
  if (typeof param.max === "number") bounds.push(`max ${param.max}`);
  const defaultVal =
    param.default_json !== undefined
      ? `default ${JSON.stringify(param.default_json)}`
      : undefined;
  if (defaultVal) bounds.push(defaultVal);
  const extra = bounds.length ? ` (${bounds.join(", ")})` : "";
  const doc = param.doc && param.doc.trim().length > 0 ? ` — ${param.doc}` : "";
  return `    • ${param.label} [${param.id}:${param.ty}]${extra}${doc}`;
}

/**
 * Pretty-print schema documentation for all nodes or a specific node type.
 *
 * @example
 * await logNodeSchemaDocs();                   // logs every node
 * await logNodeSchemaDocs("spring");           // logs only the Spring node (NodeType)
 */
export async function logNodeSchemaDocs(node?: NodeType | string): Promise<void> {
  const registry = await getNodeSchemas();
  const target = node?.toString().toLowerCase();
  const nodes = target
    ? registry.nodes.filter((entry) => entry.type_id === target)
    : registry.nodes.slice();

  if (!nodes.length) {
    console.warn(
      target
        ? `No node schema found for '${target}'.`
        : "Node schema registry is empty."
    );
    return;
  }

  nodes.sort((a, b) => a.name.localeCompare(b.name));

  for (const entry of nodes) {
    const lines: string[] = [];
    const headerDoc =
      entry.doc && entry.doc.trim().length > 0 ? entry.doc : "(no description)";
    lines.push(`\n${entry.name} (${entry.type_id}) — ${headerDoc}`);
    lines.push(`  Category: ${entry.category}`);

    if (entry.inputs.length || entry.variadic_inputs) {
      lines.push("  Inputs:");
      for (const port of entry.inputs) {
        lines.push(describePort(port));
      }
      if (entry.variadic_inputs) {
        const variadic: VariadicSpec = entry.variadic_inputs;
        const doc =
          variadic.doc && variadic.doc.trim().length > 0
            ? ` — ${variadic.doc}`
            : "";
        const max = variadic.max != null ? variadic.max : "∞";
        lines.push(
          `    • ${variadic.label} [${variadic.id}:${variadic.ty}] (variadic ${variadic.min}-${max})${doc}`
        );
      }
    }

    if (entry.outputs.length || entry.variadic_outputs) {
      lines.push("  Outputs:");
      for (const port of entry.outputs) {
        lines.push(describePort(port));
      }
      if (entry.variadic_outputs) {
        const variadic: VariadicSpec = entry.variadic_outputs;
        const doc =
          variadic.doc && variadic.doc.trim().length > 0
            ? ` — ${variadic.doc}`
            : "";
        const max = variadic.max != null ? variadic.max : "∞";
        lines.push(
          `    • ${variadic.label} [${variadic.id}:${variadic.ty}] (variadic ${variadic.min}-${max})${doc}`
        );
      }
    }

    if (entry.params.length) {
      lines.push("  Params:");
      for (const param of entry.params) {
        lines.push(describeParam(param));
      }
    }

    console.log(lines.join("\n"));
  }
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

export {
  getNodeRegistry,
  findNodeSignature,
  requireNodeSignature,
  listNodeTypeIds,
  groupNodeSignaturesByCategory,
  nodeRegistryVersion,
} from "./metadata/index.js";
