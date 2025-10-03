export type InitInput = string | URL | ArrayBufferView | ArrayBuffer | WebAssembly.Module | Response;

export interface LoadBindingsOptions<TBindings> {
  cache: { current: TBindings | null };
  importModule: () => Promise<any>;
  defaultWasmUrl: () => URL;
  init: (module: any, initArg: unknown) => Promise<void>;
  getBindings?: (module: any) => TBindings;
  expectedAbi?: number;
  getAbiVersion?: (bindings: TBindings) => number;
}

async function maybeReadFileBytes(initArg: unknown): Promise<unknown> {
  const isUrlObject = typeof initArg === "object" && initArg !== null && "href" in (initArg as any);
  const href = isUrlObject
    ? (initArg as URL).href
    : typeof initArg === "string"
    ? (initArg as string)
    : "";
  const isFileUrl =
    (isUrlObject && (initArg as URL).protocol === "file:") ||
    (typeof href === "string" && href.startsWith("file:"));

  if (!isFileUrl) {
    return initArg;
  }

  try {
    const fsSpec = "node:fs/promises";
    const urlSpec = "node:url";
    const [{ readFile }, { fileURLToPath }] = await Promise.all([
      import(/* @vite-ignore */ fsSpec),
      import(/* @vite-ignore */ urlSpec),
    ]);
    const path = isUrlObject
      ? fileURLToPath(initArg as URL)
      : fileURLToPath(new URL(href));
    const bytes = await readFile(path);
    return bytes;
  } catch {
    return initArg;
  }
}

export async function loadBindings<TBindings>(
  options: LoadBindingsOptions<TBindings>,
  initInput?: InitInput
): Promise<TBindings> {
  if (options.cache.current) {
    return options.cache.current;
  }

  const module = await options.importModule();
  let initArg: unknown = typeof initInput === "undefined" ? options.defaultWasmUrl() : initInput;
  initArg = await maybeReadFileBytes(initArg);

  await options.init(module, initArg);

  const bindings: TBindings = options.getBindings
    ? options.getBindings(module)
    : ((module as unknown) as TBindings);

  if (typeof options.expectedAbi === "number" && typeof options.getAbiVersion === "function") {
    const abi = options.getAbiVersion(bindings);
    if (abi !== options.expectedAbi) {
      throw new Error(
        `@vizij/wasm-loader ABI mismatch: expected ${options.expectedAbi}, got ${abi}. ` +
          "Rebuild the wasm package and ensure bindings are up to date."
      );
    }
  }

  options.cache.current = bindings;
  return bindings;
}
