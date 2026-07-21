/**
 * Node entrypoint for `@vizij/animation-module`: the vizij animation engine
 * packaged as an Arora wasm module, shipped as importable assets. The default
 * export condition reads the artifact from the package directory; browsers
 * (the `browser` condition) fetch it instead.
 *
 * Load it into a device with `@vizij/runtime`:
 * ```ts
 * import { loadAnimationModule } from "@vizij/animation-module";
 * const device = await startDevice(graph, undefined, [await loadAnimationModule()]);
 * ```
 */
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { headerJson, wasmUrl, type AnimationModule } from "./shared.js";

export { headerJson, headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

/** Load the packaged artifact: the inlined header (JSON) + wasm bytes read
 * from the package directory. (Bytes via single-arg `readFile` on a filesystem
 * path: types cleanly whether `node:fs/promises` resolves to the real
 * `@types/node` overloads or the minimal node shims the sibling wasm packages
 * declare.) */
export async function loadAnimationModule(): Promise<AnimationModule> {
  const wasmBytes = await readFile(fileURLToPath(wasmUrl));
  return { headerJson, wasmBytes: new Uint8Array(wasmBytes) };
}
