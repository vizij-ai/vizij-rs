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
        const initFn =
          (typed as any)?.default && typeof (typed as any).default === "function"
            ? (typed as any).default
            : typeof (typed as any) === "function"
              ? (typed as any)
              : null;
        if (initFn) {
          await initFn(initArg);
        } // CJS/node target exports are already initialised if no init fn
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

/**
 * Read the wasm ABI version after init() has completed.
 */
export function abi_version(): number {
  if (!bindingCache.current) {
    throw new Error("Call init() from @vizij/node-graph-wasm before reading abi_version().");
  }
  return Number(bindingCache.current.abi_version());
}

// --- init() ---

let _initPromise: Promise<void> | null = null;

/**
 * Initialize the wasm module once. Must be awaited before constructing Graph.
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

function parseVersion(v: unknown, fallback: bigint = 0n): bigint {
  if (typeof v === "bigint") return v;
  if (typeof v === "number" && Number.isFinite(v)) return BigInt(Math.trunc(v));
  if (typeof v === "string") {
    try {
      return BigInt(v);
    } catch {
      const n = Number(v);
      if (Number.isFinite(n)) return BigInt(Math.trunc(n));
    }
  }
  return fallback;
}

function mergeDelta(base: EvalResult, delta: EvalResult & { version?: unknown }): EvalResult {
  const mergedNodes: Record<string, any> = base.nodes ? JSON.parse(JSON.stringify(base.nodes)) : {};
  if (delta.nodes) {
    for (const [nodeId, ports] of Object.entries(delta.nodes as Record<string, any>)) {
      const existing = mergedNodes[nodeId] ?? {};
      for (const [portKey, payload] of Object.entries(ports as Record<string, any>)) {
        const val = (payload as any)?.value;
        const shape = (payload as any)?.shape;
        if (val === null && shape === null) {
          delete existing[portKey];
        } else {
          existing[portKey] = payload;
        }
      }
      mergedNodes[nodeId] = existing;
    }
  }
  const merged: EvalResult = {
    nodes: mergedNodes,
    writes: delta.writes ?? [],
  } as EvalResult;
  (merged as any).version = delta.version ?? (base as any).version;
  return merged;
}

// --- Public API class ---

/**
 * Ergonomic wrapper around wasm WasmGraph.
 * Always await init() once before constructing.
 */
export class Graph {
  private inner: any;
  private lastEvalResult: EvalResult | null = null;
  private _lastSlotValues?: Map<number, number>;
  private _hotPathToSlot?: Map<string, number>;
  private _hotIndices?: Uint32Array;
  private _hotEpsilon: number = 0;
  private _autoClearDroppedHotPaths = false;
  private _slotDiffWarm = false;
  private _lastOutputVersion: bigint = 0n;
  private _baselineCaptured = false;
  private _lastSlotDiffChanged = false;
  private _prevHotStaged?: Set<number>;
  private _debugLogging = false;
  private _outputsDirty = true;

  /**
   * Mark cached outputs as dirty; optionally reset the baseline/version so the next eval forces a full snapshot.
   * Use resetBaseline=true only when the graph structure changes.
   */
  private invalidateCachedOutputs(resetBaseline: boolean = false): void {
    if (resetBaseline) {
      this.lastEvalResult = null;
      this._lastOutputVersion = 0n;
      this._baselineCaptured = false;
    }
    this._outputsDirty = true;
  }

  constructor() {
    ensureInited();
    const bindings = bindingCache.current!;
    const WasmGraph = bindings.WasmGraph;
    this.inner = new WasmGraph();
  }

  /**
   * Load a graph spec (object or JSON string).
   * Optional opts allow hot-path registration in one call.
   */
  loadGraph(
    spec: GraphSpec | string,
    opts?: { hotPaths?: string[]; epsilon?: number; autoClearDroppedHotPaths?: boolean }
  ): void {
    this.invalidateCachedOutputs(true);
    this._lastSlotValues = undefined;
    // Preserve previous hot registration so we can clear dropped slots if requested.
    const prevHotMap = this._hotPathToSlot;
    const prevIndices = this._hotIndices;
    this._hotPathToSlot = undefined;
    this._hotIndices = undefined;
    this._slotDiffWarm = false;
    const json = typeof spec === "string" ? spec : JSON.stringify(spec);
    this.inner.load_graph(json);
    if (opts?.hotPaths && opts.hotPaths.length) {
      if (opts.autoClearDroppedHotPaths && prevHotMap && prevIndices) {
        const nextSet = new Set(opts.hotPaths);
        for (const [path, slot] of prevHotMap.entries()) {
          if (!nextSet.has(path)) {
            try {
              (this.inner as any).clear_input_slot?.(slot);
              const lastSlotValues = this._lastSlotValues as Map<number, number> | undefined;
              lastSlotValues?.delete(slot);
            } catch {
              /* best-effort */
            }
          }
        }
      }
      this.setHotPaths(opts.hotPaths, {
        epsilon: opts.epsilon,
        autoClearDroppedHotPaths: opts.autoClearDroppedHotPaths,
      });
    }
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
    // Preferred path: single-call slots + delta when available.
    if (typeof target.eval_all_slots_delta === "function") {
      const versionBefore = this._lastOutputVersion;
      const deltaRaw = target.eval_all_slots_delta(versionBefore ?? 0n);
      const delta =
        typeof deltaRaw === "string"
          ? (JSON.parse(deltaRaw) as EvalResult & { version?: number | string | bigint })
          : (deltaRaw as EvalResult & { version?: number | string | bigint });
      const merged = mergeDelta(this.lastEvalResult ?? { nodes: {}, writes: [] }, delta);
      this._lastOutputVersion = parseVersion(delta.version ?? versionBefore);
      this.lastEvalResult = merged;
      this._baselineCaptured = true;
      this._outputsDirty = false;
      return merged;
    }
    // Fallback path: two-call slots + delta after first full snapshot.
    if (
      typeof target.eval_all_slots === "function" &&
      typeof target.get_outputs_full === "function" &&
      typeof target.get_outputs_delta === "function"
    ) {
      if (!this._baselineCaptured) {
        target.eval_all_slots();
        const full = target.get_outputs_full();
        const parsed =
          typeof full === "string"
            ? (JSON.parse(full) as EvalResult & { version?: number | string | bigint })
            : (full as EvalResult & { version?: number | string | bigint });
        this.lastEvalResult = parsed;
        this._lastOutputVersion = parseVersion(parsed?.version);
        this._baselineCaptured = true;
        this._outputsDirty = false;
        return parsed;
      }
      const versionBefore = this._lastOutputVersion;
      target.eval_all_slots();
      const deltaRaw = target.get_outputs_delta(versionBefore ?? 0n);
      const delta =
        typeof deltaRaw === "string"
          ? (JSON.parse(deltaRaw) as EvalResult & { version?: number | string | bigint })
          : (deltaRaw as EvalResult & { version?: number | string | bigint });
      const merged = mergeDelta(this.lastEvalResult ?? { nodes: {}, writes: [] }, delta);
      this._lastOutputVersion = parseVersion(delta.version ?? versionBefore);
      this.lastEvalResult = merged;
      this._outputsDirty = false;
      return merged;
    }

    const result =
      typeof target.eval_all_js === "function"
        ? target.eval_all_js()
        : target.eval_all();
    const parsed =
      typeof result === "string"
        ? (JSON.parse(result) as EvalResult)
        : (result as EvalResult);
    this.lastEvalResult = parsed;
    this._outputsDirty = false;
    return parsed;
  }

  /**
   * Force a full snapshot and reset the delta baseline (e.g., after structural edits).
   */
  evalAllFull(): EvalResult {
    const target = this.inner as any;
    this._baselineCaptured = false;
    this._lastOutputVersion = 0n;
    this._outputsDirty = true;
    if (typeof target.eval_all_slots_delta === "function") {
      const full = target.eval_all_slots_delta(0);
      const parsed =
        typeof full === "string"
          ? (JSON.parse(full) as EvalResult & { version?: number | string | bigint })
          : (full as EvalResult & { version?: number | string | bigint });
      this.lastEvalResult = parsed;
      this._lastOutputVersion = parseVersion(parsed?.version);
      this._baselineCaptured = true;
      this._outputsDirty = false;
      return parsed;
    }
    if (
      typeof target.eval_all_slots === "function" &&
      typeof target.get_outputs_full === "function"
    ) {
      target.eval_all_slots();
      const full = target.get_outputs_full();
      const parsed =
        typeof full === "string"
          ? (JSON.parse(full) as EvalResult & { version?: number | string | bigint })
          : (full as EvalResult & { version?: number | string | bigint });
      this.lastEvalResult = parsed;
      this._lastOutputVersion = parseVersion(parsed?.version);
      this._baselineCaptured = true;
      this._outputsDirty = false;
      return parsed;
    }
    return this.evalAll();
  }

  /**
   * Unified staging entry: automatically routes hot paths through slots (with diff) and others via path batch.
   * Accepts numeric values; non-hot or unsupported types go through path staging.
   */
  stageInputs(
    paths: string[],
    values: Float32Array,
    shapes?: Array<ShapeJSON | null>
  ): void {
    const hotMap = this._hotPathToSlot;
    const stagedHot = new Set<number>();
    const hotIdx: number[] = [];
    const hotVals: number[] = [];
    const coldPaths: string[] = [];
    const coldVals: number[] = [];
    const coldShapes: Array<ShapeJSON | null | undefined> = [];
    const fallbackHot: string[] = [];
    for (let i = 0; i < paths.length; i += 1) {
      const p = paths[i];
      const v = values[i];
      const shape = shapes ? shapes[i] : undefined;
      const slot = hotMap?.get(p);
      const numeric = Number.isFinite(v);
      if (slot !== undefined && numeric && shape == null) {
        hotIdx.push(slot);
        hotVals.push(v);
        stagedHot.add(slot);
      } else {
        if (slot !== undefined && (!numeric || shape != null)) {
          fallbackHot.push(p);
        }
        coldPaths.push(p);
        coldVals.push(v);
        coldShapes.push(shape ?? null);
      }
    }
    // Stage cold first (or vice versa) but keep consistent ordering; choose cold then hot so hot can override if needed.
    if (coldPaths.length) {
      this.invalidateCachedOutputs();
      const target = this.inner as any;
      const hasBatch = typeof target.stage_inputs_batch === "function";
      const batchPaths: string[] = [];
      const batchVals: number[] = [];
      for (let i = 0; i < coldPaths.length; i += 1) {
        const val = coldVals[i];
        const shape = coldShapes[i];
        const canBatch = hasBatch && shape == null && Number.isFinite(val);
        if (canBatch) {
          batchPaths.push(coldPaths[i]);
          batchVals.push(val);
        } else {
          this.stageInput(coldPaths[i], val, shape ?? undefined);
        }
      }
      if (batchPaths.length) {
        target.stage_inputs_batch(batchPaths, Float32Array.from(batchVals));
      }
    }
    if (hotIdx.length) {
      const idxArray = Uint32Array.from(hotIdx);
      const valArray = Float32Array.from(hotVals);
      if (this._slotDiffWarm) {
        this._lastSlotDiffChanged = false;
        this.stageInputsBySlotDiff(idxArray, valArray, this._hotEpsilon);
        if (this._lastSlotDiffChanged) {
          this._outputsDirty = true;
        }
      } else {
        this.stageInputsBySlot(idxArray, valArray);
        this._slotDiffWarm = true;
      }
    }
    // Auto-clear dropped hot paths if enabled
    if (this._autoClearDroppedHotPaths && this._hotIndices) {
      const prev = this._prevHotStaged ?? new Set<number>();
      for (const slot of prev) {
        if (!stagedHot.has(slot)) {
          this.clearSlot(slot).catch(() => {});
        }
      }
      this._prevHotStaged = stagedHot;
    }
    if (this._debugLogging) {
      console.debug("[graph] stageInputs", {
        hotSent: hotIdx.length,
        coldSent: coldPaths.length,
        fallbackHot,
        slotDiff: this._slotDiffWarm,
      });
    } else if (fallbackHot.length) {
      console.warn(
        `[graph] stageInputs fell back to path staging for ${fallbackHot.length} hot paths (non-numeric or shaped payloads); hot paths: ${fallbackHot.slice(0, 5).join(", ")}${fallbackHot.length > 5 ? "…" : ""}`
      );
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
   * Stage inputs by slot but only send indices whose values differ from the last call.
   * Optionally supply an epsilon for float comparisons (default exact).
   */
  stageInputsBySlotDiff(
    indices: Uint32Array,
    values: Float32Array,
    epsilon = 0
  ): void {
    this._lastSlotDiffChanged = false;
    if (indices.length !== values.length) {
      throw new Error("stageInputsBySlotDiff: indices and values length mismatch");
    }
    if (typeof this._lastSlotValues === "undefined") {
      this._lastSlotValues = new Map<number, number>();
    }
    const cache = this._lastSlotValues;
    const changedIdx: number[] = [];
    const changedVals: number[] = [];
    for (let i = 0; i < values.length; i += 1) {
      const idx = indices[i];
      const prev = cache.has(idx) ? cache.get(idx)! : undefined;
      const cur = values[i];
      const isNew = typeof prev === "undefined";
      const differs = isNew
        ? true
        : epsilon === 0
          ? cur !== prev
          : Math.abs(cur - prev) > epsilon;

      if (differs) {
        changedIdx.push(idx);
        changedVals.push(cur);
      }
      cache.set(idx, cur);
    }

    if (changedIdx.length === 0) return;
    this._lastSlotDiffChanged = true;
    this.invalidateCachedOutputs();
    this.stageInputsBySlot(
      Uint32Array.from(changedIdx),
      Float32Array.from(changedVals)
    );
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
    const res =
      this._outputsDirty || !this.lastEvalResult ? this.evalAll() : this.lastEvalResult;
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
   * Fetch only the outputs that changed since a version token. Returns { version, nodes, writes }.
   * Pass version 0 (or undefined) to force a full snapshot on first call.
   */
  getOutputsDelta(sinceVersion?: number): EvalResult & { version: number } {
    const target = this.inner as any;
    const versionArg = sinceVersion ?? 0;
    const haveBaseline = !!this.lastEvalResult && this._baselineCaptured;
    const parseDelta = (delta: any) =>
      typeof delta === "string"
        ? (JSON.parse(delta) as EvalResult & { version: number })
        : (delta as EvalResult & { version: number });
    const applyParsed = (parsed: EvalResult & { version?: unknown }) => {
      const parsedVersion = parseVersion(parsed.version ?? this._lastOutputVersion);
      const sinceBig = BigInt(Math.trunc(versionArg));
      const isFullSnapshot = (parsed as any).full === true;
      const baselineMatches = haveBaseline && sinceBig === this._lastOutputVersion && !isFullSnapshot;
      this.lastEvalResult = baselineMatches
        ? mergeDelta(this.lastEvalResult ?? { nodes: {}, writes: [] }, parsed)
        : (parsed as EvalResult); // replace when baselines differ or runtime sent full snapshot
      this._lastOutputVersion = parsedVersion;
      this._baselineCaptured = true;
      this._outputsDirty = false;
      return parsed as EvalResult & { version: number };
    };

    // Fast, read-only path: serialize the cached frame without re-evaluating.
    if (!this._outputsDirty && haveBaseline && typeof target.get_outputs_delta === "function") {
      const parsed = parseDelta(target.get_outputs_delta(versionArg));
      return applyParsed(parsed);
    }

    // Dirty or missing baseline: evaluate once and return the delta for the caller's baseline.
    if (typeof target.eval_all_slots_delta === "function") {
      const parsed = parseDelta(target.eval_all_slots_delta(versionArg));
      return applyParsed(parsed);
    }

    if (typeof target.eval_all_slots === "function" && typeof target.get_outputs_delta === "function") {
      target.eval_all_slots();
      const parsed = parseDelta(target.get_outputs_delta(versionArg));
      return applyParsed(parsed);
    }

    // Fallback when only JS snapshotting is possible.
    if (haveBaseline) {
      const currentVersion = Number(this._lastOutputVersion ?? 0n);
      return { ...(this.lastEvalResult as EvalResult), version: currentVersion };
    }

    const full = this.evalAll();
    (full as any).version = (full as any).version ?? 0;
    return full as any;
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
   * Update a node parameter by key (e.g., "value", "frequency", "phase", "min", "max").
   * Value may be number | boolean | vec3 or a pre-encoded ValueJSON.
   *
   * Structural edits (e.g., Split sizes) cause the wasm runtime to drop its plan and
   * delta caches; reset the JS baseline so the next eval consumes the fresh snapshot.
   */
  setParam(nodeId: string, key: string, value: Value): void {
    // Plan cache is invalidated in wasm for structural params; drop the JS baseline too
    // so the next delta merge starts from a fresh full snapshot.
    this.invalidateCachedOutputs(true);
    const payload = toValueJSON(value);
    const target = this.inner as any;
    if (typeof target.set_param_value === "function") {
      target.set_param_value(nodeId, key, payload);
    } else {
      target.set_param(nodeId, key, JSON.stringify(payload));
    }
  }

  // --- New unified hot-path helpers ---

  /**
   * Declare hot paths for slot-based staging. Can be called after loadGraph; re-registers slots.
   */
  setHotPaths(paths: string[], opts?: { epsilon?: number; autoClearDroppedHotPaths?: boolean }): void {
    const target = this.inner as any;
    if (typeof target.register_input_paths !== "function") {
      throw new Error("hotPaths: register_input_paths not available on wasm binding");
    }
    const HOT_PATH_WARN_THRESHOLD = 10_000;
    if (paths.length > HOT_PATH_WARN_THRESHOLD) {
      console.warn(
        `[graph] setHotPaths: registering ${paths.length} hot paths; this may increase memory and setup time`
      );
    }
    const prevHotMap = this._hotPathToSlot;
    const prevIndices = this._hotIndices;
    const idx = this.registerInputPaths(paths);
    this.prepareInputSlots(idx);
    const map = new Map<string, number>();
    paths.forEach((p, i) => map.set(p, idx[i]));
    this._hotPathToSlot = map;
    this._hotIndices = idx;
    this._hotEpsilon = opts?.epsilon ?? 0;
    this._autoClearDroppedHotPaths = Boolean(opts?.autoClearDroppedHotPaths);
    this._slotDiffWarm = false;
    this._prevHotStaged = undefined;
    if (this._autoClearDroppedHotPaths && prevHotMap && prevIndices) {
      const nextSet = new Set(paths);
      for (const [path, slot] of prevHotMap.entries()) {
        if (!nextSet.has(path)) {
          try {
            target.clear_input_slot?.(slot);
            this._lastSlotValues?.delete(slot);
          } catch {
            /* best-effort */
          }
        }
      }
    }
    if (this._debugLogging) {
      console.debug("[graph] hot paths registered", {
        count: paths.length,
        epsilon: this._hotEpsilon,
        autoClearDroppedHotPaths: this._autoClearDroppedHotPaths,
      });
    }
  }

  /**
   * Clear a cached/staged slot value (allows node defaults to apply on next eval).
   */
  async clearSlot(slotIdx: number): Promise<void> {
    const target = this.inner as any;
    if (typeof target.clear_input_slot === "function") {
      target.clear_input_slot(slotIdx);
    } else {
      // Fallback: stage nothing; no-op
    }
    if (this._lastSlotValues) {
      this._lastSlotValues.delete(slotIdx);
    }
    this._outputsDirty = true;
    if (this._debugLogging) {
      console.debug("[graph] clearSlot", { slotIdx });
    }
  }

  /**
   * Clear a staged input by path.
   */
  async clearInput(path: string): Promise<void> {
    const target = this.inner as any;
    if (typeof target.clear_input_path === "function") {
      target.clear_input_path(path);
    } else {
      // Fallback: stage undefined (will serialize) — avoid heavy fallback; instead drop cache only.
    }
    // Best-effort cleanup of slot diff cache
    if (this._hotPathToSlot && this._lastSlotValues) {
      const slot = this._hotPathToSlot.get(path);
      if (slot !== undefined) {
        this._lastSlotValues.delete(slot);
      }
    }
    this._outputsDirty = true;
    if (this._debugLogging) {
      console.debug("[graph] clearInput", { path });
    }
  }

  /** Enable or disable verbose staging/eval logs for debugging. */
  setDebugLogging(enabled: boolean): void {
    this._debugLogging = enabled;
  }

  /** Inspect current staging state (hot paths, versions, flags). */
  inspectStaging(): {
    hotPaths: string[];
    epsilon: number;
    autoClearDroppedHotPaths: boolean;
    slotDiffWarm: boolean;
    lastOutputVersion: bigint;
    debugLogging: boolean;
  } {
    return {
      hotPaths: this._hotPathToSlot ? Array.from(this._hotPathToSlot.keys()) : [],
      epsilon: this._hotEpsilon,
      autoClearDroppedHotPaths: this._autoClearDroppedHotPaths,
      slotDiffWarm: this._slotDiffWarm,
      lastOutputVersion: this._lastOutputVersion,
      debugLogging: this._debugLogging,
    };
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
/**
 * Convenience helper to init() and return a ready Graph instance.
 */
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
