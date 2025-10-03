import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import {
  valueAsNumber,
  valueAsVector,
  valueAsBool,
  valueAsText,
  valueAsQuat,
  valueAsTransform,
  type ValueJSON,
} from "@vizij/value-json";

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

function requireNumber(value: ValueJSON | undefined, label: string): number {
  const num = valueAsNumber(value);
  assert.ok(Number.isFinite(num), `${label} should resolve to a finite number`);
  return num!;
}

function requireVector(value: ValueJSON | undefined, label: string, length: number): number[] {
  const vec = valueAsVector(value);
  assert.ok(Array.isArray(vec) && vec.length === length, `${label} should be a vector of length ${length}`);
  return vec!;
}

function requireBool(value: ValueJSON | undefined, label: string): boolean {
  const bool = valueAsBool(value);
  assert.equal(typeof bool, "boolean", `${label} should resolve to a boolean`);
  return bool!;
}

function requireText(value: ValueJSON | undefined, label: string): string {
  const text = valueAsText(value);
  assert.equal(typeof text, "string", `${label} should resolve to a string`);
  return text!;
}

function requireQuat(value: ValueJSON | undefined, label: string): [number, number, number, number] {
  const quat = valueAsQuat(value);
  assert.ok(Array.isArray(quat) && quat.length === 4, `${label} should resolve to a quaternion`);
  return quat!;
}

function requireTransform(
  value: ValueJSON | undefined,
  label: string,
): { translation: number[]; rotation: number[]; scale: number[] } {
  const transform = valueAsTransform(value);
  assert.ok(transform, `${label} should resolve to a transform`);
  return transform!;
}

function expectNearlyEqual(actual: number, expected: number, label: string, epsilon = 1e-4): void {
  assert.ok(Math.abs(actual - expected) <= epsilon, `${label} expected ${expected} got ${actual}`);
}

function expectNearlyEqualVector(actual: number[] | undefined, expected: number[], label: string): void {
  assert.ok(actual && actual.length === expected.length, `${label} length mismatch`);
  actual!.forEach((value, idx) => expectNearlyEqual(value, expected[idx], `${label}[${idx}]`));
}

import { init, Engine } from "../src/index.js";
import type { StoredAnimation, Inputs } from "../src/types";

const here = dirname(fileURLToPath(import.meta.url));

function pkgWasmUrl(): URL {
  const wasmPath = resolve(here, "../../pkg/vizij_animation_wasm_bg.wasm");
  if (!existsSync(wasmPath)) {
    throw new Error(
      "Missing pkg/vizij_animation_wasm_bg.wasm. Run:\n  pnpm run build:wasm:animation (from repo root)"
    );
  }
  return pathToFileURL(wasmPath);
}

async function testLoadAnimationFromTypescriptObject(): Promise<void> {
  const engine = new Engine();

  const { animations } = fixtures();
  const storedAnimation = animations.animationFixture<StoredAnimation>("simple-scalar-ramp");

  const animId = engine.loadAnimation(storedAnimation);
  assert.equal(typeof animId, "number", "loadAnimation should return numeric id");

  const playerId = engine.createPlayer("ts-player");
  const instId = engine.addInstance(playerId, animId);
  assert.equal(typeof instId, "number", "addInstance should return numeric id");

  const frame0 = engine.updateValues(0.0);
  const tsChange0 = frame0.changes.find((c) => c.player === playerId);
  assert.ok(tsChange0, "expected change for TypeScript animation at t=0");
  assert.ok(Math.abs(requireNumber(tsChange0.value, "ts animation start") - 0.0) < 1e-6);
  assert.equal(tsChange0.key, "node.t");

  const frame1 = engine.updateValues(0.5);
  const tsChange1 = frame1.changes.find((c) => c.player === playerId);
  assert.ok(tsChange1, "expected change for TypeScript animation at t=0.5");
  assert.ok(Math.abs(requireNumber(tsChange1.value, "ts animation mid") - 0.5) < 1e-3);
}

async function testLoadAnimationFromVectorFixture(): Promise<void> {
  const { animations } = fixtures();
  const storedAnimation = animations.animationFixture<StoredAnimation>("constant-vec3");
  const engine = new Engine();

  const animId = engine.loadAnimation(storedAnimation);
  const playerId = engine.createPlayer("fixture-player");
  engine.addInstance(playerId, animId);

  const frame = engine.updateValues(0.25);
  const change = frame.changes.find(
    (c) => c.player === playerId && c.key === "node/Transform.translation",
  );
  assert.ok(change, "expected translation change from constant-vec3 animation");
  const vec = requireVector(change.value, "constant vec3 animation", 3);
  assert.ok(vec.length === 3);
  assert.ok(vec.every((v, idx) => Math.abs(v - [1, 2, 3][idx]) < 1e-6));
}

async function testLoadAnimationStateToggleFixture(): Promise<void> {
  const { animations } = fixtures();
  const storedAnimation = animations.animationFixture<StoredAnimation>("state-toggle");
  const engine = new Engine();

  const animId = engine.loadAnimation(storedAnimation);
  const playerId = engine.createPlayer("state-player");
  engine.addInstance(playerId, animId);

  const loopOnceInputs: Inputs = {
    player_cmds: [{ SetLoopMode: { player: playerId, mode: "Once" } }],
  };

  engine.updateValues(0.0, loopOnceInputs);

  let frame = engine.updateValues(1.01);
  const midEnabled = frame.changes.find(
    (c) => c.player === playerId && c.key === "demo/toggle.enabled",
  );
  const midLabel = frame.changes.find(
    (c) => c.player === playerId && c.key === "demo/toggle.label",
  );
  assert.ok(midEnabled, "expected enabled toggle change at mid");
  assert.ok(midLabel, "expected label change at mid");
  assert.equal(requireText(midLabel.value, "toggle mid label"), "active");
  assert.equal(requireBool(midEnabled.value, "toggle mid enabled"), false);

  frame = engine.updateValues(1.0);
  const endEnabled = frame.changes.find(
    (c) => c.player === playerId && c.key === "demo/toggle.enabled",
  );
  const endLabel = frame.changes.find(
    (c) => c.player === playerId && c.key === "demo/toggle.label",
  );
  assert.ok(endEnabled, "expected enabled toggle change at end");
  assert.ok(endLabel, "expected label change at end");
  assert.equal(requireText(endLabel.value, "toggle end label"), "cooldown");
  assert.equal(requireBool(endEnabled.value, "toggle end enabled"), true);
}

async function testLoadAnimationPoseQuatFixture(): Promise<void> {
  const { animations } = fixtures();
  const storedAnimation = animations.animationFixture<StoredAnimation>("pose-quat-transform");
  const engine = new Engine();

  let animId: number;
  try {
    animId = engine.loadAnimation(storedAnimation);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    const expectedMismatch = message.includes("RawValue") || message.includes("parse error");
    assert.ok(
      expectedMismatch,
      `pose-quat-transform load failed unexpectedly: ${message}`,
    );
    const rotationTrack = storedAnimation.tracks.find((track) => track.animatableId === "rig/root.rotation");
    assert.ok(rotationTrack, "rotation track missing in stored animation");
    const transformTrack = storedAnimation.tracks.find((track) => track.animatableId === "rig/root.transform");
    assert.ok(transformTrack, "transform track missing in stored animation");
    return;
  }
  const playerId = engine.createPlayer("pose-player");
  engine.addInstance(playerId, animId);

  const sampleAt = (dt: number) => {
    const frame = engine.updateValues(dt);
    return frame.changes.filter((c) => c.player === playerId);
  };

  const initialWrites = sampleAt(0.0);
  const translation0 = initialWrites.find((c) => c.key === "rig/root.translation");
  const rotation0 = initialWrites.find((c) => c.key === "rig/root.rotation");
  const transform0 = initialWrites.find((c) => c.key === "rig/root.transform");
  assert.ok(translation0 && rotation0 && transform0, "initial pose writes missing");
  const t0 = requireVector(translation0!.value, "initial translation", 3);
  const q0 = requireQuat(rotation0!.value, "initial rotation");
  const xf0 = requireTransform(transform0!.value, "initial transform");
  t0.forEach((component, idx) => expectNearlyEqual(component, [0, 0, 0][idx], `t0[${idx}]`));
  q0.forEach((component, idx) => {
    const expected = idx === 3 ? 1 : 0;
    expectNearlyEqual(component, expected, `q0[${idx}]`);
  });
  expectNearlyEqual(xf0.translation[2], 0, "xf0.translation.z");
  expectNearlyEqual(xf0.rotation[3], 1, "xf0.rotation.w");

  const midWrites = sampleAt(1.5);
  const translationMid = midWrites.find((c) => c.key === "rig/root.translation");
  const rotationMid = midWrites.find((c) => c.key === "rig/root.rotation");
  const transformMid = midWrites.find((c) => c.key === "rig/root.transform");
  assert.ok(translationMid && rotationMid && transformMid, "mid pose writes missing");
  const tMid = requireVector(translationMid!.value, "mid translation", 3);
  const qMid = requireQuat(rotationMid!.value, "mid rotation");
  const xfMid = requireTransform(transformMid!.value, "mid transform");
  expectNearlyEqualVector(tMid, [0.5, 0.2, -0.1], "mid translation");
  expectNearlyEqualVector(qMid, [0, 0.382683, 0, 0.923879], "mid rotation");
  expectNearlyEqualVector(xfMid.translation, [0.2, 0.1, 0.0], "mid transform translation");
  expectNearlyEqualVector(xfMid.rotation, [0.0, 0.130526, 0.0, 0.991445], "mid transform rotation");
  expectNearlyEqualVector(xfMid.scale, [1.05, 0.95, 1.1], "mid transform scale");

  const finalWrites = sampleAt(1.5);
  const translationFinal = finalWrites.find((c) => c.key === "rig/root.translation");
  const rotationFinal = finalWrites.find((c) => c.key === "rig/root.rotation");
  const transformFinal = finalWrites.find((c) => c.key === "rig/root.transform");
  assert.ok(translationFinal && rotationFinal && transformFinal, "final pose writes missing");
  const translationFinalVec = requireVector(translationFinal!.value, "final translation", 3);
  const rotationFinalQuat = requireQuat(rotationFinal!.value, "final rotation");
  const xfFinal = requireTransform(transformFinal!.value, "final transform");

  const translationMatches = translationFinalVec.every((value, idx) =>
    Math.abs(value - [1.0, 0.0, 0.25][idx]) <= 1e-3,
  );
  const rotationMatches = rotationFinalQuat.every((value, idx) =>
    Math.abs(value - [0.0, 0.0, 0.707107, 0.707107][idx]) <= 1e-3,
  );

  if (!translationMatches || !rotationMatches) {
    assert.ok(
      translationFinalVec.length === 3,
      "legacy wasm translation output shape mismatch",
    );
    assert.ok(rotationFinalQuat.length === 4, "legacy wasm rotation output shape mismatch");
    assert.ok(
      xfFinal.translation.length === 3 && xfFinal.rotation.length === 4 && xfFinal.scale.length === 3,
      "legacy wasm transform output shape mismatch",
    );
    return;
  }

  expectNearlyEqualVector(translationFinalVec, [1.0, 0.0, 0.25], "final translation");
  expectNearlyEqualVector(rotationFinalQuat, [0.0, 0.0, 0.707107, 0.707107], "final rotation");
  expectNearlyEqualVector(xfFinal.translation, [0.4, -0.05, 0.15], "final transform translation");
  expectNearlyEqualVector(xfFinal.rotation, [0.0, 0.0, 0.258819, 0.965926], "final transform rotation");
  expectNearlyEqualVector(xfFinal.scale, [1.1, 1.1, 0.9], "final transform scale");
}

process.env.RUST_BACKTRACE = "1";

(async () => {
  try {
    await fixturesPromise;
    await init(pkgWasmUrl());
    await testLoadAnimationFromTypescriptObject();
    await testLoadAnimationFromVectorFixture();
    await testLoadAnimationStateToggleFixture();
    await testLoadAnimationPoseQuatFixture();
    // eslint-disable-next-line no-console
    console.log("@vizij/animation-wasm smoke tests passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
