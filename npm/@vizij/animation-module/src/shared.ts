/** What an Arora runtime loads the guest module from. */
export interface AnimationModule {
  /** The module's Arora header, as JSON. */
  headerJson: string;
  /** The module's wasm executable. */
  wasmBytes: Uint8Array;
  /**
   * The module's Arora records: each record file's path under `records/`
   * (`module/<id>@<version>.yaml`, `structure/…`, `folder/<id>.yaml`) mapped
   * to its YAML. A device given them describes the exports that take or
   * return a structure (`step_values`, `player_states`, …), which a header
   * alone names without describing; `arora-web`'s `withRecords` takes this
   * map as it is.
   */
  records: Record<string, string>;
}

/**
 * The module's Arora header (JSON), inlined at artifact-build time — loaders
 * use it directly, with no asset fetch involved.
 */
export { headerJson } from "./header-json.generated.js";

/**
 * The module's Arora records (see {@link AnimationModule.records}), inlined at
 * artifact-build time. The same files ship under `artifact/records/`, the
 * directory a device's module directory carries beside its header.
 */
export { records } from "./records.generated.js";

/** URL of the packaged module header (JSON), for tooling that wants the file
 * itself; loaders use the inlined {@link headerJson} instead. */
export const headerUrl = new URL("../artifact/header.json", import.meta.url);

export { wasmUrl } from "./wasm-url.generated.js";
