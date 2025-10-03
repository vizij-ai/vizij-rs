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
  graphSamples,
  urdfIkPosition,
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

function asValueObject(value: ValueJSON | undefined): Record<string, unknown> | null {
  if (!value || typeof value !== "object") {
    return null;
  }
  return value as Record<string, unknown>;
}

function assertValueFinite(value: ValueJSON, context: string): void {
  if (typeof value === "number") {
    ensureFiniteNumber(value, context);
    return;
  }
  if (typeof value === "boolean" || typeof value === "string") {
    return;
  }
  if (!value || typeof value !== "object") {
    return;
  }
  const obj = value as Record<string, unknown>;
  if ("float" in obj) {
    ensureFiniteNumber(obj.float as number, context);
    return;
  }
  if ("bool" in obj) return;
  if ("text" in obj) return;
  if ("vec2" in obj) {
    (obj.vec2 as number[]).forEach((v: number, idx: number) => ensureFiniteNumber(v, `${context}.vec2[${idx}]`));
    return;
  }
  if ("vec3" in obj) {
    (obj.vec3 as number[]).forEach((v: number, idx: number) => ensureFiniteNumber(v, `${context}.vec3[${idx}]`));
    return;
  }
  if ("vec4" in obj) {
    (obj.vec4 as number[]).forEach((v: number, idx: number) => ensureFiniteNumber(v, `${context}.vec4[${idx}]`));
    return;
  }
  if ("quat" in obj) {
    (obj.quat as number[]).forEach((v: number, idx: number) => ensureFiniteNumber(v, `${context}.quat[${idx}]`));
    return;
  }
  if ("color" in obj) {
    (obj.color as number[]).forEach((v: number, idx: number) => ensureFiniteNumber(v, `${context}.color[${idx}]`));
    return;
  }
  if ("transform" in obj) {
    const transform = obj.transform as {
      translation: number[];
      rotation: number[];
      scale: number[];
    };
    transform.translation.forEach((v: number, idx: number) =>
      ensureFiniteNumber(v, `${context}.transform.translation[${idx}]`),
    );
    transform.rotation.forEach((v: number, idx: number) =>
      ensureFiniteNumber(v, `${context}.transform.rotation[${idx}]`),
    );
    transform.scale.forEach((v: number, idx: number) =>
      ensureFiniteNumber(v, `${context}.transform.scale[${idx}]`),
    );
    return;
  }
  if ("vector" in obj) {
    (obj.vector as number[]).forEach((v: number, idx: number) => ensureFiniteNumber(v, `${context}.vector[${idx}]`));
    return;
  }
  if ("record" in obj) {
    for (const [key, child] of Object.entries(obj.record as Record<string, ValueJSON>)) {
      assertValueFinite(child, `${context}.${key}`);
    }
    return;
  }
  if ("array" in obj) {
    (obj.array as ValueJSON[]).forEach((child: ValueJSON, idx: number) => assertValueFinite(child, `${context}[${idx}]`));
    return;
  }
  if ("list" in obj) {
    (obj.list as ValueJSON[]).forEach((child: ValueJSON, idx: number) => assertValueFinite(child, `${context}.list[${idx}]`));
    return;
  }
  if ("tuple" in obj) {
    (obj.tuple as ValueJSON[]).forEach((child: ValueJSON, idx: number) => assertValueFinite(child, `${context}.tuple[${idx}]`));
    return;
  }
  if ("enum" in obj) {
    const enumVal = obj.enum as { tag: string; value: ValueJSON };
    assertValueFinite(enumVal.value, `${context}<${enumVal.tag}>`);
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
  if (typeof value === "number") {
    if (expected.length !== 1) {
      throw new Error(`${label} expected ${expected.length} entries but received scalar ${value}`);
    }
    assertNearlyEqual(value, expected[0], label);
    return;
  }
  const obj = asValueObject(value);
  if (!obj) {
    throw new Error(`${label} expected numeric vector value`);
  }
  assertValueFinite(value!, label);

  let actual: number[] | undefined;
  if ("vector" in obj) {
    actual = (obj.vector as number[]).slice();
  } else if ("vec2" in obj) {
    actual = (obj.vec2 as number[]).slice();
  } else if ("vec3" in obj) {
    actual = (obj.vec3 as number[]).slice();
  } else if ("vec4" in obj) {
    actual = (obj.vec4 as number[]).slice();
  } else if ("quat" in obj) {
    actual = (obj.quat as number[]).slice();
  }

  if (!actual) {
    throw new Error(`${label} expected numeric vector, received ${JSON.stringify(value)}`);
  }
  if (actual.length !== expected.length) {
    throw new Error(`${label} expected length ${expected.length} but received ${actual.length}`);
  }
  actual.forEach((v: number, idx: number) => assertNearlyEqual(v, expected[idx], `${label}[${idx}]`));
}

function expectListOfText(value: ValueJSON | undefined, expected: string[], label: string): void {
  if (!value || typeof value !== "object" || value === null || !("list" in value)) {
    throw new Error(`${label} expected list of text values`);
  }
  const listEntries = (value as { list: ValueJSON[] }).list;
  const actual = listEntries.map((entry: ValueJSON, idx: number) => {
    if (entry && typeof entry === "object" && "text" in entry) {
      return (entry as { text: string }).text;
    }
    throw new Error(`${label} entry ${idx} expected text but received ${JSON.stringify(entry)}`);
  });
  if (actual.length !== expected.length) {
    throw new Error(`${label} expected ${expected.length} entries but received ${actual.length}`);
  }
  actual.forEach((text: string, idx: number) => {
    if (text !== expected[idx]) {
      throw new Error(`${label}[${idx}] expected '${expected[idx]}' but received '${text}'`);
    }
  });
}

function expectTupleOfFloats(value: ValueJSON | undefined, expected: number[], label: string): void {
  if (!value || typeof value !== "object" || value === null || !("tuple" in value)) {
    throw new Error(`${label} expected tuple value`);
  }
  const tuple = (value as { tuple: ValueJSON[] }).tuple;
  if (tuple.length !== expected.length) {
    throw new Error(`${label} expected ${expected.length} items but received ${tuple.length}`);
  }
  tuple.forEach((entry: ValueJSON, idx: number) => {
    const obj = asValueObject(entry);
    if (!obj || typeof obj.float !== "number") {
      throw new Error(`${label}[${idx}] expected float but received ${JSON.stringify(entry)}`);
    }
    assertNearlyEqual(obj.float, expected[idx], `${label}[${idx}]`);
  });
}

function expectRecordTexts(value: ValueJSON | undefined, expected: Record<string, string>, label: string): void {
  if (!value || typeof value !== "object" || value === null || !("record" in value)) {
    throw new Error(`${label} expected record value`);
  }
  const recordEntries = (value as { record: Record<string, ValueJSON> }).record;
  const actualEntries = Object.entries(recordEntries);
  const expectedEntries = Object.entries(expected);
  if (actualEntries.length !== expectedEntries.length) {
    throw new Error(`${label} expected ${expectedEntries.length} fields but received ${actualEntries.length}`);
  }
  for (const [key, expectedText] of expectedEntries) {
    const entry = recordEntries[key];
    const entryObj = asValueObject(entry);
    if (!entryObj || typeof entryObj.text !== "string") {
      throw new Error(`${label}.${key} expected text value`);
    }
    if (entryObj.text !== expectedText) {
      throw new Error(`${label}.${key} expected '${expectedText}' but received '${entryObj.text}'`);
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

function valueToVector(value: ValueJSON | undefined): number[] {
  const obj = asValueObject(value);
  if (!obj) {
    if (typeof value === "number") return [value];
    throw new Error(`Value is not a vector-like payload: ${JSON.stringify(value)}`);
  }
  if ("vector" in obj) return (obj.vector as number[]).slice();
  if ("vec4" in obj) return (obj.vec4 as number[]).slice();
  if ("vec3" in obj) return (obj.vec3 as number[]).slice();
  if ("vec2" in obj) return (obj.vec2 as number[]).slice();
  if ("quat" in obj) return (obj.quat as number[]).slice();
  throw new Error(`Value is not a vector-like payload: ${JSON.stringify(value)}`);
}

function valueToFloat(value: ValueJSON | undefined): number {
  if (typeof value === "number") return value;
  const obj = asValueObject(value);
  if (obj && "float" in obj) return obj.float as number;
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
  const vector = valueToVector(write.value);
  if (!vector) {
    throw new Error(`Write '${write.path}' does not contain a vector value`);
  }
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
  const actual = valueToFloat(write.value);
  if (typeof actual !== "number") {
    throw new Error(`Write '${write.path}' does not contain a float value`);
  }
  if (!Number.isFinite(actual)) {
    throw new Error(`Float value for '${write.path}' is not finite`);
  }
  if (Math.abs(actual - expected) > epsilon) {
    throw new Error(`Float value mismatch for '${write.path}': expected ${expected}, received ${actual}`);
  }
}

function assertBool(write: WriteOpJSON, expected: boolean): void {
  const obj = asValueObject(write.value);
  if (!obj || typeof obj.bool !== "boolean") {
    throw new Error(`Write '${write.path}' does not contain a bool value`);
  }
  if (obj.bool !== expected) {
    throw new Error(`Bool value mismatch for '${write.path}': expected ${expected}, received ${obj.bool}`);
  }
}

function assertText(write: WriteOpJSON, expected: string): void {
  const obj = asValueObject(write.value);
  if (!obj || typeof obj.text !== "string") {
    throw new Error(`Write '${write.path}' does not contain a text value`);
  }
  if (obj.text !== expected) {
    throw new Error(`Text value mismatch for '${write.path}': expected '${expected}', received '${obj.text}'`);
  }
}

/* ---------- Entrypoint: run all grouped checks ---------- */
(async () => {
  try {
    await init(pkgWasmUrl());

    const urdfSample = graphSamples["urdf-ik-position"];
    assert.ok(urdfSample, "graphSamples exposes urdf-ik-position");
    assert.equal(
      urdfSample,
      urdfIkPosition,
      "graphSamples should reference the urdfIkPosition sample"
    );
    const hasUrdfNode = urdfSample.nodes.some((node) => node.type === "urdfikposition");
    assert.ok(hasUrdfNode, "urdf sample must include a urdfikposition node");

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

    // URDF FK/IK round-trip coverage across multiple poses.
    {
      const urdfXml = `
<robot name="planar_arm">
  <link name="base_link" />
  <link name="link1" />
  <link name="link2" />
  <link name="link3" />
  <link name="link4" />
  <link name="link5" />
  <link name="link6" />
  <link name="tool" />

  <joint name="joint1" type="revolute">
    <parent link="base_link" />
    <child link="link1" />
    <origin xyz="0 0 0.1" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint2" type="revolute">
    <parent link="link1" />
    <child link="link2" />
    <origin xyz="0.2 0 0" rpy="0 0 0" />
    <axis xyz="0 1 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint3" type="revolute">
    <parent link="link2" />
    <child link="link3" />
    <origin xyz="0.2 0 0" rpy="0 0 0" />
    <axis xyz="1 0 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint4" type="revolute">
    <parent link="link3" />
    <child link="link4" />
    <origin xyz="0.2 0 0" rpy="0 0 0" />
    <axis xyz="0 0 1" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint5" type="revolute">
    <parent link="link4" />
    <child link="link5" />
    <origin xyz="0.15 0 0" rpy="0 0 0" />
    <axis xyz="0 1 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="joint6" type="revolute">
    <parent link="link5" />
    <child link="link6" />
    <origin xyz="0.1 0 0" rpy="0 0 0" />
    <axis xyz="1 0 0" />
    <limit lower="-3.1416" upper="3.1416" effort="1" velocity="1" />
  </joint>

  <joint name="tool_joint" type="fixed">
    <parent link="link6" />
    <child link="tool" />
    <origin xyz="0.1 0 0" rpy="0 0 0" />
  </joint>
</robot>
`.trim();

      const fkIkGraph: GraphSpec = {
        nodes: [
          {
            id: "joint_input",
            type: "input",
            params: {
              path: "tests/urdf.joints",
              value: { vector: [0, 0, 0, 0, 0, 0] },
            },
          },
          {
            id: "fk",
            type: "urdffk",
            params: {
              urdf_xml: urdfXml,
              root_link: "base_link",
              tip_link: "tool",
            },
            inputs: { joints: { node_id: "joint_input" } },
          },
          {
            id: "ik",
            type: "urdfikposition",
            params: {
              urdf_xml: urdfXml,
              root_link: "base_link",
              tip_link: "tool",
              max_iters: 256,
              tol_pos: 0.0005,
            },
            inputs: {
              target_pos: { node_id: "fk", output_key: "position" },
              seed: { node_id: "joint_input" },
            },
          },
        ],
      };

      const fkIkGraphInstance = new Graph();
      fkIkGraphInstance.loadGraph(fkIkGraph);
      fkIkGraphInstance.setTime(0);

      const expectedJointNames = ["joint1", "joint2", "joint3", "joint4", "joint5", "joint6"] as const;
      const jointSamples: number[][] = [
        [0, 0, 0, 0, 0, 0],
        [0.2, -0.1, 0.15, -0.2, 0.1, -0.05],
        [-0.25, 0.2, -0.18, 0.22, -0.12, 0.08],
        [0.35, -0.28, 0.24, 0.18, -0.16, 0.12],
        [-0.3, 0.18, 0.12, -0.26, 0.2, -0.14],
      ];

      jointSamples.forEach((angles, sampleIdx) => {
        // Forward pass: compute FK pose for the chosen joint angles.
        console.log("FK Target", angles)
        fkIkGraphInstance.stageInput("tests/urdf.joints", angles);
        const fkResult = fkIkGraphInstance.evalAll();
        const fkNode = fkResult.nodes?.fk as any;
        assert.ok(fkNode, `fk node result missing for sample ${sampleIdx}`);
        const fkPositionValue: ValueJSON | undefined = fkNode.position?.value;
        assert.ok(fkPositionValue, `fk position missing for sample ${sampleIdx}`);
        assertValueFinite(fkPositionValue, `fk.position[${sampleIdx}]`);
        const targetPosition = valueToVector(fkPositionValue);
        console.log("ik target", targetPosition);

        const ikNode = fkResult.nodes?.ik as any;
        assert.ok(ikNode, `ik node result missing for sample ${sampleIdx}`);
        const ikValue: ValueJSON | undefined = ikNode.out?.value;
        console.log("ik result", ikValue, "\n");
        const ikObj = asValueObject(ikValue);
        assert.ok(ikObj && "record" in ikObj, `ik output missing record for sample ${sampleIdx}`);

        const ikRecord = (ikObj!.record as Record<string, ValueJSON>);
        expectedJointNames.forEach((jointName) => {
          const entry = ikRecord[jointName];
          const entryObj = asValueObject(entry);
          assert.ok(entryObj && typeof entryObj.float === "number", `ik output missing float for joint '${jointName}' (sample ${sampleIdx})`);
          const jointAngle = entryObj!.float as number;
          assert.ok(Number.isFinite(jointAngle), `ik joint '${jointName}' produced non-finite value`);
        });

        const ikAngles = expectedJointNames.map((jointName) => {
          const entryObj = asValueObject(ikRecord[jointName]);
          return typeof entryObj?.float === "number" ? (entryObj.float as number) : 0;
        });

        // Backward pass: feed IK joint solution into FK and ensure pose matches.
        fkIkGraphInstance.stageInput("tests/urdf.joints", ikAngles);
        const validationResult = fkIkGraphInstance.evalAll();
        const validationFkNode = validationResult.nodes?.fk as any;
        const validationPosValue: ValueJSON | undefined = validationFkNode?.position?.value;
        assert.ok(validationPosValue, `validation fk position missing for sample ${sampleIdx}`);
        assertValueFinite(validationPosValue, `validation fk.position[${sampleIdx}]`);
        const validationPosition = valueToVector(validationPosValue);
        approxVector(validationPosition, targetPosition, 1e-3);

        fkIkGraphInstance.step(1 / 120);
      });
    }
  } catch (err) {
    // eslint-disable-next-line no-console
    console.error(err);
    process.exit(1);
  }
})();
