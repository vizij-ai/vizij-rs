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
  type LoadBindingsOptions,
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
  return new URL("../pkg/vizij_orchestrator_wasm.js", import.meta.url);
}

function importStaticWasmModule(): Promise<unknown> {
  return import("../pkg/vizij_orchestrator_wasm.js") as unknown as Promise<unknown>;
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
    wasmUrlCache = new URL("../pkg/vizij_orchestrator_wasm_bg.wasm", import.meta.url).toString();
  }
  return wasmUrlCache;
}

const loadBindingsImpl: <TBindings>(
  options: LoadBindingsOptions<TBindings>,
  initInput?: LoaderInitInput,
) => Promise<TBindings> =
  typeof window === "undefined" ? loadWasmBindings : loadWasmBindingsBrowser;

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await (loadBindingsImpl as <TBindings>(
    options: Parameters<typeof loadWasmBindings>[0],
    initInput?: LoaderInitInput
  ) => Promise<TBindings>)<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => importWasmModule(),
      defaultWasmUrl,
      init: async (module, initArg) => {
        await module.default(initArg);
      },
      getBindings: (module) => module as WasmBindings,
      expectedAbi: 2,
      getAbiVersion: (bindings) => Number((bindings as WasmBindings).abi_version()),
    },
    input
  );
  return bindingCache.current!;
}

/**
 * Init input forwarded to @vizij/wasm-loader.
 *
 * Pass a URL/bytes/module when hosting the wasm binary yourself.
 */
export type InitInput = LoaderInitInput;

let _initPromise: Promise<void> | null = null;
/**
 * Initialize the wasm module once.
 *
 * Must be awaited before constructing {@link Orchestrator}. The loader memoizes
 * bindings, so repeated calls reuse the same module.
 *
 * @param input - Optional wasm init input (URL, bytes, Response, or Module).
 * @throws Error if ABI validation fails or the wasm module cannot be loaded.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
  })();
  return _initPromise;
}

/**
 * Read the wasm ABI version after {@link init} has completed.
 *
 * @returns ABI version number baked into the wasm binary.
 * @throws Error if the bindings are not initialized yet.
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
 * High-level wrapper around the wasm VizijOrchestrator.
 *
 * Always await {@link init} once before constructing. Methods throw when
 * the underlying wasm bindings do not expose required exports.
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
   *
   * Accepts a GraphSpec object, a JSON string, or `{ id?, spec }`.
   *
   * @param cfg - Graph registration payload.
   * @returns Controller id (auto-generated when omitted).
   * @throws Error when the payload is invalid or wasm rejects the spec.
   */
  registerGraph(cfg: GraphRegistrationInput): string {
    return this.inner.register_graph(cfg);
  }

  /**
   * Replace an existing graph controller's spec/subscriptions.
   *
   * This is the supported way to apply structural edits at runtime.
   * Requires `{ id, spec, subs? }` (string form is not supported).
   *
   * @param cfg - Replacement config with existing controller id.
   * @throws Error when the payload is invalid or wasm rejects the spec.
   */
  replaceGraph(cfg: GraphReplaceConfig): void {
    this.inner.replace_graph(cfg);
    // Structural edits invalidate delta baselines; force next stepDelta() call to establish a new baseline.
    this._lastFrameVersion = 0n;
  }

  /**
   * Register a merged graph controller.
   *
   * @param cfg - Merge configuration (graphs + conflict strategy).
   * @returns Controller id (auto-generated when omitted).
   * @throws Error when the payload is invalid or wasm rejects the merge.
   */
  registerMergedGraph(cfg: MergedGraphRegistrationConfig): string {
    return this.inner.register_merged_graph(cfg);
  }

  /**
   * Register an animation controller.
   *
   * @param cfg - Animation registration config (`setup` seeds players/instances).
   * @returns Controller id (auto-generated when omitted).
   * @throws Error when the payload is invalid or wasm rejects the setup.
   */
  registerAnimation(cfg: AnimationRegistrationConfig): string {
    return this.inner.register_animation(cfg);
  }

  /**
   * Prebind resolver used by animation controllers.
   *
   * The resolver maps typed paths to host-specific keys and is cached by the engine.
   * Returning `null`/`undefined` leaves the path unbound.
   *
   * @param resolver - Callback invoked on demand for target path binding.
   */
  prebind(resolver: WasmResolver): void {
    const f = (path: string) => resolver(path);
    this.inner.prebind(f);
  }

  /**
   * Set a blackboard input.
   *
   * @param path - Typed path string.
   * @param value - Value payload (legacy or normalized).
   * @param shape - Optional shape metadata.
   */
  setInput(path: string, value: Value, shape?: ShapeJSON): void {
    const v = toValueJSON(value);
    const s = shape ?? undefined;
    this.inner.set_input(path, v, s);
  }

  /**
   * Remove a blackboard input by path.
   *
   * @param path - Typed path string.
   * @returns `true` if the input existed and was removed.
   */
  removeInput(path: string): boolean {
    return this.inner.remove_input(path);
  }

  /**
   * Declare hot inputs to enable diffed staging for scalars.
   *
   * Hot paths are cached in the JS wrapper and compared with an optional epsilon.
   *
   * @param paths - Typed path strings to track as hot.
   * @param opts - Optional diff epsilon for numeric comparisons.
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
   *
   * Non-hot or non-numeric values always call {@link setInput}.
   *
   * @param paths - Typed path strings aligned with `values`.
   * @param values - Numeric values aligned with `paths`.
   * @param shapes - Optional shape metadata aligned with `paths`.
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
   *
   * Pass `0` (or omit) to force a full frame and establish the baseline.
   *
   * @param dt - Delta time in seconds.
   * @param sinceVersion - Version token from a previous call.
   * @returns Delta frame plus new version token.
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
   * Step the orchestrator by dt seconds.
   *
   * @param dt - Delta time in seconds.
   * @returns OrchestratorFrame for this step.
   */
  step(dt: number): OrchestratorFrame {
    const frame = this.inner.step(dt);
    return frame as OrchestratorFrame;
  }

  /**
   * List registered graph and animation controller ids.
   *
   * @returns Object containing `graphs` and `anims` arrays.
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
   * @param id - Controller id to remove.
   * @returns `true` if the controller existed and was removed.
   */
  removeGraph(id: string): boolean {
    return this.inner.remove_graph(id);
  }

  /**
   * Remove an animation controller by id.
   *
   * @param id - Controller id to remove.
   * @returns `true` if the controller existed and was removed.
   */
  removeAnimation(id: string): boolean {
    return this.inner.remove_animation(id);
  }

  /**
   * Enable or disable debug logging in the JS wrapper.
   *
   * @param enabled - When true, logs staging and hot-input events to console.
   */
  setDebugLogging(enabled: boolean): void {
    this._debugLogging = enabled;
  }

  /**
   * Normalize a GraphSpec (object or JSON string) using the Rust normalizer.
   *
   * @param spec - GraphSpec object or JSON string.
   * @returns Normalized GraphSpec with explicit edges/paths.
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
 * Convenience helper that awaits {@link init} and returns a ready {@link Orchestrator}.
 *
 * @param opts - Optional constructor options (schedule, config).
 * @returns Initialized orchestrator instance.
 */
export async function createOrchestrator(opts?: any): Promise<Orchestrator> {
  await init();
  return new Orchestrator(opts);
}
