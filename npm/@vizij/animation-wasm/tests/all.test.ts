import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import { init, Engine } from "../src/index.js";
import type { StoredAnimation } from "../src/types";

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

function readScalarValue(value: unknown): number {
  if (typeof value === "number") {
    return value;
  }
  if (value && typeof value === "object") {
    const maybeRecord = value as Record<string, unknown>;
    if (typeof maybeRecord.data === "number") {
      return maybeRecord.data;
    }
    if (typeof maybeRecord.float === "number") {
      return maybeRecord.float as number;
    }
  }
  throw new Error(`Expected scalar animation value, received ${JSON.stringify(value)}`);
}

async function testLoadAnimationFromTypescriptObject(): Promise<void> {
  const engine = new Engine();

  const storedAnimation: StoredAnimation = {
    id: "ts-ramp",
    name: "TypeScript Ramp",
    duration: 1000,
    tracks: [
      {
        id: "track-0",
        name: "Scalar Ramp",
        animatableId: "demo/node.value",
        points: [
          {
            id: "start",
            stamp: 0,
            value: 0,
            transitions: { out: { x: 0, y: 0 } },
          },
          {
            id: "end",
            stamp: 1,
            value: 1,
            transitions: { in: { x: 1, y: 1 } },
          },
        ],
      },
    ],
    groups: {},
  };

  const animId = engine.loadAnimation(storedAnimation);
  assert.equal(typeof animId, "number", "loadAnimation should return numeric id");

  const playerId = engine.createPlayer("ts-player");
  const instId = engine.addInstance(playerId, animId);
  assert.equal(typeof instId, "number", "addInstance should return numeric id");

  const frame0 = engine.updateValues(0.0);
  const tsChange0 = frame0.changes.find((c) => c.player === playerId);
  assert.ok(tsChange0, "expected change for TypeScript animation at t=0");
  console.log("We got a test here", tsChange0)
  assert.ok(Math.abs(readScalarValue(tsChange0.value) - 0.0) < 1e-6);
  assert.equal(tsChange0.key, "demo/node.value");

  const frame1 = engine.updateValues(0.5);
  const tsChange1 = frame1.changes.find((c) => c.player === playerId);
  assert.ok(tsChange1, "expected change for TypeScript animation at t=0.5");
  console.log("We got a test here", tsChange1)
  assert.ok(Math.abs(readScalarValue(tsChange1.value) - 0.5) < 1e-3);
}

async function testLoadAnimationFromJsonFixture(): Promise<void> {
  const jsonPath = resolve(here, "../../../../../crates/animation/test_fixtures/ramp.json");
  if (!existsSync(jsonPath)) {
    throw new Error(`Missing animation fixture at ${jsonPath}`);
  }

  const storedAnimation = JSON.parse(readFileSync(jsonPath, "utf8")) as StoredAnimation;
  const engine = new Engine();

  const animId = engine.loadAnimation(storedAnimation);
  const playerId = engine.createPlayer("fixture-player");
  engine.addInstance(playerId, animId);

  engine.updateValues(0.0);
  const frame = engine.updateValues(1-1e-3); // Updating to 1 will loop us back to 0
  const change = frame.changes.find((c) => c.player === playerId && c.key === "node.t");
  assert.ok(change, "expected ramp change from fixture animation");
  console.log("We got a test here", change)
  assert.ok(Math.abs(readScalarValue(change.value) - 1.0) < 1e-3, String(change.value.data));
}

process.env.RUST_BACKTRACE = "1";

(async () => {
  try {
    await init(pkgWasmUrl());
    await testLoadAnimationFromTypescriptObject();
    await testLoadAnimationFromJsonFixture();
    // eslint-disable-next-line no-console
    console.log("@vizij/animation-wasm smoke tests passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
