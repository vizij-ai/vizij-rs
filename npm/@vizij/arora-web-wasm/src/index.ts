/**
 * Stable ESM entrypoint for `@vizij/arora-web-wasm`.
 *
 * Runs a Vizij runtime in the browser *as an Arora device*: the wasm module
 * (`crates/interop/vizij-arora-web`) assembles an `arora_web::BrowserRuntime`
 * over a Vizij blackboard store, rig HAL, and the caller's node graph as the
 * device's behavior. This wrapper loads the wasm once and exposes the device
 * with idiomatic JS types: values cross the boundary in the normalized
 * `ValueJSON` vocabulary shared with the other Vizij packages.
 *
 * Typical use:
 * ```ts
 * import { init, startDevice } from "@vizij/arora-web-wasm";
 *
 * await init();
 * const device = await startDevice(graphSpec);
 * device.run(); // the device paces itself from here on
 * // each animation frame:
 * device.setValue("sensor/x", { f32: 0.75 });
 * const changes = device.drainChanges();
 * ```
 *
 * A host with its own clock skips `run()` and calls `device.step(dtMs)`
 * per frame instead.
 */
import { toValueJSON, type ValueJSON, type ValueInput } from "@vizij/value-json";
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";

/** A Vizij graph spec, as an object or already-serialized JSON. */
export type GraphSpecInput = object | string;

/**
 * An Arora wasm module to load into the device's engine: its header as JSON
 * plus its `.wasm` executable bytes — e.g. what `@vizij/animation-module`'s
 * `loadAnimationModule()` returns.
 */
export interface DeviceModule {
  headerJson: string;
  wasmBytes: Uint8Array;
}

/**
 * An Arora `Call`, as an object (or already-serialized JSON): the function
 * `id`, optionally the `module_id` it lives in (inferred from the loaded
 * modules when omitted), and `args` as `{ id, value }` pairs in the Arora
 * `Value` vocabulary.
 */
export type DeviceCall = object | string;

/** What a device call resolves to: the returned Arora `Value`. */
export interface DeviceCallResult {
  ret: unknown;
  mutated?: unknown[];
}

interface WasmVizijArora {
  step(dt_ms: number): void;
  run(period_ms?: number): Promise<never>;
  call(call_json: string): Promise<string>;
  loadGraph(graph_json: string): Promise<string>;
  setValue(path: string, value_json: string): void;
  writeValues(values_json: string): void;
  readValues(paths: string[]): Record<string, ValueJSON | null>;
  snapshot(): Record<string, ValueJSON>;
  drainChanges(): Record<string, ValueJSON | null>;
  free(): void;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  VizijArora: {
    start(graph_json?: string, modules?: DeviceModule[]): Promise<WasmVizijArora>;
  };
}

const bindingCache: { current: WasmBindings | null } = { current: null };
let wasmModulePromise: Promise<unknown> | null = null;
let wasmUrlCache: string | null = null;

function toWasmBindgenInitOptions(initArg: unknown): { module_or_path: unknown } {
  if (
    initArg &&
    typeof initArg === "object" &&
    "module_or_path" in (initArg as Record<string, unknown>)
  ) {
    return initArg as { module_or_path: unknown };
  }
  return { module_or_path: initArg };
}

function pkgWasmJsUrl(): URL {
  return new URL("../../pkg/vizij_arora_web.js", import.meta.url);
}

function importStaticWasmModule(): Promise<unknown> {
  return import("../../pkg/vizij_arora_web.js");
}

function importDynamicWasmModule(): Promise<unknown> {
  return import(/* @vite-ignore */ pkgWasmJsUrl().toString());
}

async function importWasmModule(): Promise<unknown> {
  if (!wasmModulePromise) {
    wasmModulePromise = importStaticWasmModule().catch((err) => {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn(
          "@vizij/arora-web-wasm: static wasm import failed, falling back to runtime URL import.",
          err,
        );
      }
      return importDynamicWasmModule();
    });
  }
  return wasmModulePromise;
}

function defaultWasmUrl(): string {
  if (!wasmUrlCache) {
    wasmUrlCache = new URL("../../pkg/vizij_arora_web_bg.wasm", import.meta.url).toString();
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
        await (module as WasmBindings).default(toWasmBindgenInitOptions(initArg));
      },
      getBindings: (module) => module as WasmBindings,
    },
    input,
  );
  return bindingCache.current!;
}

export type InitInput = LoaderInitInput;

let _initPromise: Promise<void> | null = null;

/**
 * Initialize the wasm module once. The returned promise is memoized; await it
 * during startup before calling `startDevice`.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
  })();
  return _initPromise;
}

/**
 * The running Vizij-on-Arora device. All methods talk to the device's shared
 * store; the graph installed at `startDevice` reads and writes the same keys.
 */
export class AroraDevice {
  private inner: WasmVizijArora;
  private selfPaced = false;

  constructor(inner: WasmVizijArora) {
    this.inner = inner;
  }

  /**
   * Advance the device one tick. `dtMs` is the wall time since the previous
   * step in milliseconds (the difference of two `requestAnimationFrame`
   * timestamps). Unavailable once `run()` has taken the device — it paces
   * itself from then on.
   */
  step(dtMs: number): void {
    this.inner.step(dtMs);
  }

  /**
   * Hand the device to its own loop, for good: a self-paced run at
   * `periodMs` (default: the runtime's ~100 Hz) that owns the device until
   * stepping fails — the returned promise only ever rejects, and `step()`
   * is unavailable from then on. The rest of this surface keeps working
   * while the device runs; it never touches the stepping device.
   */
  run(periodMs?: number): Promise<never> {
    this.selfPaced = true;
    return this.inner.run(periodMs);
  }

  /** Whether `run()` has taken the device (so `step()` is unavailable). */
  get running(): boolean {
    return this.selfPaced;
  }

  /**
   * Call a loaded module's function through the device. The call is enqueued
   * before this returns and dispatches inside the device's **next** step —
   * the same phase a remote bridge command executes in — so the returned
   * promise resolves only after that step runs. Under `run()` just `await`
   * it; a direct driver calls `step` after issuing it.
   */
  call(call: DeviceCall): Promise<DeviceCallResult> {
    const json = typeof call === "string" ? call : JSON.stringify(call);
    return this.inner.call(json).then((result) => JSON.parse(result) as DeviceCallResult);
  }

  /**
   * Replace the device's running graph **in place**: the spec reaches the
   * interpreter as the engine's LOAD call, so the store, the loaded modules,
   * and the device itself all survive the swap. Resolves once the new graph
   * is installed; on a device not under `run()` a zero-dt step is taken so
   * the swap lands without an external driver.
   */
  loadGraph(graph: GraphSpecInput): Promise<void> {
    const json = typeof graph === "string" ? graph : JSON.stringify(graph);
    const loaded = this.inner.loadGraph(json);
    if (!this.selfPaced) {
      this.inner.step(0);
    }
    return loaded.then(() => undefined);
  }

  /** Write one store key. Accepts any `ValueInput` shorthand. */
  setValue(path: string, value: ValueInput): void {
    this.inner.setValue(path, JSON.stringify(toValueJSON(value)));
  }

  /** Write several store keys as one change. */
  writeValues(values: Record<string, ValueInput>): void {
    const normalized: Record<string, ValueJSON> = {};
    for (const [path, value] of Object.entries(values)) {
      normalized[path] = toValueJSON(value);
    }
    this.inner.writeValues(JSON.stringify(normalized));
  }

  /** Read store keys; absent keys map to `null`. */
  readValues(paths: string[]): Record<string, ValueJSON | null> {
    return this.inner.readValues(paths);
  }

  /** Every key currently in the store. */
  snapshot(): Record<string, ValueJSON> {
    return this.inner.snapshot();
  }

  /**
   * The keys that changed since the last call (`null` = cleared) — poll it to
   * feed a renderer. The first drain returns the store's whole current state:
   * the subscription opens on it, so no separate init snapshot is needed.
   */
  drainChanges(): Record<string, ValueJSON | null> {
    return this.inner.drainChanges();
  }

  /** Release the wasm-side device. The instance is unusable afterwards. */
  dispose(): void {
    this.inner.free();
  }
}

/**
 * Boot the device in the browser, with `graph` (a Vizij graph spec, in any
 * form the spec normalizer accepts) installed as its behavior. The graph's
 * `input` nodes' paths become the store keys it reads each tick. Omit `graph`
 * to get the built-in passthrough proof graph (`sensor/x` → `actuator/y`).
 *
 * `modules` optionally loads Arora wasm modules into the device's engine;
 * their functions are then reachable with `AroraDevice.call` and from the
 * graph's `ExternalFunction` nodes.
 *
 * Calls `init()` if it has not run yet.
 */
export async function startDevice(
  graph?: GraphSpecInput,
  input?: InitInput,
  modules?: DeviceModule[],
): Promise<AroraDevice> {
  await init(input);
  const bindings = bindingCache.current!;
  const graphJson =
    graph === undefined ? undefined : typeof graph === "string" ? graph : JSON.stringify(graph);
  const inner = await bindings.VizijArora.start(graphJson, modules);
  return new AroraDevice(inner);
}

export { toValueJSON } from "@vizij/value-json";
export type { ValueJSON, ValueInput } from "@vizij/value-json";
