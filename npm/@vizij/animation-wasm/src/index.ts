// Stable ESM entry for @vizij/animation-wasm
// Wraps the wasm-pack output in ../pkg (built with `--target web`).
import initWasm, { VizijAnimation, abi_version } from "../pkg/vizij_animation_wasm.js";

import type {
  InitInput,
  Config,
  Inputs,
  Outputs,
  AnimationData,
  StoredAnimation,
  AnimId,
  PlayerId,
  InstId,
  Value,
  CoreEvent,
  Change,
} from "./types";

export type {
  InitInput,
  Config,
  Inputs,
  Outputs,
  AnimationData,
  StoredAnimation,
  AnimId,
  PlayerId,
  InstId,
  Value,
  CoreEvent,
  Change,
};

export { VizijAnimation, abi_version };

/* -----------------------------------------------------------
   init() — initialize the wasm module once (parity with node-graph)
----------------------------------------------------------- */

let _initPromise: Promise<void> | null = null;

function defaultWasmUrl(): URL {
  return new URL("../pkg/vizij_animation_wasm_bg.wasm", import.meta.url);
}

/**
 * Initialize the wasm module once.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;

  _initPromise = (async () => {
    if (typeof input !== "undefined") {
      // Caller provided explicit input (URL/Request/Response/BufferSource/Module)
      await initWasm(input as any);
    } else if (typeof window === "undefined" && typeof process !== "undefined") {
      // Node environment: read the wasm file from disk and pass bytes to avoid fetch(file://) issues
      const { readFile } = await import("node:fs/promises");
      const { fileURLToPath } = await import("node:url");
      const { dirname, resolve } = await import("node:path");
      const wasmUrl = new URL("../pkg/vizij_animation_wasm_bg.wasm", import.meta.url);
      const wasmPath =
        wasmUrl.protocol === "file:"
          ? fileURLToPath(wasmUrl)
          : resolve(dirname(fileURLToPath(import.meta.url)), "../pkg/vizij_animation_wasm_bg.wasm");
      const bytes = await readFile(wasmPath);
      await initWasm(bytes);
    } else {
      // Browser-like: URL is fine
      await initWasm(defaultWasmUrl());
    }

    // ABI guard
    const abi = Number(abi_version());
    if (abi !== 1) {
      throw new Error(
        `@vizij/animation-wasm ABI mismatch: expected 1, got ${abi}. ` +
          `Please rebuild the WASM package to ensure compatibility.`
      );
    }
  })();

  return _initPromise;
}

function ensureInited(): void {
  if (!_initPromise) {
    throw new Error(
      "Call init() from @vizij/animation-wasm before creating Engine or VizijAnimation."
    );
  }
}

/* -----------------------------------------------------------
   Ergonomic wrapper — Engine (parity with node-graph Graph)
----------------------------------------------------------- */

export class Engine {
  private inner: any;

  constructor(config?: Config) {
    ensureInited();
    // wasm-bindgen expects a JsValue; undefined/null uses defaults per Rust impl
    this.inner = new VizijAnimation(config as any);
  }

  /**
   * Load an animation clip into the engine. If `opts.format` is omitted,
   * this will auto-detect "stored" when `tracks` is present on the object.
   */
  loadAnimation(
    data: AnimationData | StoredAnimation,
    opts?: { format?: "core" | "stored" }
  ): AnimId {
    const format =
      opts?.format ??
      (typeof (data as any)?.tracks !== "undefined" ? "stored" : "core");

    const inner: any = this.inner;
    if (format === "stored") {
      if (typeof inner.load_stored_animation !== "function") {
        throw new Error(
          "Current WASM build does not expose load_stored_animation; rebuild vizij-animation-wasm with updated bindings."
        );
      }
      return inner.load_stored_animation(data as any) as AnimId;
    } else {
      if (typeof inner.load_animation !== "function") {
        throw new Error(
          "Current WASM build does not expose load_animation; rebuild vizij-animation-wasm with updated bindings."
        );
      }
      return inner.load_animation(data as any) as AnimId;
    }
  }

  /** Create a new player by display name */
  createPlayer(name: string): PlayerId {
    const inner: any = this.inner;
    if (typeof inner.create_player !== "function") {
      throw new Error(
        "Current WASM build does not expose create_player; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.create_player(name) as PlayerId;
  }

  /** Add an instance to a player with optional InstanceCfg */
  addInstance(player: PlayerId, anim: AnimId, cfg?: unknown): InstId {
    const inner: any = this.inner;
    if (typeof inner.add_instance !== "function") {
      throw new Error(
        "Current WASM build does not expose add_instance; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.add_instance(player as number, anim as number, (cfg ?? undefined) as any) as InstId;
  }

  /**
   * Resolve canonical target paths using a JS resolver callback.
   * The resolver should return string | number | null/undefined.
   */
  prebind(resolver: (path: string) => string | number | null | undefined): void {
    const inner: any = this.inner;
    if (typeof inner.prebind !== "function") {
      throw new Error(
        "Current WASM build does not expose prebind; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    inner.prebind(resolver as any);
  }

  /** Step the simulation by dt (seconds) with optional Inputs; returns Outputs */
  update(dt: number, inputs?: Inputs): Outputs {
    return (this.inner.update(dt, (inputs ?? undefined) as any) as unknown) as Outputs;
  }
}

/* -----------------------------------------------------------
   Convenience factory
----------------------------------------------------------- */

export async function createEngine(config?: Config): Promise<Engine> {
  await init();
  return new Engine(config);
}

/* -----------------------------------------------------------
   Backward-compat exports for legacy consumers
   - default export `init`
   - alias `Animation` pointing to `VizijAnimation`
----------------------------------------------------------- */

export default init;
// Deprecated: prefer `Engine` wrapper. Kept temporarily for legacy code.
export { VizijAnimation as Animation };
