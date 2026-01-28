/**
 * Accepted wasm init inputs passed to wasm-bindgen's init.
 *
 * Strings are treated as URLs; use `file://` URLs to load from disk in Node.
 */
export type InitInput =
  | string
  | URL
  | ArrayBufferView
  | ArrayBuffer
  | WebAssembly.Module
  | Response;

/**
 * Configure how @vizij/wasm-loader imports and initializes a wasm module.
 *
 * @typeParam TBindings - The bindings type returned by the wasm-pack JS shim.
 */
export interface LoadBindingsOptions<TBindings> {
  /** Mutable cache slot shared by the caller to memoize bindings. */
  cache: { current: TBindings | null };
  /** Dynamic import of the wasm-pack JS shim (usually `import("./pkg/...")`). */
  importModule: () => Promise<any>;
  /** Default URL for the `.wasm` binary (used when `initInput` is undefined). */
  defaultWasmUrl: () => URL | string;
  /** Initializer called with the wasm module and init argument. */
  init: (module: any, initArg: unknown) => Promise<void>;
  /** Extract bindings from the imported module (defaults to the module itself). */
  getBindings?: (module: any) => TBindings;
  /** Optional expected ABI version. */
  expectedAbi?: number;
  /** ABI version accessor for the bindings. */
  getAbiVersion?: (bindings: TBindings) => number;
}

/**
 * Load, initialize, and cache wasm bindings with optional ABI validation.
 *
 * @param options - Loader configuration (cache, import, init, ABI guard).
 * @param initInput - Optional init argument (URL, bytes, module, response).
 * @param maybeReadFileBytes - Hook to resolve file:// URLs in Node.
 * @returns The initialized bindings, cached for subsequent calls.
 * @throws Error when ABI validation fails or the init call rejects.
 */
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
  initArg = await maybeReadFileBytes(initArg);

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
