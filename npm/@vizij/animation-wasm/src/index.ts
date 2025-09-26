// Stable ESM entry for @vizij/animation-wasm
// Wraps the wasm-pack output in ../pkg (built with `--target web`).
import initWasm, { VizijAnimation, abi_version } from "../pkg/vizij_animation_wasm.js";

import type {
  InitInput,
  Config,
  BakingConfig,
  Inputs,
  InstanceUpdate,
  Outputs,
  OutputsWithDerivatives,
  AnimationData,
  StoredAnimation,
  AnimId,
  PlayerId,
  InstId,
  Value,
  CoreEvent,
  Change,
  DerivativeChange,
  AnimationInfo,
  PlayerInfo,
  InstanceInfo,
  BakedAnimationData,
  BakedDerivativeAnimation,
  BakedDerivativeTrack,
  BakedTrack,
} from "./types";

export type {
  InitInput,
  Config,
  BakingConfig,
  Inputs,
  InstanceUpdate,
  Outputs,
  OutputsWithDerivatives,
  AnimationData,
  StoredAnimation,
  AnimId,
  PlayerId,
  InstId,
  Value,
  CoreEvent,
  Change,
  DerivativeChange,
  AnimationInfo,
  PlayerInfo,
  InstanceInfo,
  BakedAnimationData,
  BakedDerivativeAnimation,
  BakedDerivativeTrack,
  BakedTrack,
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
    return this.updateValues(dt, inputs);
  }

  /** Step the simulation and return only blended animation values. */
  updateValues(dt: number, inputs?: Inputs): Outputs {
    const inner: any = this.inner;
    if (typeof inner.update_values !== "function") {
      throw new Error(
        "Current WASM build does not expose update_values; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.update_values(dt, (inputs ?? undefined) as any) as Outputs;
  }

  /** Step the simulation and return blended values plus derivatives. */
  updateValuesWithDerivatives(dt: number, inputs?: Inputs): OutputsWithDerivatives {
    const inner: any = this.inner;
    if (typeof inner.update_values_with_derivatives !== "function") {
      throw new Error(
        "Current WASM build does not expose update_values_with_derivatives; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.update_values_with_derivatives(
      dt,
      (inputs ?? undefined) as any
    ) as OutputsWithDerivatives;
  }

  /** Bake samples for a loaded animation clip. */
  bakeAnimation(anim: AnimId, cfg?: Partial<BakingConfig>): BakedAnimationData {
    const inner: any = this.inner;
    if (typeof inner.bake_animation !== "function") {
      throw new Error(
        "Current WASM build does not expose bake_animation; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.bake_animation(anim as number, (cfg ?? undefined) as any) as BakedAnimationData;
  }

  /** Bake samples and derivatives for a loaded animation clip. */
  bakeAnimationWithDerivatives(
    anim: AnimId,
    cfg?: Partial<BakingConfig>
  ): { animation: BakedAnimationData; derivatives: BakedDerivativeAnimation } {
    const inner: any = this.inner;
    if (typeof inner.bake_animation_with_derivatives !== "function") {
      throw new Error(
        "Current WASM build does not expose bake_animation_with_derivatives; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.bake_animation_with_derivatives(
      anim as number,
      (cfg ?? undefined) as any
    ) as {
      animation: BakedAnimationData;
      derivatives: BakedDerivativeAnimation;
    };
  }

  /** Remove a player and all its instances */
  removePlayer(player: PlayerId): boolean {
    const inner: any = this.inner;
    if (typeof inner.remove_player !== "function") {
      throw new Error("remove_player not available; rebuild vizij-animation-wasm");
    }
    return !!inner.remove_player(player as number);
  }

  /** Remove a specific instance from a player */
  removeInstance(player: PlayerId, inst: InstId): boolean {
    const inner: any = this.inner;
    if (typeof inner.remove_instance !== "function") {
      throw new Error("remove_instance not available; rebuild vizij-animation-wasm");
    }
    return !!inner.remove_instance(player as number, inst as number);
  }

  /** Unload an animation; auto-detach referencing instances */
  unloadAnimation(anim: AnimId): boolean {
    const inner: any = this.inner;
    if (typeof inner.unload_animation !== "function") {
      throw new Error("unload_animation not available; rebuild vizij-animation-wasm");
    }
    return !!inner.unload_animation(anim as number);
  }

  /** Enumerate animations in the engine */
  listAnimations(): AnimationInfo[] {
    const inner: any = this.inner;
    if (typeof inner.list_animations !== "function") {
      throw new Error("list_animations not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_animations() as unknown) as AnimationInfo[];
  }

  /** Enumerate players and playback info */
  listPlayers(): PlayerInfo[] {
    const inner: any = this.inner;
    if (typeof inner.list_players !== "function") {
      throw new Error("list_players not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_players() as unknown) as PlayerInfo[];
  }

  /** Enumerate instances for a given player */
  listInstances(player: PlayerId): InstanceInfo[] {
    const inner: any = this.inner;
    if (typeof inner.list_instances !== "function") {
      throw new Error("list_instances not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_instances(player as number) as unknown) as InstanceInfo[];
  }

  /** Enumerate resolved output keys currently associated with a player's instances */
  listPlayerKeys(player: PlayerId): string[] {
    const inner: any = this.inner;
    if (typeof inner.list_player_keys !== "function") {
      throw new Error("list_player_keys not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_player_keys(player as number) as unknown) as string[];
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
