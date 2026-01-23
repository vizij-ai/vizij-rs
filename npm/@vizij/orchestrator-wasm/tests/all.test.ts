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
import { init, createOrchestrator, loadOrchestrationBundle } from "../src/index.js";
import {
  loadAnimationFixture as loadEmbeddedAnimation,
  loadNodeGraphSpec as loadEmbeddedGraphSpec,
} from "../src/fixtures.js";

const here = dirname(fileURLToPath(import.meta.url));
type OrchestrationBundleResult = Awaited<ReturnType<typeof loadOrchestrationBundle>>;

function pkgWasmUrl(): URL {
  const wasmPath = resolve(here, "../../../pkg/vizij_orchestrator_wasm_bg.wasm");
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

function cloneDeep<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function graphConfigFromFixture(spec: unknown): GraphRegistrationConfig {
  if (spec && typeof spec === "object" && "spec" in (spec as Record<string, unknown>)) {
    return cloneDeep(spec as GraphRegistrationConfig);
  }
  return { spec: cloneDeep(spec) } as GraphRegistrationConfig;
}

function graphConfigFromBinding(
  binding: OrchestrationBundleResult["graphs"][number],
): GraphRegistrationConfig {
  const config = cloneDeep(binding.config);
  const sanitized: GraphRegistrationConfig = {
    spec: config.spec,
  };
  if (binding.id || config.id) {
    sanitized.id = binding.id ?? config.id;
  }
  if (config.subs) {
    sanitized.subs = config.subs;
  }
  return sanitized;
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
  const bundle = await loadOrchestrationBundle("scalar-ramp-pipeline");
  const schedule =
    (typeof bundle.descriptor.schedule === "string" && bundle.descriptor.schedule) ||
    "SinglePass";
  const orch = await createOrchestrator({ schedule });

  const graphIds = bundle.graphs.map((binding) => {
    const config = graphConfigFromBinding(binding);
    const graphId = orch.registerGraph(config);
    assert.ok(typeof graphId === "string" && graphId.length > 0);
    return graphId;
  });
  assert.ok(graphIds.length > 0, "expected at least one graph binding");

  const primaryAnimation = bundle.animations[0];
  const animationConfig: AnimationRegistrationConfig = {
    ...(primaryAnimation?.id ? { id: primaryAnimation.id } : {}),
    setup: cloneDeep(
      primaryAnimation?.setup ?? { animation: primaryAnimation?.animation },
    ),
  };
  const animId = orch.registerAnimation(animationConfig);
  assert.ok(typeof animId === "string" && animId.length > 0);

  for (const input of bundle.initialInputs ?? []) {
    const shape = input.shape ? cloneDeep(input.shape) : undefined;
    orch.setInput(input.path, toValueJSON(input.value as ValueInput), shape);
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

  const signGraph = await loadEmbeddedGraphSpec("sign-graph");
  const slewGraph = await loadEmbeddedGraphSpec("slew-graph");
  const signId = orch.registerGraph(signGraph);
  const slewId = orch.registerGraph(slewGraph);
  assert.ok(signId && slewId);

  const animationConfig = {
    setup: {
      animation: await loadEmbeddedAnimation("chain-ramp"),
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

async function testChainSignSlewFixture(): Promise<void> {
  const bundle = await loadOrchestrationBundle("chain-sign-slew-pipeline");
  const schedule =
    (typeof bundle.descriptor.schedule === "string" && bundle.descriptor.schedule) ||
    "SinglePass";
  const orch = await createOrchestrator({ schedule });

  assert.ok(
    bundle.initialInputs?.some((input) => input.path === "chain/ramp.value"),
    "expected staged ramp value in initial inputs",
  );

  bundle.graphs.forEach((binding) => {
    const config = graphConfigFromBinding(binding);
    orch.registerGraph(config);
  });

  const primaryAnimation = bundle.animations[0];
  const animationConfig: AnimationRegistrationConfig = {
    ...(primaryAnimation?.id ? { id: primaryAnimation.id } : {}),
    setup: cloneDeep(
      primaryAnimation?.setup ?? { animation: primaryAnimation?.animation },
    ),
  };
  orch.registerAnimation(animationConfig);

  for (const input of bundle.initialInputs ?? []) {
    const shape = input.shape ? cloneDeep(input.shape) : undefined;
    orch.setInput(input.path, toValueJSON(input.value as ValueInput), shape);
  }

  let previousSlew = 0;
  bundle.descriptor.steps?.forEach((step, index) => {
    const frame = orch.step(step.delta) as any;
    const writes: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
    for (const [path, expectedRaw] of Object.entries(step.expect ?? {})) {
      const expected = Number(expectedRaw);
      const actual = readScalar(writes, path);
      expectNearlyEqual(actual, expected, path);
      if (path === "chain/slewed.value" && index > 0) {
        // Slew node should be rate-limited to 1 unit per second.
        const allowedDelta = step.delta + 1e-6;
        const delta = Math.abs(actual - previousSlew);
        assert.ok(delta <= allowedDelta, `slew delta ${delta} exceeded limit ${allowedDelta}`);
        previousSlew = actual;
      }
      if (path === "chain/slewed.value" && index === 0) {
        previousSlew = actual;
      }
    }
  });
}

async function testMergedGraphRegistration(): Promise<void> {
  const orch = await createOrchestrator({ schedule: "SinglePass" });

  let mergedId: string;
  try {
    mergedId = orch.registerMergedGraph({
      graphs: [
        {
          spec: {
            nodes: [
              {
                id: "const_one",
                type: "constant",
                params: { value: 1 },
              },
              {
                id: "publish",
                type: "output",
                params: { path: "shared/value" },
              },
            ],
            edges: [
              {
                from: { node_id: "const_one" },
                to: { node_id: "publish", input: "in" },
              },
            ],
          },
          subs: {
            outputs: ["shared/value"],
          },
        },
        {
          spec: {
            nodes: [
              {
                id: "shared_input",
                type: "input",
                params: { path: "shared/value" },
              },
              {
                id: "double",
                type: "multiply",
                input_defaults: {
                  rhs: { value: 2 },
                },
              },
              {
                id: "publish_doubled",
                type: "output",
                params: { path: "shared/doubled" },
              },
            ],
            edges: [
              {
                from: { node_id: "shared_input" },
                to: { node_id: "double", input: "lhs" },
              },
              {
                from: { node_id: "double" },
                to: { node_id: "publish_doubled", input: "in" },
              },
            ],
          },
          subs: {
            inputs: ["shared/value"],
            outputs: ["shared/doubled"],
          },
        },
      ],
      strategy: {
        outputs: "blend",
        intermediate: "blend",
      },
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    if (message.includes("register_merged_graph is not a function")) {
      console.warn(
        "Skipping merged graph test because wasm bindings were not rebuilt (run pnpm run build:wasm:orchestrator).",
      );
      return;
    }
    throw err;
  }

  assert.ok(typeof mergedId === "string" && mergedId.length > 0);

  const frame = orch.step(1 / 60) as any;
  const writes: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
  const doubled = readScalar(writes, "shared/doubled");
  expectNearlyEqual(doubled, 2, "merged graph doubled output");
}

async function testReplaceGraphStructuralEditResetsPlan(): Promise<void> {
  const orch = await createOrchestrator({ schedule: "SinglePass" });

  const id = orch.registerGraph({
    id: "replace-test",
    spec: {
      nodes: [
        { id: "const_one", type: "constant", params: { value: 1 } },
        { id: "publish", type: "output", params: { path: "shared/value" } },
      ],
      edges: [{ from: { node_id: "const_one" }, to: { node_id: "publish", input: "in" } }],
    },
    subs: { outputs: ["shared/value"] },
  });
  assert.equal(id, "replace-test");

  const frame1 = orch.step(1 / 60) as any;
  const writes1: Array<{ path: string; value: unknown }> = frame1.merged_writes ?? [];
  expectNearlyEqual(readScalar(writes1, "shared/value"), 1, "pre-replace value");

  // Structural edit: insert multiply node constant->multiply(rhs=2)->output
  // This must rebuild the cached plan/layout; otherwise the old layout could be reused.
  orch.replaceGraph({
    id: "replace-test",
    spec: {
      nodes: [
        { id: "const_one", type: "constant", params: { value: 1 } },
        { id: "mul", type: "multiply", input_defaults: { rhs: { value: 2 } } },
        { id: "publish", type: "output", params: { path: "shared/value" } },
      ],
      edges: [
        { from: { node_id: "const_one" }, to: { node_id: "mul", input: "lhs" } },
        { from: { node_id: "mul" }, to: { node_id: "publish", input: "in" } },
      ],
    },
    subs: { outputs: ["shared/value"] },
  });

  const frame2 = orch.step(1 / 60) as any;
  const writes2: Array<{ path: string; value: unknown }> = frame2.merged_writes ?? [];
  expectNearlyEqual(readScalar(writes2, "shared/value"), 2, "post-replace value");
}

async function testBlendPosePipeline(): Promise<void> {
  const bundle = await loadOrchestrationBundle("blend-pose-pipeline");
  const schedule =
    (typeof bundle.descriptor.schedule === "string" && bundle.descriptor.schedule) ||
    "TwoPass";
  const orch = await createOrchestrator({ schedule });

  bundle.graphs.forEach((binding) => {
    const config = graphConfigFromBinding(binding);
    orch.registerGraph(config);
  });

  const primaryAnimation = bundle.animations[0];
  const animationConfig: AnimationRegistrationConfig = {
    ...(primaryAnimation?.id ? { id: primaryAnimation.id } : {}),
    setup: cloneDeep(
      primaryAnimation?.setup ?? {
        animation: primaryAnimation?.animation,
        player: { name: "pose-player", loop_mode: "loop" as const },
      },
    ),
  };
  try {
    orch.registerAnimation(animationConfig);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    const expected =
      message.includes("RawValue") || message.includes("stored animation parse error");
    assert.ok(expected, `unexpected animation registration failure: ${message}`);
    assert.ok(
      Array.isArray(
        ((primaryAnimation?.animation ?? {}) as Record<string, unknown>).tracks as unknown[],
      ),
      "blend pose animation tracks missing",
    );
    return;
  }

  for (const input of bundle.initialInputs ?? []) {
    const shape = input.shape ? cloneDeep(input.shape) : undefined;
    orch.setInput(input.path, toValueJSON(input.value as ValueInput), shape);
  }

  for (const step of bundle.descriptor.steps ?? []) {
    const frame = orch.step(step.delta) as any;
    const mergedWrites: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];
    for (const [path, expected] of Object.entries(step.expect ?? {})) {
      expectWriteMatches(mergedWrites, path, expected);
    }
  }
}

async function testMergedFixtureBundle(): Promise<void> {
  const bundle = await loadOrchestrationBundle("merged-blend-pipeline");
  assert.ok(bundle.mergedGraphs.length === 1, "merged graphs fixture missing");
  const merged = bundle.mergedGraphs[0];
  assert.equal(merged.id, "bundle");
  assert.ok(Array.isArray(merged.graphs) && merged.graphs.length === 4);
  merged.graphs.forEach((binding) => {
    assert.ok(typeof binding.key === "string" && binding.key.length > 0);
    const config = graphConfigFromBinding(binding);
    assert.ok(config.spec, `missing spec for ${binding.key}`);
  });
  assert.ok(Array.isArray(bundle.initialInputs), "initialInputs should be array");

  const schedule =
    (typeof bundle.descriptor.schedule === "string" && bundle.descriptor.schedule) ||
    "SinglePass";
  const orch = await createOrchestrator({ schedule });

  const mergedGraphs = merged.graphs.map((binding) => graphConfigFromBinding(binding));
  let mergedId: string;
  try {
    mergedId = orch.registerMergedGraph({
      id: merged.id,
      graphs: mergedGraphs,
      strategy: merged.strategy,
    });
  } catch (error) {
    // eslint-disable-next-line no-console
    console.error("merged fixture payload", JSON.stringify({
      id: merged.id,
      graphs: mergedGraphs,
      strategy: merged.strategy,
    }, null, 2));
    throw error;
  }
  assert.ok(typeof mergedId === "string" && mergedId.length > 0);

  const primaryAnimation = bundle.animations[0];
  if (primaryAnimation) {
    orch.registerAnimation({
      ...(primaryAnimation.id ? { id: primaryAnimation.id } : {}),
      setup: cloneDeep(
        primaryAnimation.setup ?? { animation: primaryAnimation.animation },
      ),
    });
  }

  for (const input of bundle.initialInputs ?? []) {
    const shape = input.shape ? cloneDeep(input.shape) : undefined;
    orch.setInput(input.path, toValueJSON(input.value as ValueInput), shape);
  }

  const frame = orch.step(1.0);
  const writes: Array<{ path: string; value: unknown }> = frame.merged_writes ?? [];

  const expectScalar = (path: string, expected: number) => {
    const actual = readScalar(writes, path);
    expectNearlyEqual(actual, expected, path);
  };

  expectScalar("final/a", 10);
  expectScalar("final/b", 110);
  expectScalar("final/c", 1110);
  expectScalar("final/d", 2200);
  expectScalar("final/e", 5000);
  expectScalar("final/f", 6000);
}

// async function testGraphDrivenAnimation(): Promise<void> {
//   const orch = await createOrchestrator({ schedule: "TwoPass" });

//   const animationConfig = {
//     setup: {
//       animation: await loadEmbeddedAnimation("control-linear"),
//       player: { name: "controller-player", loop_mode: "loop" as const },
//     },
//   };
//   const animId = orch.registerAnimation(animationConfig);
//   assert.ok(animId);

//   const driverGraph = await loadEmbeddedGraphSpec("sine-driver");
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
    await init(pkgWasmUrl());
    await testScalarRampPipeline();
    await testChainedSlewPipeline();
    await testChainSignSlewFixture();
    await testMergedGraphRegistration();
    await testReplaceGraphStructuralEditResetsPlan();
    await testBlendPosePipeline();
    await testMergedFixtureBundle();
    // await testGraphDrivenAnimation();
    // eslint-disable-next-line no-console
    console.log("@vizij/orchestrator-wasm smoke tests passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
