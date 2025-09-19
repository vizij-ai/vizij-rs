declare module "../../pkg/vizij_graph_wasm.js" {
  // Minimal type shim for wasm-pack ESM bundle to satisfy TS in src/.
  // Runtime types are provided by the generated JS; we use `any` here.
  const init: (input?: any) => Promise<any>;
  export default init;
  export class WasmGraph {}
  export function get_node_schemas_json(): string;
  export function normalize_graph_spec_json(json: string): string;
}
