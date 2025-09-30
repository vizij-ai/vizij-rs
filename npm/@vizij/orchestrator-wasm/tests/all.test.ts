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
            id: "anim_input",
            type: "input",
            params: {
              path: "demo/animation.value",
              value: { type: "float", data: 0 },
            },
          },
          {
            id: "gain_input",
            type: "input",
            params: {
              path: "demo/graph/gain",
              value: { type: "float", data: 1.5 },
            },
          },
          {
            id: "offset_input",
            type: "input",
            params: {
              path: "demo/graph/offset",
              value: { type: "float", data: 0.25 },
            },
          },
          {
            id: "scaled",
            type: "multiply",
            inputs: {
              a: { node_id: "anim_input" },
              b: { node_id: "gain_input" },
            },
          },
          {
            id: "output_sum",
            type: "add",
            inputs: {
              lhs: { node_id: "scaled" },
              rhs: { node_id: "offset_input" },
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
        inputs: [
          "demo/animation.value",
          "demo/graph/gain",
          "demo/graph/offset",
        ],
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
          duration: 2001,
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

    // manually seed gain/offset once
    orch.setInput("demo/graph/gain", { type: "float", data: 1.5 });
    orch.setInput("demo/graph/offset", { type: "float", data: 0.25 });

    // Step 1: animation still at initial ramp value (0) -> graph output should be offset (0.25)
    let frame = orch.step(0.0) as any;
    let merged = frame.merged_writes;
    assert.ok(Array.isArray(merged));
    const animWrite0 = merged.find((w: any) => w.path === "demo/animation.value");
    const graphWrite0 = merged.find((w: any) => w.path === "demo/output/value");
    assert.ok(animWrite0, "animation write should exist");
    assert.ok(graphWrite0, "graph write should exist");
    const animVal0 = animWrite0.value?.data ?? animWrite0.value;
    const graphVal0 = graphWrite0.value?.data ?? graphWrite0.value;
    assert.ok(Math.abs(animVal0 - 0.0) < 1e-6, `expected animation 0, got ${animVal0}`);
    assert.ok(Math.abs(graphVal0 - 0.25) < 1e-6, `expected graph 0.25, got ${graphVal0}`);

    // Step 2: animation at ~0.5 -> graph output should be 0.5 * 1.5 + 0.25 = 1.0
    frame = orch.step(1.0) as any;
    merged = frame.merged_writes;
    const animWrite1 = merged.find((w: any) => w.path === "demo/animation.value");
    const graphWrite1 = merged.find((w: any) => w.path === "demo/output/value");
    const animVal1 = animWrite1.value?.data ?? animWrite1.value;
    const graphVal1 = graphWrite1.value?.data ?? graphWrite1.value;
    assert.ok(Math.abs(animVal1 - 0.5) < 1e-3, `expected animation 0.5, got ${animVal1}`);
    assert.ok(Math.abs(graphVal1 - 1.0) < 1e-3, `expected graph 1.0, got ${graphVal1}`);

    // Step 3: animation at ~1.0 -> graph output should be 1.0 * 1.5 + 0.25 = 1.75
    frame = orch.step(1.0) as any;
    merged = frame.merged_writes;
    const animWrite2 = merged.find((w: any) => w.path === "demo/animation.value");
    const graphWrite2 = merged.find((w: any) => w.path === "demo/output/value");
    const animVal2 = animWrite2.value?.data ?? animWrite2.value;
    const graphVal2 = graphWrite2.value?.data ?? graphWrite2.value;
    assert.ok(Math.abs(animVal2 - 1.0) < 1e-3, `expected animation 1.0, got ${animVal2}`);
    assert.ok(Math.abs(graphVal2 - 1.75) < 1e-3, `expected graph 1.75, got ${graphVal2}`);

    // Simple sanity print
    // eslint-disable-next-line no-console
    console.log("Orchestrator shim basic smoke test passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
