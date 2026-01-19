// Stable ESM entry for @vizij/orchestrator-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
import type {
  AnimationRegistrationConfig,
  AnimationSetup,
  GraphRegistrationInput,
  GraphSubscriptions,
  MergedGraphRegistrationConfig,
  MergeStrategyOptions,
  MergeConflictStrategy,
  OrchestratorFrame,
  ValueJSON,
  ShapeJSON,
  WriteOpJSON,
  ConflictLog,
  GraphRegistrationConfig,
  GraphReplaceConfig,
} from "./types";
import { toValueJSON, type ValueInput } from "@vizij/value-json";
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";
type WasmResolver = (path: string) => string | number | null | undefined;

interface WasmOrchestratorInstance {
  register_graph(cfg: GraphRegistrationConfig | string): string;
  replace_graph(cfg: GraphReplaceConfig): void;
  register_merged_graph(cfg: MergedGraphRegistrationConfig): string;
  register_animation(cfg: AnimationRegistrationConfig): string;
  prebind(resolver: WasmResolver): void;
  set_input(path: string, value: ValueJSON, shape?: ShapeJSON): void;
  remove_input(path: string): boolean;
  step(dt: number): OrchestratorFrame;
  list_controllers(): { graphs?: string[]; anims?: string[] } | undefined | null;
  remove_graph(id: string): boolean;
  remove_animation(id: string): boolean;
}

interface WasmOrchestratorCtor {
  new (opts?: unknown): WasmOrchestratorInstance;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  VizijOrchestrator: WasmOrchestratorCtor;
  normalize_graph_spec_json: (json: string) => string;
  abi_version: () => number;
}

const bindingCache: { current: WasmBindings | null } = { current: null };
let wasmModulePromise: Promise<WasmBindings | unknown> | null = null;
let wasmUrlCache: string | null = null;

function pkgWasmJsUrl(): URL {
  return new URL("../../pkg/vizij_orchestrator_wasm.js", import.meta.url);
}

function importStaticWasmModule(): Promise<unknown> {
  return import("../../pkg/vizij_orchestrator_wasm.js");
}

function importDynamicWasmModule(): Promise<unknown> {
  return import(/* @vite-ignore */ pkgWasmJsUrl().toString());
}

async function importWasmModule(): Promise<unknown> {
  if (!wasmModulePromise) {
    wasmModulePromise = importStaticWasmModule().catch((err) => {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn(
          "@vizij/orchestrator-wasm: static wasm import failed, falling back to runtime URL import.",
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
    wasmUrlCache = new URL("../../pkg/vizij_orchestrator_wasm_bg.wasm", import.meta.url).toString();
  }
  return wasmUrlCache;
}

const loadBindingsImpl: typeof loadWasmBindings =
  typeof window === "undefined" ? loadWasmBindings : loadWasmBindingsBrowser;

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await loadBindingsImpl<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => importWasmModule(),
      defaultWasmUrl,
      init: async (module, initArg) => {
        await module.default(initArg);
      },
      getBindings: (module) => module as WasmBindings,
      expectedAbi: 2,
      getAbiVersion: (bindings) => Number(bindings.abi_version()),
    },
    input
  );
  return bindingCache.current!;
}

/** Init input forwarded to @vizij/wasm-loader. */
export type InitInput = LoaderInitInput;

let _initPromise: Promise<void> | null = null;
/**
 * Initialize the wasm module once. Must be awaited before constructing Orchestrator.
 *
 * @example
 * ```ts
 * import { init } from "@vizij/orchestrator-wasm";
 *
 * await init();
 * ```
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
  })();
  return _initPromise;
}

/**
 * Read the wasm ABI version after init() has completed.
 *
 * @throws Error if init() has not completed yet.
 */
export function abi_version(): number {
  if (!bindingCache.current) {
    throw new Error("Call init() from @vizij/orchestrator-wasm before reading abi_version().");
  }
  return Number(bindingCache.current.abi_version());
}

function ensureInited(): void {
  if (!_initPromise) {
    throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
  }
}

export type {
  ValueJSON as Value,
  ShapeJSON as Shape,
  WriteOpJSON,
  OrchestratorFrame,
  ConflictLog,
  GraphRegistrationConfig,
  MergeStrategyOptions,
  MergeConflictStrategy,
  GraphRegistrationInput,
  MergedGraphRegistrationConfig,
  GraphSubscriptions,
  AnimationRegistrationConfig,
  AnimationSetup,
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
type Value = ValueInput;

export {
  listOrchestrationFixtures,
  loadOrchestrationBundle,
  loadOrchestrationDescriptor,
  loadOrchestrationJson,
} from "./fixtures.js";

/**
 * Ergonomic wrapper around the wasm VizijOrchestrator.
 * Always await init() once before constructing.
 *
 * @example
 * ```ts
 * import { init, Orchestrator } from "@vizij/orchestrator-wasm";
 *
 * await init();
 * const orch = new Orchestrator();
 * ```
 */
export class Orchestrator {
  private inner: WasmOrchestratorInstance;
  private _hotInputs?: Set<string>;
  private _hotLastValues?: Map<string, number>;
  private _hotEpsilon: number = 0;
  private _debugLogging = false;
  private _lastFrameVersion: bigint = 0n;

  constructor(opts?: any) {
    ensureInited();
    if (!bindingCache.current) {
      throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
    }
    const Ctor = bindingCache.current.VizijOrchestrator;
    this.inner = new Ctor(opts ?? undefined) as WasmOrchestratorInstance;
  }

  /**
   * Register a graph controller.
   * Accepts a GraphSpec object or a JSON string or { id?, spec }.
   *
   * @example
   * ```ts
   * const id = orch.registerGraph({
   *   spec: { nodes: [], edges: [] },
   *   subs: { inputs: ["inputs.foo"], outputs: ["outputs.bar"] },
   * });
   * ```
   */
  registerGraph(cfg: GraphRegistrationInput): string {
    return this.inner.register_graph(cfg);
  }

  /**
   * Replace an existing graph controller's spec/subscriptions.
   * This is the supported way to apply structural edits at runtime.
   *
   * Requires { id, spec, subs? } (string form is not supported here).
   *
   * @example
   * ```ts
   * orch.replaceGraph({ id: "graph-1", spec: { nodes: [], edges: [] } });
   * ```
   */
  replaceGraph(cfg: GraphReplaceConfig): void {
    this.inner.replace_graph(cfg);
    // Structural edits invalidate delta baselines; force next stepDelta() call to establish a new baseline.
    this._lastFrameVersion = 0n;
  }

  /**
   * Register a merged graph controller.
   *
   * @example
   * ```ts
   * const mergedId = orch.registerMergedGraph({
   *   graphs: [{ id: "a", spec: { nodes: [] } }, { id: "b", spec: { nodes: [] } }],
   *   strategy: { outputs: "namespace" },
   * });
   * ```
   */
  registerMergedGraph(cfg: MergedGraphRegistrationConfig): string {
    return this.inner.register_merged_graph(cfg);
  }

  /**
   * Register an animation controller.
   * Accepts { id?: string, setup?: any }.
   *
   * @example
   * ```ts
   * const animId = orch.registerAnimation({ setup: { player: { loop_mode: "loop" } } });
   * ```
   */
  registerAnimation(cfg: AnimationRegistrationConfig): string {
    return this.inner.register_animation(cfg);
  }

  /**
   * Prebind resolver used by animation controllers.
   * resolver(path: string) => string|number|null|undefined
   *
   * @example
   * ```ts
   * orch.prebind((path) => (path.startsWith("arm.") ? 0 : null));
   * ```
   */
  prebind(resolver: WasmResolver): void {
    const f = (path: string) => resolver(path);
    this.inner.prebind(f);
  }

  /**
   * Set a blackboard input. value may be a ValueJSON or legacy shape.
   * shape is optional.
   *
   * @example
   * ```ts
   * orch.setInput("inputs.speed", 1.0);
   * ```
   */
  setInput(path: string, value: Value, shape?: ShapeJSON): void {
    const v = toValueJSON(value);
    const s = shape ?? undefined;
    this.inner.set_input(path, v, s);
  }

  /**
   * Remove a blackboard input by path.
   *
   * @example
   * ```ts
   * orch.removeInput("inputs.speed");
   * ```
   */
  removeInput(path: string): boolean {
    return this.inner.remove_input(path);
  }

  /**
   * Declare hot inputs to enable diffed staging for scalars.
   *
   * @example
   * ```ts
   * orch.setHotInputs(["inputs.speed"], { epsilon: 0.01 });
   * ```
   */
  setHotInputs(paths: string[], opts?: { epsilon?: number }): void {
    this._hotInputs = new Set(paths);
    this._hotLastValues = new Map();
    this._hotEpsilon = opts?.epsilon ?? 0;
    if (this._debugLogging) {
      console.debug("[orch] hot inputs registered", { count: paths.length, epsilon: this._hotEpsilon });
    }
  }

  /**
   * Smart staging: routes hot scalar inputs through a diff and only calls setInput when changed.
   * Non-hot or non-numeric values always call setInput.
   *
   * @example
   * ```ts
   * orch.setInputsSmart(["inputs.speed"], new Float32Array([1.2]));
   * ```
   */
  setInputsSmart(paths: string[], values: Float32Array, shapes?: (ShapeJSON | null)[]): void {
    const hot = this._hotInputs;
    const cache = this._hotLastValues ?? (this._hotLastValues = new Map());
    const eps = this._hotEpsilon;
    let hotSent = 0;
    let coldSent = 0;
    for (let i = 0; i < paths.length; i += 1) {
      const p = paths[i];
      const v = values[i];
      const shape = shapes ? shapes[i] ?? undefined : undefined;
      const numeric = Number.isFinite(v);
      if (hot?.has(p) && numeric) {
        const prev = cache.get(p);
        const differs =
          prev === undefined
            ? true
            : eps === 0
              ? v !== prev
              : Math.abs(v - prev) > eps;
        if (differs) {
          this.setInput(p, v, shape);
          cache.set(p, v);
          hotSent += 1;
        }
      } else {
        this.setInput(p, v, shape);
        coldSent += 1;
      }
    }
    if (this._debugLogging) {
      console.debug("[orch] setInputsSmart", { hotSent, coldSent, eps });
    }
  }

  /**
   * Step the orchestrator and return only changes since a version token.
   * Pass 0 (or omit) to force a full frame and establish the baseline.
   *
   * @example
   * ```ts
   * const frame = orch.stepDelta(1 / 60);
   * ```
   */
  stepDelta(dt: number, sinceVersion?: number | bigint): OrchestratorFrame & { version: bigint } {
    const token =
      typeof sinceVersion === "undefined" ? this._lastFrameVersion : BigInt(sinceVersion);
    if (typeof (this.inner as any).step_delta === "function") {
      const res = (this.inner as any).step_delta(dt, token);
      const parsed = typeof res === "string" ? (JSON.parse(res) as any) : res;
      const version = BigInt(parsed?.version ?? 0);
      this._lastFrameVersion = version;
      return { ...parsed, version };
    }
    const full = this.step(dt);
    this._lastFrameVersion = this._lastFrameVersion + 1n;
    return { ...(full as any), version: this._lastFrameVersion };
  }

  /**
   * Step the orchestrator by dt seconds. Returns the OrchestratorFrame (JS object).
   *
   * @example
   * ```ts
   * const frame = orch.step(1 / 60);
   * ```
   */
  step(dt: number): OrchestratorFrame {
    const frame = this.inner.step(dt);
    return frame as OrchestratorFrame;
  }

  /**
   * List registered graph and animation controller ids.
   *
   * @example
   * ```ts
   * const { graphs, anims } = orch.listControllers();
   * ```
   */
  listControllers(): { graphs: string[]; anims: string[] } {
    const result = this.inner.list_controllers();
    const graphs = Array.isArray(result?.graphs) ? (result!.graphs as string[]) : [];
    const anims = Array.isArray(result?.anims) ? (result!.anims as string[]) : [];
    return { graphs, anims };
  }

  /**
   * Remove a graph controller by id.
   *
   * @example
   * ```ts
   * orch.removeGraph("graph-1");
   * ```
   */
  removeGraph(id: string): boolean {
    return this.inner.remove_graph(id);
  }

  /**
   * Remove an animation controller by id.
   *
   * @example
   * ```ts
   * orch.removeAnimation("anim-1");
   * ```
   */
  removeAnimation(id: string): boolean {
    return this.inner.remove_animation(id);
  }

  /**
   * Enable or disable debug logging in the JS wrapper.
   *
   * @example
   * ```ts
   * orch.setDebugLogging(true);
   * ```
   */
  setDebugLogging(enabled: boolean): void {
    this._debugLogging = enabled;
  }

  /**
   * Normalize a GraphSpec (object or JSON string) using the Rust normalizer.
   *
   * @example
   * ```ts
   * const normalized = await orch.normalizeGraphSpec({ nodes: [] });
   * ```
   */
  async normalizeGraphSpec(spec: object | string): Promise<object> {
    await init();
    const mod = await loadBindings();
    const json = typeof spec === "string" ? spec : JSON.stringify(spec);
    const normalized = mod.normalize_graph_spec_json(json);
    return JSON.parse(normalized);
  }
}

/**
 * Convenience helper to init() and return a ready Orchestrator instance.
 *
 * @example
 * ```ts
 * import { createOrchestrator } from "@vizij/orchestrator-wasm";
 *
 * const orch = await createOrchestrator();
 * ```
 */
export async function createOrchestrator(opts?: any): Promise<Orchestrator> {
  await init();
  return new Orchestrator(opts);
}
