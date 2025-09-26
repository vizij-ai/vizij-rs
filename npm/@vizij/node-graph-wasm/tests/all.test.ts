import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import { dirname, resolve } from "node:path";

import {
  init,
  Graph,
  oscillatorBasics,
  vectorPlayground,
  logicGate,
  tupleSpringDampSlew,
  layeredRigBlend,
  hierarchicalBlend,
  weightedAverage,
  nestedTelemetry,
  type EvalResult,
  type ValueJSON,
  type GraphSpec,
  type WriteOpJSON,
  type ShapeJSON,
} from "../src/index.js";

type EvalSpec = Parameters<Graph["loadGraph"]>[0];

const here = dirname(fileURLToPath(import.meta.url));

// Prefer local tests/fixtures directory if present, otherwise fallback to dist tests path used in some sources
const candidateFixtures = resolve(here, "fixtures");
const fixturesDir = existsSync(candidateFixtures) ? candidateFixtures : resolve(here, "../../tests/fixtures");

function pkgWasmUrl(): URL {
  const wasmPath = resolve(here, "../../pkg/vizij_graph_wasm_bg.wasm");
  if (!existsSync(wasmPath)) {
    throw new Error(
      "Missing pkg/vizij_graph_wasm_bg.wasm. Run:\n  wasm-pack build crates/node-graph/vizij-graph-wasm --target web --out-dir npm/@vizij/node-graph-wasm/pkg --release (from repo root vizij-rs/)"
    );
  }
  return pathToFileURL(wasmPath);
}

function loadJsonFixture(name: string): string {
  const path = resolve(fixturesDir, name);
  if (!existsSync(path)) {
    throw new Error(`Missing JSON fixture ${name} at ${path}`);
  }
  return readFileSync(path, "utf8");
}

/* ---------- Generic validation helpers (merged) ---------- */

function ensureFiniteNumber(value: number, context: string): void {
  if (!Number.isFinite(value)) {
    throw new Error(`${context} produced non-finite number ${value}`);
  }
}

function assertValueFinite(value: ValueJSON, context: string): void {
  if ("float" in value) {
    ensureFiniteNumber(value.float, context);
    return;
  }
  if ("bool" in value) return;
  if ("text" in value) return;
  if ("vec2" in value) {
    value.vec2.forEach((v, idx) => ensureFiniteNumber(v, `${context}.vec2[${idx}]`));
    return;
  }
  if ("vec3" in value) {
    value.vec3.forEach((v, idx) => ensureFiniteNumber(v, `${context}.vec3[${idx}]`));
    return;
  }
  if ("vec4" in value) {
    value.vec4.forEach((v, idx) => ensureFiniteNumber(v, `${context}.vec4[${idx}]`));
    return;
  }
  if ("quat" in value) {
    value.quat.forEach((v, idx) => ensureFiniteNumber(v, `${context}.quat[${idx}]`));
    return;
  }
  if ("color" in value) {
    value.color.forEach((v, idx) => ensureFiniteNumber(v, `${context}.color[${idx}]`));
    return;
  }
  if ("transform" in value) {
    value.transform.pos.forEach((v, idx) => ensureFiniteNumber(v, `${context}.transform.pos[${idx}]`));
    value.transform.rot.forEach((v, idx) => ensureFiniteNumber(v, `${context}.transform.rot[${idx}]`));
    value.transform.scale.forEach((v, idx) => ensureFiniteNumber(v, `${context}.transform.scale[${idx}]`));
    return;
  }
  if ("vector" in value) {
    value.vector.forEach((v, idx) => ensureFiniteNumber(v, `${context}.vector[${idx}]`));
    return;
  }
  if ("record" in value) {
    for (const [key, child] of Object.entries(value.record)) {
      assertValueFinite(child, `${context}.${key}`);
    }
    return;
  }
  if ("array" in value) {
    value.array.forEach((child, idx) => assertValueFinite(child, `${context}[${idx}]`));
    return;
  }
  if ("list" in value) {
    value.list.forEach((child, idx) => assertValueFinite(child, `${context}.list[${idx}]`));
    return;
  }
  if ("tuple" in value) {
    value.tuple.forEach((child, idx) => assertValueFinite(child, `${context}.tuple[${idx}]`));
    return;
  }
  if ("enum" in value) {
    assertValueFinite(value.enum.value, `${context}<${value.enum.tag}>`);
    return;
  }
}

const EPSILON = 1e-4;
function assertNearlyEqual(actual: number, expected: number, context: string): void {
  if (Math.abs(actual - expected) > EPSILON) {
    throw new Error(`${context} expected ${expected} but received ${actual}`);
  }
}

function expectNumericVector(value: ValueJSON | undefined, expected: number[], label: string): void {
  if (!value) {
    throw new Error(`${label} write missing`);
  }
  assertValueFinite(value, label);

  let actual: number[] | undefined;
  if ("vector" in value) {
    actual = value.vector;
  } else if ("vec2" in value) {
    actual = value.vec2;
  } else if ("vec3" in value) {
    actual = value.vec3;
  } else if ("vec4" in value) {
    actual = value.vec4;
  } else if ("quat" in value) {
    actual = value.quat;
  }

  if (!actual) {
    throw new Error(`${label} expected numeric vector, received ${JSON.stringify(value)}`);
  }
  if (actual.length !== expected.length) {
    throw new Error(`${label} expected length ${expected.length} but received ${actual.length}`);
  }
  actual.forEach((v, idx) => assertNearlyEqual(v, expected[idx], `${label}[${idx}]`));
}

function expectListOfText(value: ValueJSON | undefined, expected: string[], label: string): void {
  if (!value || !("list" in value)) {
    throw new Error(`${label} expected list of text values`);
  }
  const actual = value.list.map((entry, idx) => {
    if ("text" in entry) {
      return entry.text;
    }
    throw new Error(`${label} entry ${idx} expected text but received ${JSON.stringify(entry)}`);
  });
  if (actual.length !== expected.length) {
    throw new Error(`${label} expected ${expected.length} entries but received ${actual.length}`);
  }
  actual.forEach((text, idx) => {
    if (text !== expected[idx]) {
      throw new Error(`${label}[${idx}] expected '${expected[idx]}' but received '${text}'`);
    }
  });
}

function expectTupleOfFloats(value: ValueJSON | undefined, expected: number[], label: string): void {
  if (!value || !("tuple" in value)) {
    throw new Error(`${label} expected tuple value`);
  }
  const tuple = value.tuple;
  if (tuple.length !== expected.length) {
    throw new Error(`${label} expected ${expected.length} items but received ${tuple.length}`);
  }
  tuple.forEach((entry, idx) => {
    if (!("float" in entry)) {
      throw new Error(`${label}[${idx}] expected float but received ${JSON.stringify(entry)}`);
    }
    assertNearlyEqual(entry.float, expected[idx], `${label}[${idx}]`);
  });
}

function expectRecordTexts(value: ValueJSON | undefined, expected: Record<string, string>, label: string): void {
  if (!value || !("record" in value)) {
    throw new Error(`${label} expected record value`);
  }
  const actualEntries = Object.entries(value.record);
  const expectedEntries = Object.entries(expected);
  if (actualEntries.length !== expectedEntries.length) {
    throw new Error(`${label} expected ${expectedEntries.length} fields but received ${actualEntries.length}`);
  }
  for (const [key, expectedText] of expectedEntries) {
    const entry = value.record[key];
    if (!entry || !("text" in entry)) {
      throw new Error(`${label}.${key} expected text value`);
    }
    if (entry.text !== expectedText) {
      throw new Error(`${label}.${key} expected '${expectedText}' but received '${entry.text}'`);
    }
  }
}

/* ---------- Functions used by the 'strict' tests (vector/float assertions) ---------- */
function approxEqual(a: number, b: number, eps = EPSILON): void {
  assert.ok(Math.abs(a - b) <= eps, `Expected ${a} ≈ ${b}`);
}

function approxVector(actual: number[], expected: number[], eps = EPSILON): void {
  assert.strictEqual(actual.length, expected.length, `Vector length mismatch (${actual.length} !== ${expected.length})`);
  for (let i = 0; i < expected.length; i += 1) {
    approxEqual(actual[i], expected[i], eps);
  }
}

function valueToVector(value: ValueJSON): number[] {
  if ("vector" in value) return value.vector.slice();
  if ("vec4" in value) return value.vec4.slice();
  if ("vec3" in value) return value.vec3.slice();
  if ("vec2" in value) return value.vec2.slice();
  if ("quat" in value) return value.quat.slice();
  throw new Error(`Value is not a vector-like payload: ${JSON.stringify(value)}`);
}

function valueToFloat(value: ValueJSON): number {
  if ("float" in value) return value.float;
  throw new Error(`Value is not a float payload: ${JSON.stringify(value)}`);
}

function findWrite(res: EvalResult, path: string) {
  const match = res.writes.find((w) => w.path === path);
  if (!match) {
    throw new Error(`Expected write for path '${path}'`);
  }
  return match;
}

function writesToMap(writes: EvalResult["writes"]): Map<string, ValueJSON> {
  const map = new Map<string, ValueJSON>();
  for (const w of writes) {
    map.set(w.path, w.value);
  }
  return map;
}

/* ---------- Unified runSample used by many tests ---------- */
interface RunOptions {
  prepare?: (graph: Graph) => void | Promise<void>;
  expectations?: (VectorExpectation | FloatExpectation)[];
}
interface VectorExpectation { path: string; expectVector: number[]; }
interface FloatExpectation { path: string; expectFloat: number; }

async function runSample(name: string, spec: EvalSpec | GraphSpec | string, options: RunOptions = {}): Promise<EvalResult> {
  const g = new Graph();
  try {
    // Validate the JS/JSON spec before handing to the WASM loader to produce
    // more actionable errors in this test harness.
    const raw = spec as any;
    if (raw && typeof raw === "object" && Array.isArray((raw as any).nodes)) {
      for (let i = 0; i < (raw as any).nodes.length; i += 1) {
        const node = (raw as any).nodes[i];
        if (!node || typeof node !== "object" || typeof node.type !== "string") {
          throw new Error(
            `Spec validation failed for sample '${name}': node at index ${i} missing or invalid 'type': ${JSON.stringify(
              node,
            )}`,
          );
        }
      }
    }

    // Graph::loadGraph accepts either object or JSON string depending on spec
    (g as any).loadGraph(spec as any);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    throw new Error(`Failed to load sample '${name}': ${message}`);
  }

  if (options.prepare) await options.prepare(g);

  g.setTime(0);
  let res: EvalResult = g.evalAll();
  // Do an extra step to exercise dt/stateful nodes as original tests did
  g.step(1 / 60);
  res = g.evalAll();

  assert.ok(Array.isArray(res.writes) && res.writes.length > 0, `Sample '${name}' produced no writes`);
  for (const w of res.writes) {
    assert.ok(w.path && typeof w.path === "string", `Sample '${name}' produced a write with invalid path`);
    assert.ok(w.value && typeof w.value === "object", `Sample '${name}' produced a write with invalid value`);
    assert.ok(w.shape && typeof w.shape === "object", `Sample '${name}' produced a write missing shape metadata`);
    assertValueFinite(w.value, `${name}:${w.path}`);
  }

  assert.ok(res.nodes && typeof res.nodes === "object", `Sample '${name}' did not return per-node outputs`);

  // Apply expectations if present (strict vector/float assertions)
  if (options.expectations) {
    for (const exp of options.expectations) {
      const write = findWrite(res, exp.path);
      if ("expectVector" in exp) {
        approxVector(valueToVector(write.value), exp.expectVector);
      } else if ("expectFloat" in exp) {
        approxEqual(valueToFloat(write.value), exp.expectFloat);
      } else {
        throw new Error(`Sample '${name}' expectation missing assertion type`);
      }
    }
  }

  return res;
}

/* ---------- Simple helpers reused by older tests ---------- */
function requireWrite(result: EvalResult, path: string): WriteOpJSON {
  const write = result.writes.find((w) => w.path === path);
  if (!write) {
    throw new Error(`Expected write for path '${path}'`);
  }
  return write;
}

function assertVector(write: WriteOpJSON, expected: number[], epsilon = 1e-4): void {
  const value: any = write.value;
  if (!value || !Array.isArray(value.vector)) {
    throw new Error(`Write '${write.path}' does not contain a vector value`);
  }
  const vector = value.vector as number[];
  if (vector.length !== expected.length) {
    throw new Error(`Vector length mismatch for '${write.path}': expected ${expected.length}, received ${vector.length}`);
  }
  vector.forEach((component, idx) => {
    if (!Number.isFinite(component)) {
      throw new Error(`Vector component ${idx} for '${write.path}' is not finite`);
    }
    if (Math.abs(component - expected[idx]) > epsilon) {
      throw new Error(`Vector component ${idx} for '${write.path}' differs: expected ${expected[idx]}, received ${component}`);
    }
  });
}

function assertFloat(write: WriteOpJSON, expected: number, epsilon = 1e-4): void {
  const value: any = write.value;
  if (!value || typeof value.float !== "number") {
    throw new Error(`Write '${write.path}' does not contain a float value`);
  }
  const actual = value.float;
  if (!Number.isFinite(actual)) {
    throw new Error(`Float value for '${write.path}' is not finite`);
  }
  if (Math.abs(actual - expected) > epsilon) {
    throw new Error(`Float value mismatch for '${write.path}': expected ${expected}, received ${actual}`);
  }
}

function assertBool(write: WriteOpJSON, expected: boolean): void {
  const value: any = write.value;
  if (!value || typeof value.bool !== "boolean") {
    throw new Error(`Write '${write.path}' does not contain a bool value`);
  }
  if (value.bool !== expected) {
    throw new Error(`Bool value mismatch for '${write.path}': expected ${expected}, received ${value.bool}`);
  }
}

function assertText(write: WriteOpJSON, expected: string): void {
  const value: any = write.value;
  if (!value || typeof value.text !== "string") {
    throw new Error(`Write '${write.path}' does not contain a text value`);
  }
  if (value.text !== expected) {
    throw new Error(`Text value mismatch for '${write.path}': expected '${expected}', received '${value.text}'`);
  }
}

/* ---------- Entrypoint: run all grouped checks ---------- */
(async () => {
  try {
    await init(pkgWasmUrl());

    // Basic smoke checks used across tests
    await runSample("oscillator-basics", oscillatorBasics);
    await runSample("logic-gate", logicGate);
    await runSample("tuple-spring-damp-slew", tupleSpringDampSlew);
    {

      const vecResult = await runSample("vector-playground", vectorPlayground);
      const vecWrites = writesToMap(vecResult.writes);
      expectNumericVector(vecWrites.get("samples/vector.sum"), [1, 3, 3], "samples/vector.sum");
      expectNumericVector(vecWrites.get("samples/vector.sub"), [1, 1, 3], "samples/vector.sub");
    }
    // layered-rig-blend checks
    {
      const layeredResult = await runSample("layered-rig-blend", layeredRigBlend);
      const layeredWrites = writesToMap(layeredResult.writes);
      expectNumericVector(layeredWrites.get("samples/rig.pose"), [0.475, 0.0625, 0.1], "samples/rig.pose");
      expectNumericVector(layeredWrites.get("samples/rig.weights"), [0.6, 0.25, 0.85], "samples/rig.weights");
      expectListOfText(layeredWrites.get("samples/rig.tags"), ["arm", "blend"], "samples/rig.tags");
      expectTupleOfFloats(layeredWrites.get("samples/rig.counterTuple"), [2, 3], "samples/rig.counterTuple");
    }

    // json blend graph
    {
      const jsonSpec = loadJsonFixture("blend-graph.json");
      const jsonResult = await runSample("json-blend-graph", jsonSpec);
      const jsonWrites = writesToMap(jsonResult.writes);
      expectNumericVector(jsonWrites.get("samples/json.pose"), [0.005, 0.115, 0.275], "samples/json.pose");
      expectNumericVector(jsonWrites.get("samples/json.weights"), [0.1, 0.3, 0.6], "samples/json.weights");
      expectRecordTexts(jsonWrites.get("samples/json.channels"), { primary: "left", secondary: "foot" }, "samples/json.channels");
    }

    // hierarchical blend / weighted average checks
    await runSample("hierarchical-blend", hierarchicalBlend, {
      expectations: [
        { path: "samples/nested.pose", expectVector: [0.025, 0.325, 0.225, 1, 2, 2] },
        { path: "samples/nested.offset_xy", expectVector: [0.025, 0.325] },
        { path: "samples/nested.offset_z", expectFloat: 0.225 },
        { path: "samples/nested.aim_distance", expectFloat: 3 },
        { path: "samples/nested.weight_2", expectFloat: 0.75 },
      ],
    });

    await runSample("weighted-average", weightedAverage, {
      expectations: [
        { path: "samples/weighted.sum", expectVector: [0.56, 0.3, 0.8] },
        { path: "samples/weighted.average", expectVector: [0.4666667, 0.25, 0.6666667] },
        { path: "samples/weighted.total", expectFloat: 1.2 },
      ],
    });

    // weighted-average-from-json (uses fixtures)
    {
      const weightedSpecJson = readFileSync(resolve(fixturesDir, "weighted-blend-graph.json"), "utf8");
      const weightedStage: Record<string, { value: ValueJSON; shape?: ShapeJSON }> = JSON.parse(readFileSync(resolve(fixturesDir, "weighted-blend-inputs.json"), "utf8"));
      await runSample("weighted-average-from-json", weightedSpecJson, {
        prepare: (graph) => {
          for (const [path, payload] of Object.entries(weightedStage)) {
            graph.stageInput(path, payload.value, payload.shape);
          }
        },
        expectations: [
          { path: "samples/weighted.sum", expectVector: [0.56, 0.3, 0.8] },
          { path: "samples/weighted.average", expectVector: [0.4666667, 0.25, 0.6666667] },
          { path: "samples/weighted.total", expectFloat: 1.2 },
        ],
      });
    }

    // New fixture exercising the WeightedSumVector + BlendWeightedAverage nodes
    {
      const wsSpecJson = readFileSync(resolve(fixturesDir, "weighted-sum-helper-graph.json"), "utf8");
      await runSample("weighted-sum-helper", wsSpecJson, {
        expectations: [
          { path: "samples/ws.sum", expectFloat: 3.0 },
          { path: "samples/ws.total", expectFloat: 1.5 },
          { path: "samples/ws.avg", expectFloat: 1.0 },
        ],
      });
    }

    // nested telemetry checks
    {
      const telemetryResult = await runSample("nested-telemetry", nestedTelemetry);
      assertVector(requireWrite(telemetryResult, "samples/telemetry.corrected"), [-0.5, 9.3, -0.3, -0.4, -0.45, 0.8]);
      assertVector(requireWrite(telemetryResult, "samples/telemetry.offsets"), [0.5, 0.5, 0.5, -0.5, -0.25, 0.75]);
      assertFloat(requireWrite(telemetryResult, "samples/telemetry.gain"), 0.6);
      assertText(requireWrite(telemetryResult, "samples/telemetry.label"), "imu");
      assertBool(requireWrite(telemetryResult, "samples/telemetry.active"), true);
    }

    // If we reached here, all consolidated checks passed
    // eslint-disable-next-line no-console
    console.log("All consolidated samples tests passed");
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
