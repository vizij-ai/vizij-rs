// Registers vizij-web's own TypeScript loader (read-only use) so the render
// package sources and the authoring app utils import untouched, plus a shim
// that serves lodash's named exports to ESM.
import { register } from "node:module";
import { pathToFileURL } from "node:url";
const LOADER = "/Users/victor.paleologue/Code/Semio/vizij-web/packages/@vizij/render/tests/node-ts-loader.mjs";
register(pathToFileURL(LOADER), pathToFileURL(LOADER));
register(new URL("./cjs-named-exports-loader.mjs", import.meta.url), import.meta.url);
