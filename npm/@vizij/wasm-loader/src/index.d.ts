import { type InitInput, type LoadBindingsOptions } from "./shared.js";
/**
 * Load wasm bindings with Node/browser-aware defaults and ABI validation.
 */
export declare function loadBindings<TBindings>(options: LoadBindingsOptions<TBindings>, initInput?: InitInput): Promise<TBindings>;
export type { InitInput, LoadBindingsOptions } from "./shared.js";
