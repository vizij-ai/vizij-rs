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
