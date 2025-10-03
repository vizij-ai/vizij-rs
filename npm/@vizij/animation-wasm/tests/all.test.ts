import assert from "node:assert/strict";
import { existsSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import {
  valueAsNumber,
  valueAsVector,
  valueAsBool,
  valueAsText,
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

process.env.RUST_BACKTRACE = "1";

(async () => {
  try {
    await fixturesPromise;
    await init(pkgWasmUrl());
    await testLoadAnimationFromTypescriptObject();
    await testLoadAnimationFromVectorFixture();
    await testLoadAnimationStateToggleFixture();
    // eslint-disable-next-line no-console
    console.log("@vizij/animation-wasm smoke tests passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
