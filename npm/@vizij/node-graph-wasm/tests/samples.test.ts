/* Runtime test to ensure samples load and evaluate with writes. */
import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import {
  init,
  Graph,
  oscillatorBasics,
  vectorPlayground,
  logicGate,
  tupleSpringDampSlew,
  type EvalResult,
} from "../src/index.js";

function pkgWasmUrl(): URL {
  const here = dirname(fileURLToPath(import.meta.url));
  const wasmPath = resolve(here, "../../pkg/vizij_graph_wasm_bg.wasm");
  if (!existsSync(wasmPath)) {
    throw new Error(
      "Missing pkg/vizij_graph_wasm_bg.wasm. Run:\n  wasm-pack build crates/node-graph/vizij-graph-wasm --target web --out-dir npm/@vizij/node-graph-wasm/pkg --release (from repo root vizij-rs/)"
    );
  }
  return pathToFileURL(wasmPath);
}

async function runSample(name: string, spec: any): Promise<void> {
  const g = new Graph();
  g.loadGraph(spec);
  g.setTime(0);
  // Two ticks to exercise dt updates and stateful nodes
  let res: EvalResult = g.evalAll();
  g.step(1 / 60);
  res = g.evalAll();

  // Basic assertions: writes present with path, value, shape
  if (!Array.isArray(res.writes) || res.writes.length === 0) {
    throw new Error(`Sample '${name}' produced no writes`);
  }
  for (const w of res.writes) {
    if (!w.path || typeof w.path !== "string") {
      throw new Error(`Sample '${name}' produced a write with invalid path`);
    }
    if (!w.value || typeof w.value !== "object") {
      throw new Error(`Sample '${name}' produced a write with invalid value`);
    }
    if (!w.shape || typeof w.shape !== "object") {
      throw new Error(`Sample '${name}' produced a write missing shape metadata`);
    }
  }

  // Ensure nodes map exists with { value, shape } snapshots
  if (!res.nodes || typeof res.nodes !== "object") {
    throw new Error(`Sample '${name}' did not return per-node outputs`);
  }
}

(async () => {
  try {
    await init(pkgWasmUrl());

    await runSample("oscillator-basics", oscillatorBasics);
    await runSample("vector-playground", vectorPlayground);
    await runSample("logic-gate", logicGate);
    await runSample("tuple-spring-damp-slew", tupleSpringDampSlew);

    // eslint-disable-next-line no-console
    console.log("Samples test passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
