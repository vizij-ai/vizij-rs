/**
 * Stable ESM entrypoint for `@vizij/animation-wasm`.
 *
 * This wrapper initializes the wasm-bindgen package, enforces ABI compatibility, and exposes
 * an ergonomic `Engine` class plus the shared value helpers used by Vizij hosts.
 */
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";
import type {
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
  ChangeWithDerivative,
  AnimationInfo,
  PlayerInfo,
  InstanceInfo,
  BakedAnimationData,
  BakedDerivativeAnimationData,
  BakedAnimationBundle,
} from "./types";

export type {
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
  ChangeWithDerivative,
  AnimationInfo,
  PlayerInfo,
  InstanceInfo,
  BakedAnimationData,
  BakedDerivativeAnimationData,
  BakedAnimationBundle,
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

/**
 * Return the ABI version reported by the loaded animation wasm module.
 *
 * This is mainly useful when diagnosing local rebuild mismatches between the wrapper and the
 * generated `pkg/` artifacts. Call `init()` successfully before reading it.
 */
export function abi_version(): number {
  if (!bindingCache.current) {
    throw new Error("Call init() from @vizij/animation-wasm before reading abi_version().");
  }
  return Number(bindingCache.current.abi_version());
}

/* -----------------------------------------------------------
   Shared wasm loader
----------------------------------------------------------- */

type WasmAnimationCtor = new (config?: unknown) => unknown;

type WasmBindings = {
  default: (input?: unknown) => Promise<unknown>;
  VizijAnimation: WasmAnimationCtor;
  abi_version: () => number;
};

const bindingCache: { current: WasmBindings | null } = { current: null };
let wasmModulePromise: Promise<WasmBindings | unknown> | null = null;
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
  return new URL("../../pkg/vizij_animation_wasm.js", import.meta.url);
}

function importStaticWasmModule(): Promise<unknown> {
  return import("../../pkg/vizij_animation_wasm.js");
}

function importDynamicWasmModule(): Promise<unknown> {
  return import(/* @vite-ignore */ pkgWasmJsUrl().toString());
}

async function importWasmModule(): Promise<unknown> {
  if (!wasmModulePromise) {
    wasmModulePromise = importStaticWasmModule().catch((err) => {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn(
          "@vizij/animation-wasm: static wasm import failed, falling back to runtime URL import.",
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
    wasmUrlCache = new URL("../../pkg/vizij_animation_wasm_bg.wasm", import.meta.url).toString();
  }
  return wasmUrlCache;
}

const loadBindingsImpl =
  typeof window === "undefined"
    ? loadWasmBindings
    : (loadWasmBindingsBrowser as typeof loadWasmBindings);

export type InitInput = LoaderInitInput;

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await loadBindingsImpl<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => importWasmModule(),
      defaultWasmUrl,
      init: async (module: unknown, initArg: unknown) => {
        const typed = module as WasmBindings;
        await typed.default(toWasmBindgenInitOptions(initArg));
      },
      getBindings: (module: unknown) => module as WasmBindings,
      expectedAbi: 2,
      getAbiVersion: (bindings) => Number(bindings.abi_version()),
    },
    input
  );

  return bindingCache.current!;
}

/* -----------------------------------------------------------
   init() — initialize the wasm module once (parity with node-graph)
----------------------------------------------------------- */

let _initPromise: Promise<void> | null = null;

/**
 * Initialize the wasm module once.
 *
 * The returned promise is memoized, so repeated calls reuse the same wasm instance. Most hosts
 * should await this during startup before constructing `Engine` or `VizijAnimation`.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;

  _initPromise = (async () => {
    await loadBindings(input);
  })();

  return _initPromise;
}

function ensureInited(): void {
  if (!_initPromise) {
    throw new Error(
      "Call init() from @vizij/animation-wasm before creating Engine or VizijAnimation."
    );
  }
  if (!bindingCache.current) {
    throw new Error("WASM bindings were not initialized correctly.");
  }
}

/* -----------------------------------------------------------
   Ergonomic wrapper — Engine (parity with node-graph Graph)
----------------------------------------------------------- */

/**
 * High-level animation wrapper around the wasm runtime.
 *
 * Use this class for normal application code: loading clips, creating players, attaching
 * instances, stepping time, and consuming emitted value changes/events. Prefer this over the raw
 * `VizijAnimation` binding unless you specifically need the low-level surface.
 */
export class Engine {
  private inner: any;

  /**
   * Create an animation engine wrapper bound to the already-initialized wasm module.
   *
   * `config` is forwarded to the underlying runtime constructor as-is.
   */
  constructor(config?: Config) {
    ensureInited();
    const bindings = bindingCache.current!;
    const Ctor = bindings.VizijAnimation;
    this.inner = new Ctor(config as any);
  }

  /**
   * Load an animation clip into the engine. If `opts.format` is omitted,
   * this will auto-detect `"stored"` when `tracks` is present on the object.
   *
   * The returned `AnimId` is later used by `addInstance()`, `bakeAnimation()`, and
   * `unloadAnimation()`.
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

  /**
   * Create a new player and return its opaque player id.
   *
   * The `name` is for host/debug readability; it does not need to be unique.
   */
  createPlayer(name: string): PlayerId {
    const inner: any = this.inner;
    if (typeof inner.create_player !== "function") {
      throw new Error(
        "Current WASM build does not expose create_player; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.create_player(name) as PlayerId;
  }

  /**
   * Attach a loaded animation to a player and return the created instance id.
   *
   * Use `cfg` to pass optional instance settings such as weight, time scaling, or start offset
   * when the underlying wasm build supports them.
   */
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
   * Resolve canonical animation target paths to host-specific keys before stepping.
   *
   * The resolver should return a stable string or number handle. Returning `null`/`undefined`
   * keeps the original canonical path, which will then appear in emitted change records.
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

  /**
   * Advance the engine by `dt` seconds and return the value changes/events for that tick.
   *
   * Optional `inputs` are applied before the engine advances time, so player commands and instance
   * updates affect the same returned frame.
   */
  updateValues(dt: number, inputs?: Inputs): Outputs {
    const inner: any = this.inner;
    if (typeof inner.update_values !== "function") {
      throw new Error(
        "Current WASM build does not expose update_values; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.update_values(dt, (inputs ?? undefined) as any) as Outputs;
  }

  /**
   * Advance the engine by `dt` seconds and return both emitted changes and derivative samples.
   *
   * Use this when a host needs per-output velocity-style data in addition to the sampled values.
   */
  updateValuesAndDerivatives(dt: number, inputs?: Inputs): OutputsWithDerivatives {
    const inner: any = this.inner;
    if (typeof inner.update_values_and_derivatives !== "function") {
      throw new Error(
        "Current WASM build does not expose update_values_and_derivatives; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.update_values_and_derivatives(dt, (inputs ?? undefined) as any) as OutputsWithDerivatives;
  }

  /**
   * Backwards-compatible alias for `updateValues()`.
   */
  update(dt: number, inputs?: Inputs): Outputs {
    return this.updateValues(dt, inputs);
  }

  /**
   * Bake a loaded animation clip into pre-sampled tracks. The returned object
   * mirrors `vizij-animation-core`'s `BakedAnimationData` schema.
   *
   * Baking does not require a player or instance; it operates directly on the loaded clip id.
   */
  bakeAnimation(anim: AnimId, cfg?: BakingConfig): BakedAnimationData {
    const inner: any = this.inner;
    if (typeof inner.bake_animation !== "function") {
      throw new Error(
        "Current WASM build does not expose bake_animation; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.bake_animation(anim as number, (cfg ?? undefined) as any) as BakedAnimationData;
  }

  /**
   * Bake a loaded animation clip into sampled values plus derivative tracks.
   *
   * This is the offline/export-oriented companion to `updateValuesAndDerivatives()`.
   */
  bakeAnimationWithDerivatives(anim: AnimId, cfg?: BakingConfig): BakedAnimationBundle {
    const inner: any = this.inner;
    if (typeof inner.bake_animation_with_derivatives !== "function") {
      throw new Error(
        "Current WASM build does not expose bake_animation_with_derivatives; rebuild vizij-animation-wasm with updated bindings."
      );
    }
    return inner.bake_animation_with_derivatives(
      anim as number,
      (cfg ?? undefined) as any,
    ) as BakedAnimationBundle;
  }

  /**
   * Remove a player and all instances attached to it.
   *
   * Returns `false` when the player id is unknown.
   */
  removePlayer(player: PlayerId): boolean {
    const inner: any = this.inner;
    if (typeof inner.remove_player !== "function") {
      throw new Error("remove_player not available; rebuild vizij-animation-wasm");
    }
    return !!inner.remove_player(player as number);
  }

  /**
   * Remove one instance from a player.
   *
   * Returns `false` when either the player or instance id is unknown.
   */
  removeInstance(player: PlayerId, inst: InstId): boolean {
    const inner: any = this.inner;
    if (typeof inner.remove_instance !== "function") {
      throw new Error("remove_instance not available; rebuild vizij-animation-wasm");
    }
    return !!inner.remove_instance(player as number, inst as number);
  }

  /**
   * Unload a previously loaded animation clip.
   *
   * Any instances that still reference the clip are detached by the underlying runtime.
   */
  unloadAnimation(anim: AnimId): boolean {
    const inner: any = this.inner;
    if (typeof inner.unload_animation !== "function") {
      throw new Error("unload_animation not available; rebuild vizij-animation-wasm");
    }
    return !!inner.unload_animation(anim as number);
  }

  /**
   * Return the currently loaded animations and their runtime metadata.
   */
  listAnimations(): AnimationInfo[] {
    const inner: any = this.inner;
    if (typeof inner.list_animations !== "function") {
      throw new Error("list_animations not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_animations() as unknown) as AnimationInfo[];
  }

  /**
   * Return the currently registered players and their playback state.
   */
  listPlayers(): PlayerInfo[] {
    const inner: any = this.inner;
    if (typeof inner.list_players !== "function") {
      throw new Error("list_players not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_players() as unknown) as PlayerInfo[];
  }

  /**
   * Return the instances currently attached to `player`.
   */
  listInstances(player: PlayerId): InstanceInfo[] {
    const inner: any = this.inner;
    if (typeof inner.list_instances !== "function") {
      throw new Error("list_instances not available; rebuild vizij-animation-wasm");
    }
    return (inner.list_instances(player as number) as unknown) as InstanceInfo[];
  }

  /**
   * Return the resolved output keys currently associated with a player's instances.
   *
   * When `prebind()` mapped canonical target paths to host handles, those resolved keys are what
   * appear here.
   */
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

/**
 * Initialize the wasm module if needed and return a ready-to-use `Engine`.
 */
export async function createEngine(config?: Config): Promise<Engine> {
  await init();
  return new Engine(config);
}

export {
  listAnimationFixtures,
  loadAnimationFixture,
  loadAnimationJson,
  resolveAnimationPath,
} from "./fixtures.js";

/* -----------------------------------------------------------
   Backward-compat exports for legacy consumers
   - default export `init`
   - alias `Animation` pointing to `VizijAnimation`
----------------------------------------------------------- */

export default init;
/**
 * Deprecated compatibility alias for the raw `VizijAnimation` binding.
 *
 * New code should prefer `Engine`, which provides the stable consumer-oriented wrapper surface.
 */
export { VizijAnimation as Animation };
/**
 * Raw wasm-bindgen animation binding exposed for legacy consumers.
 *
 * Prefer `Engine` for new code. This surface mirrors the underlying wasm methods more directly and
 * therefore exposes fewer TypeScript-level ergonomics and guarantees.
 */
export const VizijAnimation: WasmAnimationCtor = new Proxy(
  function () {},
  {
    construct(_target: () => void, args: any[]): object {
      ensureInited();
      if (!bindingCache.current) {
        throw new Error("Call init() from @vizij/animation-wasm before constructing VizijAnimation.");
      }
      const Inner = bindingCache.current.VizijAnimation as unknown as WasmAnimationCtor;
      return new Inner(...(args as any[])) as object;
    },
  }
) as unknown as WasmAnimationCtor;
