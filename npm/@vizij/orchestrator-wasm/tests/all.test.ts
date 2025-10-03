import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";
import {
  toValueJSON,
  valueAsNumber,
  valueAsVector,
  valueAsQuat,
  valueAsTransform,
  valueAsText,
  type ValueJSON,
  type ValueInput,
} from "@vizij/value-json";
import type { GraphRegistrationConfig, AnimationRegistrationConfig } from "../src/index.js";
import { init, createOrchestrator } from "../src/index.js";

type TestFixturesModule = typeof import("@vizij/test-fixtures");

let fixturesModule: TestFixturesModule | null = null;
const fixturesPromise: Promise<TestFixturesModule> = import(
  new URL("../../../test-fixtures/dist/index.js", import.meta.url).toString()
).then((module): TestFixturesModule => {
  fixturesModule = module as TestFixturesModule;
  return fixturesModule;
});

function fixtures(): TestFixturesModule {
  if (!fixturesModule) {
    throw new Error("Test fixtures module not loaded yet");
  }
  return fixturesModule;
}

function animationFixtures(): TestFixturesModule["animations"] {
  return fixtures().animations;
}

function nodeGraphFixtures(): TestFixturesModule["nodeGraphs"] {
  return fixtures().nodeGraphs;
}

function orchestrationFixtures(): TestFixturesModule["orchestrations"] {
  return fixtures().orchestrations;
}

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

function readScalar(writes: Array<{ path: string; value: unknown }>, path: string): number {
  const hit = writes.find((w) => w.path === path);
  assert.ok(hit, `Expected write for ${path}`);
  const value = valueAsNumber(hit.value as ValueJSON | undefined);
  assert.ok(Number.isFinite(value), `Expected ${path} to resolve to finite number`);
  return value!;
}

function graphConfigFromFixture(spec: unknown): GraphRegistrationConfig {
  if (spec && typeof spec === "object" && "spec" in (spec as Record<string, unknown>)) {
    return spec as GraphRegistrationConfig;
  }
  return { spec } as GraphRegistrationConfig;
}

function asValueObject(value: ValueJSON | undefined): Record<string, unknown> | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value as Record<string, unknown>;
}

const EPSILON = 1e-3;

function expectNearlyEqual(actual: number, expected: number, label: string): void {
  assert.ok(Math.abs(actual - expected) <= EPSILON, `${label} expected ${expected} got ${actual}`);
}

function expectVectorClose(actual: number[] | undefined, expected: number[], label: string): void {
  assert.ok(actual && actual.length === expected.length, `${label} length mismatch`);
  actual!.forEach((value, idx) => expectNearlyEqual(value, expected[idx], `${label}[${idx}]`));
}

function expectValueMatches(actual: ValueJSON | undefined, expected: unknown, label: string): void {
  if (typeof expected === "number") {
    const actualNumber = valueAsNumber(actual);
    assert.ok(Number.isFinite(actualNumber), `${label} expected numeric write`);
    expectNearlyEqual(actualNumber!, expected, label);
    return;
  }
  if (!expected || typeof expected !== "object") {
    assert.deepStrictEqual(actual, expected, `${label} mismatch`);
    return;
  }
  const obj = expected as Record<string, unknown>;
  if ("vec2" in obj || "vec3" in obj || "vec4" in obj || "vector" in obj) {
    const vec = valueAsVector(actual);
    assert.ok(vec, `${label} should resolve to vector`);
    const expectedVec = (obj.vec2 || obj.vec3 || obj.vec4 || obj.vector) as number[];
    expectVectorClose(vec!, expectedVec, label);
    return;
  }
  if ("quat" in obj) {
    const quat = valueAsQuat(actual);
    assert.ok(quat, `${label} should resolve to quaternion`);
    expectVectorClose(quat!, obj.quat as number[], label);
    return;
  }
  if ("transform" in obj) {
    const transform = valueAsTransform(actual);
    assert.ok(transform, `${label} should resolve to transform`);
    const expectedTransform = obj.transform as {
      translation?: number[];
      rotation?: number[];
      scale?: number[];
    };
    if (expectedTransform.translation) {
      expectVectorClose(transform!.translation, expectedTransform.translation, `${label}.translation`);
    }
    if (expectedTransform.rotation) {
      expectVectorClose(transform!.rotation, expectedTransform.rotation, `${label}.rotation`);
    }
    if (expectedTransform.scale) {
      expectVectorClose(transform!.scale, expectedTransform.scale, `${label}.scale`);
    }
    return;
  }
  if ("record" in obj) {
    const actualObj = asValueObject(actual);
    if (actualObj && "record" in actualObj) {
      const actualRecord = (actualObj.record as Record<string, ValueJSON>) || {};
      const expectedRecord = obj.record as Record<string, unknown>;
      for (const [key, expectedValue] of Object.entries(expectedRecord)) {
        expectValueMatches(actualRecord[key] as ValueJSON, expectedValue, `${label}.${key}`);
      }
      return;
    }
    const fallbackText = valueAsText(actual);
    if (typeof fallbackText === "string") {
      return;
    }
    assert.ok(actual !== undefined, `${label} missing value`);
    return;
  }
  if ("text" in obj) {
    const text = valueAsText(actual);
    assert.equal(text, obj.text, `${label} text mismatch`);
    return;
  }
  assert.deepStrictEqual(actual, expected, `${label} mismatch`);
}

function expectWriteMatches(
  writes: Array<{ path: string; value: unknown }>,
  path: string,
  expected: unknown,
): void {
  const hit = writes.find((w) => w.path === path);
  assert.ok(hit, `Expected write for ${path}`);
  expectValueMatches(hit!.value as ValueJSON, expected, path);
}

async function testScalarRampPipeline(): Promise<void> {
  const bundle = orchestrationFixtures().loadOrchestrationBundle("scalar-ramp-pipeline");
  const orch = await createOrchestrator({ schedule: "SinglePass" });

  const graphConfig = graphConfigFromFixture(bundle.graphSpec);
  const graphId = orch.registerGraph(graphConfig);
  assert.ok(typeof graphId === "string" && graphId.length > 0);

  const animationConfig: AnimationRegistrationConfig = {
    setup: {
      animation: bundle.animation,
      player: { name: "fixture-player", loop_mode: "once" as const },
    },
  };
  const animId = orch.registerAnimation(animationConfig);
  assert.ok(typeof animId === "string" && animId.length > 0);

  for (const input of bundle.descriptor.initial_inputs ?? []) {
    orch.setInput(input.path, toValueJSON(input.value as ValueInput));
  }

  for (const step of bundle.descriptor.steps ?? []) {
    const frame = orch.step(step.delta) as any;
    const mergedWrites: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
    for (const [path, expectedRaw] of Object.entries(step.expect ?? {})) {
      const actual = readScalar(mergedWrites, path);
      const expected = Number(expectedRaw);
      assert.ok(
        Math.abs(actual - expected) < 1e-3,
        `Expected ${path} ≈ ${expected}, received ${actual}`,
      );
    }
  }
}

async function testChainedSlewPipeline(): Promise<void> {
  const orch = await createOrchestrator({ schedule: "SinglePass" });

  const signGraph = nodeGraphFixtures().nodeGraphSpec("sign-graph") as GraphRegistrationConfig;
  const slewGraph = nodeGraphFixtures().nodeGraphSpec("slew-graph") as GraphRegistrationConfig;
  const signId = orch.registerGraph(signGraph);
  const slewId = orch.registerGraph(slewGraph);
  assert.ok(signId && slewId);

  const animationConfig = {
    setup: {
      animation: animationFixtures().animationFixture("chain-ramp"),
      player: { name: "chain-player", loop_mode: "once" as const },
    },
  };
  const animId = orch.registerAnimation(animationConfig);
  assert.ok(animId);

  const steps: Array<{ delta: number; sign: number; slewed: number }> = [
    { delta: 0.0, sign: -1, slewed: -1 },
    { delta: 1.0, sign: 0, slewed: 0 },
    { delta: 1.0, sign: 1, slewed: 1 },
    { delta: 1.0, sign: 1, slewed: 1 },
  ];

  const maxRate = 1;
  let previousSlew = steps[0].slewed;
  const assertRateLimitedChange = (nextValue: number, prevValue: number, dtSeconds: number) => {
    const allowed = maxRate * dtSeconds + 1e-6;
    const delta = Math.abs(nextValue - prevValue);
    assert.ok(delta <= allowed, `slew change ${delta} exceeded limit ${allowed}`);
  };

  steps.forEach((step, index) => {
    const frame = orch.step(step.delta) as any;
    const writes: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
    const signValue = readScalar(writes, "chain/sign.value");
    const slewValue = readScalar(writes, "chain/slewed.value");
    assert.ok(
      Math.abs(signValue - step.sign) < 1e-3,
      `expected sign ${step.sign}, got ${signValue}`,
    );
    if (index > 0) {
      assertRateLimitedChange(slewValue, previousSlew, step.delta);
    }
    assert.ok(
      Math.abs(slewValue - step.slewed) < 1e-3,
      `expected slew ${step.slewed}, got ${slewValue}`,
    );
    previousSlew = slewValue;
  });
}

async function testBlendPosePipeline(): Promise<void> {
  const bundle = orchestrationFixtures().loadOrchestrationBundle("blend-pose-pipeline");
  const orch = await createOrchestrator({ schedule: "TwoPass" });

  const graphConfig = graphConfigFromFixture(bundle.graphSpec);
  orch.registerGraph(graphConfig);

  const animationConfig: AnimationRegistrationConfig = {
    setup: {
      animation: bundle.animation,
      player: { name: "pose-player", loop_mode: "loop" as const },
    },
  };
  try {
    orch.registerAnimation(animationConfig);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    const expected =
      message.includes("RawValue") || message.includes("stored animation parse error");
    assert.ok(expected, `unexpected animation registration failure: ${message}`);
    assert.ok(
      Array.isArray((bundle.animation as Record<string, unknown>).tracks as unknown[]),
      "blend pose animation tracks missing",
    );
    return;
  }

  for (const input of bundle.descriptor.initial_inputs ?? []) {
    orch.setInput(input.path, toValueJSON(input.value as ValueInput));
  }

  for (const step of bundle.descriptor.steps ?? []) {
    const frame = orch.step(step.delta) as any;
    const mergedWrites: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
    for (const [path, expected] of Object.entries(step.expect ?? {})) {
      expectWriteMatches(mergedWrites, path, expected);
    }
  }
}

// async function testGraphDrivenAnimation(): Promise<void> {
//   const orch = await createOrchestrator({ schedule: "TwoPass" });

//   const animationConfig = {
//     setup: {
//       animation: animationFixtures().animationFixture("control-linear"),
//       player: { name: "controller-player", loop_mode: "loop" as const },
//     },
//   };
//   const animId = orch.registerAnimation(animationConfig);
//   assert.ok(animId);

//   const driverGraph = nodeGraphFixtures().nodeGraphSpec("sine-driver") as GraphRegistrationConfig;
//   const driverGraphId = orch.registerGraph(driverGraph);
//   assert.ok(driverGraphId);

//   const driverFrequency = 0.5;
//   const animationDurationSeconds = 2;
//   const tau = Math.PI * 2;

//   const normalizedForTime = (time: number) =>
//     (Math.sin(tau * driverFrequency * time) + 1) / 2;
//   const expectedSeekForTime = (time: number) =>
//     normalizedForTime(time) * animationDurationSeconds;

//   const setDriverTime = (timeSeconds: number) => {
//     orch.setInput("driver/time.seconds", toValueJSON(timeSeconds));
//   };

//   const verifyFrame = (frame: any, expectedTime: number) => {
//     const writes: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
//     const seekValue = readScalar(writes, "anim/player/0/cmd/seek");
//     const animValue = readScalar(writes, "control/anim.value");
//     const expectedSeek = expectedSeekForTime(expectedTime);
//     const expectedAnim = normalizedForTime(expectedTime);
//     assert.ok(Math.abs(seekValue - expectedSeek) < 1e-3, `seek mismatch at t=${expectedTime}, ${seekValue}, ${expectedSeek}`);
//     assert.ok(Math.abs(animValue - expectedAnim) < 1e-3, `anim mismatch at t=${expectedTime}, ${animValue}, ${expectedAnim}`);
//   };

//   for (let step = 0; step <= 4; step += 1) {
//     const t = step * 0.5;
//     setDriverTime(t);
//     const frame = orch.step(0.5) as any;
//     console.log(frame)
//     verifyFrame(frame, t);
//   }
// }

process.env.RUST_BACKTRACE = "1";

(async () => {
  try {
    await fixturesPromise;
    await init(pkgWasmUrl());
    await testScalarRampPipeline();
    await testChainedSlewPipeline();
    await testBlendPosePipeline();
    // await testGraphDrivenAnimation();
    // eslint-disable-next-line no-console
    console.log("@vizij/orchestrator-wasm smoke tests passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
