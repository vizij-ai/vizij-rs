/**
 * Node entrypoint for `@vizij/animation-module`: the vizij animation engine
 * packaged as an Arora wasm module, shipped as importable assets. The default
 * export condition reads the artifact from the package directory; browsers
 * (the `browser` condition) fetch it instead.
 *
 * Load it into a device with `@vizij/arora-web-wasm`:
 * ```ts
 * import { loadAnimationModule } from "@vizij/animation-module";
 * const device = await startDevice(graph, undefined, [await loadAnimationModule()]);
 * ```
 */
import { readFile } from "node:fs/promises";
import { headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

export { headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

/** Read the packaged artifact: the module's header (JSON) + wasm bytes. */
export async function loadAnimationModule(): Promise<AnimationModule> {
  const [headerJson, wasm] = await Promise.all([readFile(headerUrl, "utf8"), readFile(wasmUrl)]);
  return { headerJson, wasmBytes: new Uint8Array(wasm) };
}
