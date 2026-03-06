/**
 * Browser-oriented loader entrypoint for Vizij wasm wrapper packages.
 *
 * Browsers can pass URLs, Responses, or modules directly, so this variant forwards inputs to
 * the shared loader without any Node-specific file handling.
 */
import {
  loadBindingsInternal,
  type InitInput,
  type LoadBindingsOptions,
} from "./shared.js";

async function maybeReadFileBytes(initArg: unknown): Promise<unknown> {
  return initArg;
}

export async function loadBindings<TBindings>(
  options: LoadBindingsOptions<TBindings>,
  initInput?: InitInput,
): Promise<TBindings> {
  return loadBindingsInternal(options, initInput, maybeReadFileBytes);
}

export type { InitInput, LoadBindingsOptions } from "./shared.js";
