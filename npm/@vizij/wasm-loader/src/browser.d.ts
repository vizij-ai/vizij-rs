import { type InitInput, type LoadBindingsOptions } from "./shared.js";
export declare function loadBindings<TBindings>(options: LoadBindingsOptions<TBindings>, initInput?: InitInput): Promise<TBindings>;
export type { InitInput, LoadBindingsOptions } from "./shared.js";
