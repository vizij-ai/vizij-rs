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
 * // each animation frame:
 * device.setValue("sensor/x", { f32: 0.75 });
 * device.step(dtMs);
 * const changes = device.drainChanges();
 * ```
 */
import { toValueJSON, type ValueJSON, type ValueInput } from "@vizij/value-json";
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";

/** A Vizij graph spec, as an object or already-serialized JSON. */
export type GraphSpecInput = object | string;

interface WasmVizijArora {
  step(dt_ms: number): boolean;
  setValue(path: string, value_json: string): void;
  writeValues(values_json: string): void;
  readValues(paths: string[]): unknown;
  snapshot(): unknown;
  drainChanges(): unknown;
  free(): void;
}

/**
 * The wasm side serializes its path→Value objects with serde-wasm-bindgen's
 * default map representation: JS `Map`s, nested. Convert them (deeply) to the
 * plain objects the `ValueJSON` vocabulary uses; plain objects pass through,
 * so a wasm module that already emits objects is a no-op.
 */
function toPlain(value: unknown): unknown {
  if (value instanceof Map) {
    const out: Record<string, unknown> = {};
    for (const [k, v] of value.entries()) {
      out[String(k)] = toPlain(v);
    }
    return out;
  }
  if (Array.isArray(value)) {
    return value.map(toPlain);
  }
  return value;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  VizijArora: {
    start(graph_json?: string): Promise<WasmVizijArora>;
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

  constructor(inner: WasmVizijArora) {
    this.inner = inner;
  }

  /**
   * Advance the device one tick. `dtMs` is the wall time since the previous
   * step in milliseconds (the difference of two `requestAnimationFrame`
   * timestamps). Returns `true` while the device is live; once it returns
   * `false` the device is unregistered — stop stepping.
   */
  step(dtMs: number): boolean {
    return this.inner.step(dtMs);
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
    return toPlain(this.inner.readValues(paths)) as Record<string, ValueJSON | null>;
  }

  /** Every key currently in the store. */
  snapshot(): Record<string, ValueJSON> {
    return toPlain(this.inner.snapshot()) as Record<string, ValueJSON>;
  }

  /**
   * The keys that changed since the last call (`null` = cleared). Call right
   * after `step` to feed a renderer.
   */
  drainChanges(): Record<string, ValueJSON | null> {
    return toPlain(this.inner.drainChanges()) as Record<string, ValueJSON | null>;
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
 * Calls `init()` if it has not run yet.
 */
export async function startDevice(
  graph?: GraphSpecInput,
  input?: InitInput,
): Promise<AroraDevice> {
  await init(input);
  const bindings = bindingCache.current!;
  const graphJson =
    graph === undefined ? undefined : typeof graph === "string" ? graph : JSON.stringify(graph);
  const inner = await bindings.VizijArora.start(graphJson);
  return new AroraDevice(inner);
}

export { toValueJSON } from "@vizij/value-json";
export type { ValueJSON, ValueInput } from "@vizij/value-json";
