// Stable ESM entry for @vizij/orchestrator-wasm
// Wraps the wasm-pack output in ../../pkg (built with `--target web`).
import type { OrchestratorFrame, ValueJSON, ShapeJSON, WriteOpJSON } from "./types";
let _bindings: any | null = null;

function pkgWasmJsUrl(): URL {
  return new URL("../../pkg/vizij_orchestrator_wasm.js", import.meta.url);
}

function defaultWasmUrl(): URL {
  return new URL("../../pkg/vizij_orchestrator_wasm_bg.wasm", import.meta.url);
}

async function loadBindings(input?: InitInput): Promise<any> {
  if (!_bindings) {
    const mod: any = await import(/* @vite-ignore */ pkgWasmJsUrl().toString());
    let initArg: any = input ?? defaultWasmUrl();

    // Node.js file:// support: read bytes if a file: URL is passed
    try {
      const isUrlObj = typeof initArg === "object" && initArg !== null && "href" in (initArg as any);
      const href = isUrlObj ? (initArg as URL).href : (typeof initArg === "string" ? (initArg as string) : "");
      const isFileUrl =
        (isUrlObj && (initArg as URL).protocol === "file:") ||
        (typeof href === "string" && href.startsWith("file:"));

      if (isFileUrl) {
        const fsSpec = "node:fs/promises";
        const urlSpec = "node:url";
        const [{ readFile }, { fileURLToPath }] = await Promise.all([
          import(/* @vite-ignore */ fsSpec),
          import(/* @vite-ignore */ urlSpec),
        ]);
        const path = fileURLToPath(isUrlObj ? (initArg as URL) : new URL(href));
        const bytes = await readFile(path);
        initArg = bytes;
      }
    } catch {
      // ignore - bundlers handle URLs in the browser
    }

    await mod.default(initArg);
    _bindings = mod;
  }
  return _bindings;
}

export type InitInput = string | URL | Uint8Array;

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

function ensureInited(): void {
  if (!_initPromise) {
    throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
  }
}

export type { ValueJSON as Value, ShapeJSON, WriteOpJSON, OrchestratorFrame };
type Value = ValueJSON;

/**
 * Ergonomic wrapper around the wasm VizijOrchestrator.
 * Always await init() once before constructing.
 */
export class Orchestrator {
  private inner: any;

  constructor(opts?: any) {
    ensureInited();
    if (!_bindings) {
      throw new Error("Call init() from @vizij/orchestrator-wasm before creating Orchestrator instances.");
    }
    const Ctor = (_bindings as any).VizijOrchestrator;
    this.inner = new Ctor(opts ?? undefined);
  }

  /**
   * Register a graph controller.
   * Accepts a GraphSpec object or a JSON string or { id?, spec }.
   */
  registerGraph(cfg: object | string): string {
    const arg = typeof cfg === "string" ? cfg : cfg;
    return this.inner.register_graph(arg);
  }

  /**
   * Register an animation controller.
   * Accepts { id?: string, setup?: any }.
   */
  registerAnimation(cfg: object): string {
    return this.inner.register_animation(cfg);
  }

  /**
   * Prebind resolver used by animation controllers.
   * resolver(path: string) => string|number|null|undefined
   */
  prebind(resolver: (path: string) => string | number | null | undefined): void {
    const f = (path: string) => resolver(path);
    this.inner.prebind(f);
  }

  /**
   * Set a blackboard input. value may be a ValueJSON or legacy shape.
   * shape is optional.
   */
  setInput(path: string, value: Value, shape?: ShapeJSON): void {
    const v = value;
    const s = shape ?? undefined;
    this.inner.set_input(path, v, s);
  }

  removeInput(path: string): boolean {
    return this.inner.remove_input(path);
  }

  /**
   * Step the orchestrator by dt seconds. Returns the OrchestratorFrame (JS object).
   */
  step(dt: number): any {
    return this.inner.step(dt);
  }

  listControllers(): any {
    return this.inner.list_controllers();
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
