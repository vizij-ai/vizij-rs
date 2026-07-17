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
import { fileURLToPath } from "node:url";
import { headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

export { headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

/** Read the packaged artifact: the module's header (JSON) + wasm bytes. */
export async function loadAnimationModule(): Promise<AnimationModule> {
  // Read both parts as bytes (single-arg `readFile`, on filesystem paths rather
  // than the URL objects) and decode the header as UTF-8 ourselves. Reading
  // bytes instead of passing an encoding keeps one code path that types cleanly
  // whether `node:fs/promises` resolves to the real `@types/node` overloads or
  // the minimal node shims the sibling wasm packages declare.
  const [headerBytes, wasmBytes] = await Promise.all([
    readFile(fileURLToPath(headerUrl)),
    readFile(fileURLToPath(wasmUrl)),
  ]);
  return {
    headerJson: new TextDecoder().decode(headerBytes),
    wasmBytes: new Uint8Array(wasmBytes),
  };
}
