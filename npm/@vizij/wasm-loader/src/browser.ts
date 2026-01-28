import {
  loadBindingsInternal,
  type InitInput,
  type LoadBindingsOptions,
} from "./shared.js";

async function maybeReadFileBytes(initArg: unknown): Promise<unknown> {
  return initArg;
}

/**
 * Load wasm bindings in browser-like runtimes (no file:// handling).
 *
 * @param options - Loader configuration (cache, import, init, ABI guard).
 * @param initInput - Optional init argument (URL, bytes, module, response).
 * @returns The initialized bindings, cached for subsequent calls.
 * @throws Error when ABI validation fails or the init call rejects.
 */
export async function loadBindings<TBindings>(
  options: LoadBindingsOptions<TBindings>,
  initInput?: InitInput,
): Promise<TBindings> {
  return loadBindingsInternal(options, initInput, maybeReadFileBytes);
}

export type { InitInput, LoadBindingsOptions } from "./shared.js";
