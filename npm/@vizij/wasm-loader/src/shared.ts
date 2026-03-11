/**
 * Shared wasm binding loader primitives used by the Vizij npm wrappers.
 *
 * The helpers in this module coordinate module import, initialization, ABI checks, and cached
 * binding reuse across the browser and Node-facing loader entrypoints.
 */
type InitInputValue =
  | string
  | URL
  | ArrayBufferView
  | ArrayBuffer
  | WebAssembly.Module
  | Response;

export type InitInput =
  | InitInputValue
  | { module_or_path: InitInputValue };

export interface LoadBindingsOptions<TBindings> {
  cache: { current: TBindings | null };
  importModule: () => Promise<any>;
  defaultWasmUrl: () => URL | string;
  init: (module: any, initArg: unknown) => Promise<void>;
  getBindings?: (module: any) => TBindings;
  expectedAbi?: number;
  getAbiVersion?: (bindings: TBindings) => number;
}

export async function loadBindingsInternal<TBindings>(
  options: LoadBindingsOptions<TBindings>,
  initInput: InitInput | undefined,
  maybeReadFileBytes: (initArg: unknown) => Promise<unknown>,
): Promise<TBindings> {
  if (options.cache.current) {
    return options.cache.current;
  }

  const module = await options.importModule();
  let initArg: unknown =
    typeof initInput === "undefined"
      ? options.defaultWasmUrl()
      : initInput;
  if (
    initArg &&
    typeof initArg === "object" &&
    "module_or_path" in (initArg as Record<string, unknown>)
  ) {
    const moduleOrPath = await maybeReadFileBytes(
      (initArg as { module_or_path: unknown }).module_or_path,
    );
    initArg = { module_or_path: moduleOrPath };
  } else {
    initArg = await maybeReadFileBytes(initArg);
  }

  await options.init(module, initArg);

  const bindings: TBindings = options.getBindings
    ? options.getBindings(module)
    : ((module as unknown) as TBindings);

  if (
    typeof options.expectedAbi === "number" &&
    typeof options.getAbiVersion === "function"
  ) {
    const abi = options.getAbiVersion(bindings);
    if (abi !== options.expectedAbi) {
      throw new Error(
        `@vizij/wasm-loader ABI mismatch: expected ${options.expectedAbi}, got ${abi}. ` +
          "Rebuild the wasm package and ensure bindings are up to date.",
      );
    }
  }

  options.cache.current = bindings;
  return bindings;
}
