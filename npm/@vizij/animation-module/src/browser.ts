/**
 * Browser entrypoint for `@vizij/animation-module`: same surface as the Node
 * entrypoint, fetching the packaged artifact over HTTP (bundlers rewrite the
 * `new URL(..., import.meta.url)` asset references).
 */
import { headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

export { headerUrl, wasmUrl, type AnimationModule } from "./shared.js";

async function fetched(url: URL): Promise<Response> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`fetch ${url} failed: ${response.status} ${response.statusText}`);
  }
  return response;
}

/** Fetch the packaged artifact: the module's header (JSON) + wasm bytes. */
export async function loadAnimationModule(): Promise<AnimationModule> {
  const [headerJson, wasm] = await Promise.all([
    fetched(headerUrl).then((r) => r.text()),
    fetched(wasmUrl).then((r) => r.arrayBuffer()),
  ]);
  return { headerJson, wasmBytes: new Uint8Array(wasm) };
}
