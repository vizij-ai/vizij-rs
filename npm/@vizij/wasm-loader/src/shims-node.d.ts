declare module "node:fs/promises" {
  export const readFile: (
    path: string | URL,
    options?: { encoding?: BufferEncoding } | BufferEncoding
  ) => Promise<Uint8Array>;
}

declare module "node:url" {
  export const fileURLToPath: (url: string | URL) => string;
}
