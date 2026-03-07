/**
 * Stable ESM entrypoint for `@vizij/orchestrator-wasm`.
 *
 * This wrapper initializes the orchestrator wasm bindings, validates ABI compatibility, and
 * exposes the higher-level `Orchestrator` class used by browser and Node consumers.
 */
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

function toWasmBindgenInitOptions(initArg: unknown): { module_or_path: unknown } {
  return { module_or_path: initArg };
}

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
      init: async (module, initArg) => {
        await module.default(toWasmBindgenInitOptions(initArg));
      },
      getBindings: (module) => module as WasmBindings,
      expectedAbi: 2,
      getAbiVersion: (bindings) => Number(bindings.abi_version()),
    },
    input
  );
  return bindingCache.current!;
}

export type InitInput = LoaderInitInput;

let _initPromise: Promise<void> | null = null;
/**
 * Initialize the wasm module once.
 *
 * The returned promise is memoized, so most applications should await this once during startup
 * before constructing `Orchestrator` instances.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
  })();
  return _initPromise;
}

/**
 * Return the ABI version reported by the loaded orchestrator wasm module.
 *
 * This is primarily useful for diagnosing wrapper-versus-`pkg/` mismatches during local rebuilds.
 * Call `init()` successfully before reading it.
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
 *
 * Use this wrapper to register graph and animation controllers, stage blackboard inputs, and step
 * the orchestrator without dealing with the raw wasm class directly. Always await `init()` once
 * before constructing.
 */
export class Orchestrator {
  private inner: WasmOrchestratorInstance;
  private _hotInputs?: Set<string>;
  private _hotLastValues?: Map<string, number>;
  private _hotEpsilon: number = 0;
  private _debugLogging = false;
  private _lastFrameVersion: bigint = 0n;

  /**
   * Create an orchestrator wrapper bound to the already-initialized wasm module.
   *
   * `opts` is forwarded to the underlying runtime constructor unchanged. Typical hosts use it for
   * scheduler/runtime configuration.
   */
  constructor(opts?: any) {
    ensureInited();
    if (!bindingCache.current) {
      throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
    }
    const Ctor = bindingCache.current.VizijOrchestrator;
    this.inner = new Ctor(opts ?? undefined) as WasmOrchestratorInstance;
  }

  /**
   * Register one graph controller and return its resolved controller id.
   *
   * `cfg` may be a raw graph JSON string or a typed registration object with optional controller
   * id and subscription filters. The returned id is the handle used by `replaceGraph()` and
   * `removeGraph()`.
   */
  registerGraph(cfg: GraphRegistrationInput): string {
    return this.inner.register_graph(cfg);
  }

  /**
   * Replace an existing graph controller's spec/subscriptions.
   * This is the supported way to apply structural edits at runtime.
   *
   * Requires { id, spec, subs? } (string form is not supported here).
   */
  replaceGraph(cfg: GraphReplaceConfig): void {
    this.inner.replace_graph(cfg);
    // Structural edits invalidate delta baselines; force next stepDelta() call to establish a new baseline.
    this._lastFrameVersion = 0n;
  }

  /**
   * Register a merged graph controller assembled from several graph registrations.
   *
   * Graphs are merged in declaration order. Use `strategy` to control how output and intermediate
   * name collisions are resolved. The returned id identifies the combined controller.
   */
  registerMergedGraph(cfg: MergedGraphRegistrationConfig): string {
    return this.inner.register_merged_graph(cfg);
  }

  /**
   * Register one animation controller and return its resolved controller id.
   *
   * Pass `setup` when the controller should boot with a specific animation/player/instance
   * configuration instead of starting empty.
   */
  registerAnimation(cfg: AnimationRegistrationConfig): string {
    return this.inner.register_animation(cfg);
  }

  /**
   * Install a resolver used by animation controllers to map canonical target paths to host handles.
   *
   * The callback should return a stable string or number handle for known paths. Returning
   * `null`/`undefined` leaves the path unresolved and the canonical string path is used instead.
   */
  prebind(resolver: WasmResolver): void {
    const f = (path: string) => resolver(path);
    this.inner.prebind(f);
  }

  /**
   * Overwrite a blackboard input at `path`.
   *
   * The value remains present for future steps until it is replaced or removed. `shape` is
   * optional and only needed when the host wants to preserve explicit shape metadata.
   */
  setInput(path: string, value: Value, shape?: ShapeJSON): void {
    const v = toValueJSON(value);
    const s = shape ?? undefined;
    this.inner.set_input(path, v, s);
  }

  /**
   * Remove a blackboard input previously staged with `setInput()`.
   *
   * Returns `true` only when an entry existed at `path`.
   */
  removeInput(path: string): boolean {
    return this.inner.remove_input(path);
  }

  /**
   * Mark a subset of blackboard input paths as hot so repeated scalar updates can be diffed.
   *
   * Use this before `setInputsSmart()` when a host streams high-frequency numeric values and wants
   * to avoid re-sending unchanged inputs every frame.
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
   * Stage many inputs, diffing hot numeric entries against the previously sent value.
   *
   * Entries whose paths were registered via `setHotInputs()` are only forwarded when the value
   * changed by more than `epsilon`. Non-hot entries, shaped values, and other non-numeric payloads
   * are always forwarded through `setInput()`.
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
   * Step the orchestrator and return only changes since a previously observed frame version.
   *
   * Pass the `version` returned by an earlier `stepDelta()` call. Passing `0` or omitting the
   * token forces a full frame and establishes a new delta baseline.
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
   * Advance the orchestrator by `dt` seconds and return the full frame snapshot.
   *
   * The frame contains merged writes, conflict diagnostics, timing data, and controller events for
   * that scheduler tick.
   */
  step(dt: number): OrchestratorFrame {
    const frame = this.inner.step(dt);
    return frame as OrchestratorFrame;
  }

  /**
   * List the ids of currently registered graph and animation controllers.
   *
   * This is a read-only inspection helper; it does not validate that a controller is active or
   * currently producing output.
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
   * Returns `false` when no graph controller with that id is currently registered.
   */
  removeGraph(id: string): boolean {
    return this.inner.remove_graph(id);
  }

  /**
   * Remove an animation controller by id.
   *
   * Returns `false` when no animation controller with that id is currently registered.
   */
  removeAnimation(id: string): boolean {
    return this.inner.remove_animation(id);
  }

  /**
   * Enable or disable verbose wrapper-side debug logging.
   *
   * Logging is limited to helper behavior such as hot-input diffing; it does not change
   * orchestrator semantics.
   */
  setDebugLogging(enabled: boolean): void {
    this._debugLogging = enabled;
  }

  /**
   * Normalize a graph specification through the Rust-side node-graph normalizer.
   *
   * This is useful before registration when a host wants the exact canonical graph JSON the wasm
   * runtime will consume.
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
 * Initialize the wasm module if needed and return a ready-to-use `Orchestrator`.
 */
export async function createOrchestrator(opts?: any): Promise<Orchestrator> {
  await init();
  return new Orchestrator(opts);
}
