// Re-export wasm-pack output with a stable import path.
import init, { Animation, abi_version } from "../pkg/vizij_animation_wasm.js";

export default init;                 // <-- provide default export
export { init, Animation, abi_version };  // <-- and named exports

export type { AnimationConfig, AnimationOutputs } from "./types";
