/**
 * Minimal Node.js type shims to satisfy TypeScript in this package without adding @types/node.
 * These are used only for optional Node code paths in init().
 */

declare var process: any;

declare module "node:fs/promises" {
  export const readFile: (path: string | URL) => Promise<Uint8Array>;
}

declare module "node:url" {
  export const fileURLToPath: (url: string | URL) => string;
  export const pathToFileURL: (path: string) => URL;
}

declare module "node:path" {
  export const dirname: (p: string) => string;
  export const resolve: (...paths: string[]) => string;
}

declare module "node:fs" {
  export const readFileSync: (
    path: string | URL,
    options?: { encoding?: BufferEncoding } | BufferEncoding
  ) => string;
  export const existsSync: (path: string | URL) => boolean;
}

declare module "node:assert/strict" {
  interface AssertFn {
    (value: unknown, message?: string): asserts value;
    ok(value: unknown, message?: string): asserts value;
    equal(actual: unknown, expected: unknown, message?: string): void;
    strictEqual(actual: unknown, expected: unknown, message?: string): void;
    fail(message?: string): never;
  }
  const assert: AssertFn;
  export default assert;
}

declare module "@vizij/wasm-loader" {
  export type InitInput = unknown;

  export interface LoadBindingsOptions<TBindings> {
    cache: { current: TBindings | null };
    importModule: () => Promise<any>;
    defaultWasmUrl: () => URL;
    init: (module: any, initArg: unknown) => Promise<void>;
    getBindings?: (module: any) => TBindings;
    expectedAbi?: number;
    getAbiVersion?: (bindings: TBindings) => number;
  }

  export function loadBindings<TBindings>(
    options: LoadBindingsOptions<TBindings>,
    initInput?: InitInput
  ): Promise<TBindings>;
}
