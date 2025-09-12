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
}

declare module "node:path" {
  export const dirname: (p: string) => string;
  export const resolve: (...paths: string[]) => string;
}
