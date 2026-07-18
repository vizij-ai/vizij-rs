/** The two parts an Arora runtime loads a guest module from. */
export interface AnimationModule {
  /** The module's Arora header, as JSON. */
  headerJson: string;
  /** The module's wasm executable. */
  wasmBytes: Uint8Array;
}

/**
 * The module's Arora header (JSON), inlined at artifact-build time — loaders
 * use it directly, with no asset fetch involved.
 */
export { headerJson } from "./header-json.generated.js";

/** URL of the packaged module header (JSON), for tooling that wants the file
 * itself; loaders use the inlined {@link headerJson} instead. */
export const headerUrl = new URL("../artifact/header.json", import.meta.url);

/** URL of the packaged module executable (`wasm32-wasip1`). */
export const wasmUrl = new URL("../artifact/vizij_animation_module.wasm", import.meta.url);
