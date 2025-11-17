// Stable ESM entry for @vizij/blackboard-wasm
// Wraps the wasm-pack output in ../pkg (built with `--target web`).
import initWasm, * as bindings from "../pkg/vizij_blackboard_wasm.js";

export { bindings };

let _initPromise: Promise<void> | null = null;

function defaultWasmUrl(): URL {
  return new URL("../pkg/vizij_blackboard_wasm_bg.wasm", import.meta.url);
}

export type InitInput = Parameters<typeof initWasm>[0];

export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    if (typeof input !== "undefined") {
      await initWasm(input as any);
    } else if (typeof window === "undefined" && typeof process !== "undefined") {
      // Node: load bytes directly
      const { readFile } = await import("node:fs/promises");
      const { fileURLToPath } = await import("node:url");
      const { dirname, resolve } = await import("node:path");
      const wasmUrl = defaultWasmUrl();
      const wasmPath =
        wasmUrl.protocol === "file:" ? fileURLToPath(wasmUrl) : resolve(dirname(fileURLToPath(import.meta.url)), "../pkg/vizij_blackboard_wasm_bg.wasm");
      const bytes = await readFile(wasmPath);
      await initWasm(bytes);
    } else {
      await initWasm(defaultWasmUrl());
    }
  })();
  return _initPromise;
}

export default init;
