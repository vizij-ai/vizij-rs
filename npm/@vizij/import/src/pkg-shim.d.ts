declare module "../../pkg/*";

declare module "../../pkg/vizij_import_wasm.js" {
  // Minimal shim for the wasm-pack ESM bundle so src/ type-checks before
  // pkg/ is built; the wrapper narrows these through `WasmBindings`.
  const init: (input?: any) => Promise<any>;
  export default init;
}
