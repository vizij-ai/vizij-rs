import {
  loadBindingsInternal,
  type InitInput,
  type LoadBindingsOptions,
} from "./shared.js";

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
    if (typeof window !== "undefined") {
      return initArg;
    }
    const maybeProcess = (globalThis as any)?.process;
    if (!maybeProcess?.versions?.node) {
      return initArg;
    }
    const importDynamic = new Function(
      "specifier",
      "return import(specifier);",
    ) as (specifier: string) => Promise<any>;
    const [{ readFile }, { fileURLToPath }] = await Promise.all([
      importDynamic("fs/promises"),
      importDynamic("url"),
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

/**
 * Load wasm bindings with Node/browser-aware defaults and ABI validation.
 */
export async function loadBindings<TBindings>(
  options: LoadBindingsOptions<TBindings>,
  initInput?: InitInput
): Promise<TBindings> {
  return loadBindingsInternal(options, initInput, maybeReadFileBytes);
}

export type { InitInput, LoadBindingsOptions } from "./shared.js";
