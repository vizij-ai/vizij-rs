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

process.env.RUST_BACKTRACE = "1";

(async () => {
  try {
    await init(pkgWasmUrl());

    const orch = await createOrchestrator({ schedule: "SinglePass" });

    const graphSpec = {
      spec: {
        nodes: [
      {
        id: "input",
        type: "input",
        params: {
          path: "demo/input/value",
          value: { type: "float", data: 0 },
        },
      },
          {
            id: "gain",
            type: "constant",
            params: { value: { type: "float", data: 1.5 } },
          },
          {
            id: "scaled",
            type: "multiply",
            inputs: {
              a: { node_id: "input" },
              b: { node_id: "gain" },
            },
          },
          {
            id: "offset_constant",
            type: "constant",
            params: { value: { type: "float", data: 0.25 } },
          },
          {
            id: "output_sum",
            type: "add",
            inputs: {
              lhs: { node_id: "scaled" },
              rhs: { node_id: "offset_constant" },
            },
          },
          {
            id: "out",
            type: "output",
            params: { path: "demo/output/value" },
            inputs: { in: { node_id: "output_sum" } },
          },
        ],
      },
      subs: {
        inputs: ["demo/input/value"],
        outputs: ["demo/output/value"],
      },
    };

    const graphId = orch.registerGraph(graphSpec);
    assert.ok(typeof graphId === "string" && graphId.length > 0);

    const animationConfig = {
      setup: {
        animation: {
          id: "demo-ramp",
          name: "Demo Ramp",
          duration: 2000,
          groups: [],
          tracks: [
            {
              id: "ramp-track",
              name: "Ramp Value",
              animatableId: "demo/animation.value",
              points: [
                { id: "start", stamp: 0, value: 0 },
                { id: "end", stamp: 1, value: 1 },
              ],
            },
          ],
        },
        player: {
          name: "demo-player",
          loop_mode: "loop" as const,
        },
      },
    };

    const animId = orch.registerAnimation(animationConfig);
    assert.ok(typeof animId === "string" && animId.length > 0);

    // Set an input and step the orchestrator
    orch.setInput("demo/input/value", { type: "float", data: 0.5 });
    const frame = orch.step(1 / 60) as any;

    // Basic assertions on returned frame shape
    assert.ok(typeof frame.epoch === "number");
    assert.ok(typeof frame.dt === "number");
    assert.ok(Array.isArray(frame.merged_writes));
    assert.ok(typeof frame.timings_ms === "object");
    assert.ok(Array.isArray(frame.conflicts));
    assert.ok(Array.isArray(frame.events));

    // If merged_writes contains entries, ensure expected fields exist
    const paths = new Set(frame.merged_writes.map((w: any) => w.path));
    assert.ok(paths.has("demo/output/value"), "graph output should be present");
    assert.ok(paths.has("demo/animation.value"), "animation output should be present");

    // Simple sanity print
    // eslint-disable-next-line no-console
    console.log("Orchestrator shim basic smoke test passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
