// Stable ESM entry for @vizij/orchestrator-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
import type {
  AnimationRegistrationConfig,
  AnimationSetup,
  GraphRegistrationInput,
  GraphSubscriptions,
  OrchestratorFrame,
  ValueJSON,
  ShapeJSON,
  WriteOpJSON,
  ConflictLog,
  GraphRegistrationConfig,
} from "./types";
import { toValueJSON, type ValueInput } from "@vizij/value-json";
import { loadBindings as loadWasmBindings, type InitInput as LoaderInitInput } from "@vizij/wasm-loader";
type WasmResolver = (path: string) => string | number | null | undefined;

interface WasmOrchestratorInstance {
  register_graph(cfg: GraphRegistrationConfig | string): string;
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

function pkgWasmJsUrl(): URL {
  return new URL("../../pkg/vizij_orchestrator_wasm.js", import.meta.url);
}

function defaultWasmUrl(): URL {
  return new URL("../../pkg/vizij_orchestrator_wasm_bg.wasm", import.meta.url);
}

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await loadWasmBindings<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => import(/* @vite-ignore */ pkgWasmJsUrl().toString()),
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

export type InitInput = LoaderInitInput;

/**
 * Initialize the wasm module once.
 */
let _initPromise: Promise<void> | null = null;
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
  })();
  return _initPromise;
}

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
  GraphRegistrationInput,
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

/**
 * Ergonomic wrapper around the wasm VizijOrchestrator.
 * Always await init() once before constructing.
 */
export class Orchestrator {
  private inner: WasmOrchestratorInstance;

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
   */
  registerGraph(cfg: GraphRegistrationInput): string {
    return this.inner.register_graph(cfg);
  }

  /**
   * Register an animation controller.
   * Accepts { id?: string, setup?: any }.
   */
  registerAnimation(cfg: AnimationRegistrationConfig): string {
    return this.inner.register_animation(cfg);
  }

  /**
   * Prebind resolver used by animation controllers.
   * resolver(path: string) => string|number|null|undefined
   */
  prebind(resolver: WasmResolver): void {
    const f = (path: string) => resolver(path);
    this.inner.prebind(f);
  }

  /**
   * Set a blackboard input. value may be a ValueJSON or legacy shape.
   * shape is optional.
   */
  setInput(path: string, value: Value, shape?: ShapeJSON): void {
    const v = toValueJSON(value);
    const s = shape ?? undefined;
    this.inner.set_input(path, v, s);
  }

  removeInput(path: string): boolean {
    return this.inner.remove_input(path);
  }

  /**
   * Step the orchestrator by dt seconds. Returns the OrchestratorFrame (JS object).
   */
  step(dt: number): OrchestratorFrame {
    const frame = this.inner.step(dt);
    return frame as OrchestratorFrame;
  }

  listControllers(): { graphs: string[]; anims: string[] } {
    const result = this.inner.list_controllers();
    const graphs = Array.isArray(result?.graphs) ? (result!.graphs as string[]) : [];
    const anims = Array.isArray(result?.anims) ? (result!.anims as string[]) : [];
    return { graphs, anims };
  }

  removeGraph(id: string): boolean {
    return this.inner.remove_graph(id);
  }

  removeAnimation(id: string): boolean {
    return this.inner.remove_animation(id);
  }

  /**
   * Normalize a GraphSpec (object or JSON string) using the Rust normalizer.
   */
  async normalizeGraphSpec(spec: object | string): Promise<object> {
    await init();
    const mod = await loadBindings();
    const json = typeof spec === "string" ? spec : JSON.stringify(spec);
    const normalized = mod.normalize_graph_spec_json(json);
    return JSON.parse(normalized);
  }
}

export async function createOrchestrator(opts?: any): Promise<Orchestrator> {
  await init();
  return new Orchestrator(opts);
}
