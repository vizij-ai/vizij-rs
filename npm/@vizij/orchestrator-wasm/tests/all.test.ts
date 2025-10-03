import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import { toValueJSON } from "@vizij/value-json";
import { init, createOrchestrator } from "../src/index.js";

const here = dirname(fileURLToPath(import.meta.url));
const fixturePath = resolve(
  here,
  "../../../../../crates/orchestrator/vizij-orchestrator-core/fixtures/demo_single_pass.json"
);
const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));

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

    const graphSpec = fixture.graph;

    const graphId = orch.registerGraph(graphSpec);
    assert.ok(typeof graphId === "string" && graphId.length > 0);

    const animationConfig = fixture.animation;

    const animId = orch.registerAnimation(animationConfig);
    assert.ok(typeof animId === "string" && animId.length > 0);

    // manually seed gain/offset once
    orch.setInput("demo/graph/gain", toValueJSON(1.5));
    orch.setInput("demo/graph/offset", toValueJSON(0.25));

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

    const scalarFromValue = (value: any): number => {
      if (value && typeof value === "object") {
        if (typeof value.data === "number") return value.data;
        if (typeof value.float === "number") return value.float;
      }
      return value as number;
    };

    const readScalar = (writes: any[], path: string): number => {
      const hit = writes.find((w: any) => w.path === path);
      assert.ok(hit, `Expected write for ${path}`);
      return scalarFromValue(hit.value);
    };

    // Chained controllers: animation -> sign graph -> slew graph.
    const chainedOrch = await createOrchestrator({ schedule: "SinglePass" });

        const signGraphSpec = {
      spec: {
        nodes: [
          {
            id: "ramp_in",
            type: "input",
            params: {
              path: "chain/ramp.value",
              value: toValueJSON(0),
            },
          },
          { id: "zero", type: "constant", params: { value: toValueJSON(0) } },
          { id: "one", type: "constant", params: { value: toValueJSON(1) } },
          { id: "neg_one", type: "constant", params: { value: toValueJSON(-1) } },
          {
            id: "gt_zero",
            type: "greaterthan",
            inputs: {
              lhs: { node_id: "ramp_in" },
              rhs: { node_id: "zero" },
            },
          },
          {
            id: "lt_zero",
            type: "lessthan",
            inputs: {
              lhs: { node_id: "ramp_in" },
              rhs: { node_id: "zero" },
            },
          },
          {
            id: "neg_or_zero",
            type: "if",
            inputs: {
              cond: { node_id: "lt_zero" },
              then: { node_id: "neg_one" },
              else: { node_id: "zero" },
            },
          },
          {
            id: "sign_value",
            type: "if",
            inputs: {
              cond: { node_id: "gt_zero" },
              then: { node_id: "one" },
              else: { node_id: "neg_or_zero" },
            },
          },
          {
            id: "sign_out",
            type: "output",
            params: { path: "chain/sign.value" },
            inputs: { in: { node_id: "sign_value" } },
          },
        ],
      },
      subs: {
        inputs: ["chain/ramp.value"],
        outputs: ["chain/sign.value"],
      },
    };

    const slewGraphSpec = {
      spec: {
        nodes: [
          {
            id: "sign_in",
            type: "input",
            params: {
              path: "chain/sign.value",
              value: toValueJSON(0),
            },
          },
          {
            id: "slew",
            type: "slew",
            inputs: { in: { node_id: "sign_in" } },
          },
          {
            id: "slew_out",
            type: "output",
            params: { path: "chain/slewed.value" },
            inputs: { in: { node_id: "slew" } },
          },
        ],
      },
      subs: {
        inputs: ["chain/sign.value"],
        outputs: ["chain/slewed.value"],
      },
    };

    const signGraphId = chainedOrch.registerGraph(signGraphSpec);
    const slewGraphId = chainedOrch.registerGraph(slewGraphSpec);
    assert.ok(signGraphId && slewGraphId);

    const chainAnimationConfig = {
      setup: {
        animation: {
          id: "chain-ramp",
          name: "Chain Ramp",
          duration: 2000,
          groups: [],
          tracks: [
            {
              id: "chain-ramp-track",
              name: "Chain Ramp Value",
              animatableId: "chain/ramp.value",
              points: [
                { id: "neg", stamp: 0, value: -1 },
                { id: "zero", stamp: 0.5, value: 0 },
                { id: "pos", stamp: 1, value: 1 },
              ],
            },
          ],
        },
        player: {
          name: "chain-player",
          loop_mode: "once" as const,
        },
      },
    };

    const chainAnimId = chainedOrch.registerAnimation(chainAnimationConfig);
    assert.ok(chainAnimId);

    const assertRateLimitedChange = (
      nextValue: number,
      prevValue: number,
      dtSeconds: number,
      maxRate: number,
    ) => {
      const allowed = maxRate * dtSeconds + 1e-6;
      const delta = Math.abs(nextValue - prevValue);
      assert.ok(
        delta <= allowed,
        `slew change ${delta} exceeded limit ${allowed}`,
      );
    };

    let chainFrame = chainedOrch.step(0.0) as any;
    let chainWrites = chainFrame.merged_writes;
    assert.ok(Array.isArray(chainWrites));
    let signValue = readScalar(chainWrites, "chain/sign.value");
    let slewValue = readScalar(chainWrites, "chain/slewed.value");
    assert.ok(Math.abs(signValue - -1) < 1e-6, `expected sign -1, got ${signValue}`);
    assert.ok(Math.abs(slewValue - -1) < 1e-6, `expected slew -1, got ${slewValue}`);

    const maxRate = 1;
    let prevSlew = slewValue;

    const stepAndCheck = (dtSeconds: number, expectedSign: number, expectedSlew: number) => {
      chainFrame = chainedOrch.step(dtSeconds) as any;
      chainWrites = chainFrame.merged_writes;
      signValue = readScalar(chainWrites, "chain/sign.value");
      slewValue = readScalar(chainWrites, "chain/slewed.value");
      assert.ok(
        Math.abs(signValue - expectedSign) < 1e-3,
        `expected sign ${expectedSign}, got ${signValue}`,
      );
      assertRateLimitedChange(slewValue, prevSlew, dtSeconds, maxRate);
      assert.ok(
        Math.abs(slewValue - expectedSlew) < 1e-3,
        `expected slew ${expectedSlew}, got ${slewValue}`,
      );
      prevSlew = slewValue;
    };

    stepAndCheck(1.0, 0, 0);
    stepAndCheck(1.0, 1, 1);
    stepAndCheck(1.0, 1, 1);

    // Graph-driven animation: use a sin(time) graph to seek the animation player.
    const controllerOrch = await createOrchestrator({ schedule: "TwoPass" });
    const driverFrequency = 0.5; // Hz
    const animationDurationSeconds = 2;
    const tau = Math.PI * 2;

    const driverAnimationConfig = {
      setup: {
        animation: {
          id: "sin-driven",
          name: "Sin Driven",
          duration: 2000,
          groups: [],
          tracks: [
            {
              id: "sin-track",
              name: "Sinusoid Track",
              animatableId: "control/anim.value",
              points: [
                { id: "start", stamp: 0, value: 0 },
                { id: "end", stamp: 1, value: 1 },
              ],
            },
          ],
        },
        player: {
          name: "controller-player",
          loop_mode: "loop" as const,
        },
      },
    };

    const controllerAnimId = controllerOrch.registerAnimation(driverAnimationConfig);
    assert.ok(controllerAnimId);

    const driverGraphSpec = {
      spec: {
        nodes: [
          {
            id: "host_time",
            type: "input",
            params: {
              path: "driver/time.seconds",
              value: toValueJSON(0),
            },
          },
          { id: "freq", type: "constant", params: { value: toValueJSON(driverFrequency) } },
          {
            id: "time_scaled",
            type: "multiply",
            inputs: {
              a: { node_id: "host_time" },
              b: { node_id: "freq" },
            },
          },
          { id: "tau", type: "constant", params: { value: toValueJSON(tau) } },
          {
            id: "phase",
            type: "multiply",
            inputs: {
              a: { node_id: "time_scaled" },
              b: { node_id: "tau" },
            },
          },
          {
            id: "sin_val",
            type: "sin",
            inputs: { in: { node_id: "phase" } },
          },
          { id: "one", type: "constant", params: { value: toValueJSON(1) } },
          {
            id: "shifted",
            type: "add",
            inputs: {
              lhs: { node_id: "sin_val" },
              rhs: { node_id: "one" },
            },
          },
          { id: "half", type: "constant", params: { value: toValueJSON(0.5) } },
          {
            id: "normalized",
            type: "multiply",
            inputs: {
              a: { node_id: "shifted" },
              b: { node_id: "half" },
            },
          },
          {
            id: "duration",
            type: "constant",
            params: { value: toValueJSON(animationDurationSeconds) },
          },
          {
            id: "seek_seconds",
            type: "multiply",
            inputs: {
              a: { node_id: "normalized" },
              b: { node_id: "duration" },
            },
          },
          {
            id: "seek_out",
            type: "output",
            params: { path: "anim/player/0/cmd/seek" },
            inputs: { in: { node_id: "seek_seconds" } },
          },
        ],
      },
      subs: {
        inputs: ["driver/time.seconds"],
        outputs: ["anim/player/0/cmd/seek"],
      },
    };

    const driverGraphId = controllerOrch.registerGraph(driverGraphSpec);
    assert.ok(driverGraphId);

    const normalizedForTime = (time: number) =>
      (Math.sin(tau * driverFrequency * time) + 1) / 2;
    const expectedSeekForTime = (time: number) =>
      normalizedForTime(time) * animationDurationSeconds;

    const setDriverTime = (timeSeconds: number) => {
      controllerOrch.setInput("driver/time.seconds", toValueJSON(timeSeconds));
    };

    const verifyDriverFrame = (frame: any, expectedTime: number) => {
      const writes = frame.merged_writes;
      const seekValue = readScalar(writes, "anim/player/0/cmd/seek");
      const animValue = readScalar(writes, "control/anim.value");
      const expectedSeek = expectedSeekForTime(expectedTime);
      const wrappedSeek = ((expectedSeek % animationDurationSeconds) + animationDurationSeconds) % animationDurationSeconds;
      const wrappedNormalized = animationDurationSeconds > 0 ? wrappedSeek / animationDurationSeconds : 0;
      assert.ok(
        Math.abs(seekValue - expectedSeek) < 1e-3,
        `expected seek ${expectedSeek}, got ${seekValue} at t=${expectedTime}`,
      );
      assert.ok(
        Math.abs(animValue - wrappedNormalized) < 0.25,
        `animation value ${animValue} should roughly follow normalized ${wrappedNormalized}`,
      );
    };

    let simulatedTime = 0;
    setDriverTime(simulatedTime);
    verifyDriverFrame(controllerOrch.step(0.0) as any, simulatedTime);

    const driverSteps = [0.25, 0.25, 0.25, 0.25];
    for (const dt of driverSteps) {
      simulatedTime += dt;
      setDriverTime(simulatedTime);
      const frame = controllerOrch.step(dt) as any;
      verifyDriverFrame(frame, simulatedTime);
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
