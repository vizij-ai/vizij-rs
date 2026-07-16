/** The two parts an Arora runtime loads a guest module from. */
export interface AnimationModule {
  /** The module's Arora header, as JSON. */
  headerJson: string;
  /** The module's wasm executable. */
  wasmBytes: Uint8Array;
}

/** URL of the packaged module header (JSON). */
export const headerUrl = new URL("../artifact/header.json", import.meta.url);

/** URL of the packaged module executable (`wasm32-wasip1`). */
export const wasmUrl = new URL("../artifact/vizij_animation_module.wasm", import.meta.url);
