/**
 * Browser entrypoint for `@vizij/animation-module`: same surface as the Node
 * entrypoint. The header and records ship inlined in the JS; only the wasm
 * executable is fetched (bundlers rewrite the `new URL(..., import.meta.url)`
 * asset reference).
 */
import { headerJson, records, wasmUrl, type AnimationModule } from "./shared.js";

export { headerJson, headerUrl, records, wasmUrl, type AnimationModule } from "./shared.js";

/** Load the packaged artifact: the inlined header (JSON) and records, and the
 * fetched wasm bytes. */
export async function loadAnimationModule(): Promise<AnimationModule> {
  const response = await fetch(wasmUrl);
  if (!response.ok) {
    throw new Error(`fetch ${wasmUrl} failed: ${response.status} ${response.statusText}`);
  }
  const wasm = await response.arrayBuffer();
  return { headerJson, wasmBytes: new Uint8Array(wasm), records };
}
