import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import { init, createOrchestrator } from "../src/index.js";

const here = dirname(fileURLToPath(import.meta.url));

function pkgWasmUrl(): URL {
  const wasmPath = resolve(here, "../../pkg/vizij_orchestrator_wasm_bg.wasm");
  if (!existsSync(wasmPath)) {
    throw new Error(
      "Missing pkg/vizij_orchestrator_wasm_bg.wasm. Run:\n  npm run build:wasm:orchestrator (from repo root)"
    );
  }
  return pathToFileURL(wasmPath);
}

(async () => {
  try {
    await init(pkgWasmUrl());

    const orch = await createOrchestrator({ schedule: "SinglePass" });

    // Register a minimal graph controller (empty nodes)
    const graphId = orch.registerGraph({ spec: { nodes: [] } });
    assert.ok(typeof graphId === "string" && graphId.length > 0);

    // Register a minimal animation controller
    const animId = orch.registerAnimation({ setup: {} });
    assert.ok(typeof animId === "string" && animId.length > 0);

    // Set an input and step the orchestrator
    orch.setInput("test/x", { float: 0.42 });
    const frame = orch.step(1 / 60) as any;

    // Basic assertions on returned frame shape
    assert.ok(typeof frame.epoch === "number");
    assert.ok(typeof frame.dt === "number");
    assert.ok(Array.isArray(frame.merged_writes));
    assert.ok(typeof frame.timings_ms === "object");
    assert.ok(Array.isArray(frame.conflicts));
    assert.ok(Array.isArray(frame.events));

    // If merged_writes contains entries, ensure expected fields exist
    if (frame.merged_writes.length > 0) {
      const w = frame.merged_writes[0];
      assert.ok(typeof w.path === "string");
      assert.ok(typeof w.value === "object");
    }

    // Simple sanity print
    // eslint-disable-next-line no-console
    console.log("Orchestrator shim basic smoke test passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
