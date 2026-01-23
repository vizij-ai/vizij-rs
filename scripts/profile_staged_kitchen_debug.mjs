#!/usr/bin/env node
/**
 * Staged kitchen benchmark (debug build) for CPU profiling.
 *
 * - Uses the debug wasm build at crates/node-graph/vizij-graph-wasm/pkg-debug
 * - Runs only the staged input/slot path that changed recently.
 * - Keeps the timeline monotonic by stepping DT before each eval.
 * - Emits a deterministic output signature (sha256 of canonical JSON) so we
 *   can compare against the slots+delta bench for correctness.
 *
 * Env overrides:
 *   SIZE=<int>    number of kitchen blocks (default 500)
 *   STEPS=<int>   iterations to run per sample (default 200)
 *   SAMPLES=<int> repeat the run this many times and report median (default 3)
 */
import { performance } from "node:perf_hooks";
import { createHash } from "node:crypto";
import path from "node:path";
import { pathToFileURL } from "node:url";

const SIZE = Number(process.env.SIZE ?? 500);
const STEPS = Number(process.env.STEPS ?? 50);
const SAMPLES = Number(process.env.SAMPLES ?? 1);
const DT = 1 / 60;

// Load debug wasm locally to keep source maps/function names intact in profiles.
const wasmPath = pathToFileURL(
  path.resolve("crates/node-graph/vizij-graph-wasm/pkg-debug/vizij_graph_wasm.js"),
);
const { WasmGraph } = await import(wasmPath);

const now = () => performance.now();

const stableStringify = (value) => {
  const norm = (v) => {
    if (v === null) return null;
    const t = typeof v;
    if (t === "number" || t === "string" || t === "boolean") return v;
    if (t === "bigint") return Number(v);
    if (Array.isArray(v)) return v.map(norm);
    if (t === "object") {
      const out = {};
      for (const key of Object.keys(v).sort()) {
        out[key] = norm(v[key]);
      }
      return out;
    }
    return String(v);
  };
  return JSON.stringify(norm(value));
};

const signature = (value) =>
  createHash("sha256").update(stableStringify(value)).digest("hex");

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[mid - 1] + sorted[mid]) / 2
    : sorted[mid];
}

// --- Kitchen graph generator (mirrors Rust bench kitchen_block/chain) ---
function kitchenBlock(idx, basePath) {
  const nodes = [];
  const edges = [];
  const nid = (suffix) => `${suffix}_${idx}`;
  const pid = (p) => `${basePath}/${idx}/${p}`;

  // Inputs & constants
  const inA = nid("in_a");
  const inB = nid("in_b");
  nodes.push({ id: inA, type: "input", params: { path: pid("a"), value: idx } });
  nodes.push({
    id: inB,
    type: "input",
    params: { path: pid("b"), value: idx * 2 },
  });

  const c = (name, value) => {
    const id = nid(name);
    nodes.push({ id, type: "constant", params: { value } });
    return id;
  };
  const constGain = c("c_gain", 1.5);
  const constBias = c("c_bias", 0.25);
  const constDiv = c("c_div", 3.0);
  const constMin = c("c_min", -1.0);
  const constMax = c("c_max", 1.0);
  const inMin = c("in_min", -2.0);
  const inMax = c("in_max", 2.0);
  const outMin = c("out_min", 0.0);
  const outMax = c("out_max", 10.0);
  const constScalar = c("c_scalar", 0.75);
  const constFreq = c("c_freq", 2.0 + idx * 0.01);

  const vconst = nid("vconst");
  nodes.push({
    id: vconst,
    type: "vectorconstant",
    params: { x: 1.0, y: 2.0, z: 3.0 },
  });

  // Helpers
  const addEdge = (from, to, input) =>
    edges.push({
      from: { node_id: from, output: "out" },
      to: { node_id: to, input },
    });
  const addNode = (id, type, params = {}) => {
    nodes.push({ id, type, params });
    return id;
  };

  // Arithmetic chain
  const sum = addNode(nid("sum"), "add");
  addEdge(inA, sum, "operand_1");
  addEdge(inB, sum, "operand_2");

  const mult = addNode(nid("mult"), "multiply");
  addEdge(sum, mult, "operand_1");
  addEdge(constGain, mult, "operand_2");

  const div = addNode(nid("div"), "divide");
  addEdge(mult, div, "lhs");
  addEdge(constDiv, div, "rhs");

  // Trig + clamp/remap
  const sin = addNode(nid("sin"), "sin");
  addEdge(div, sin, "in");

  const clamp = addNode(nid("clamp"), "clamp");
  addEdge(sin, clamp, "in");
  addEdge(constMin, clamp, "min");
  addEdge(constMax, clamp, "max");

  const remap = addNode(nid("remap"), "remap");
  addEdge(clamp, remap, "in");
  addEdge(inMin, remap, "in_min");
  addEdge(inMax, remap, "in_max");
  addEdge(outMin, remap, "out_min");
  addEdge(outMax, remap, "out_max");

  // Vector ops
  const join = addNode(nid("join"), "join");
  addEdge(sin, join, "operand_1");
  addEdge(clamp, join, "operand_2");
  addEdge(remap, join, "operand_3");

  const vadd = addNode(nid("vadd"), "vectoradd");
  addEdge(vconst, vadd, "a");
  addEdge(join, vadd, "b");

  const vscale = addNode(nid("vscale"), "vectorscale");
  addEdge(vadd, vscale, "v");
  addEdge(constScalar, vscale, "scalar");

  const vnorm = addNode(nid("vnorm"), "vectornormalize");
  addEdge(vscale, vnorm, "in");

  const vdot = addNode(nid("vdot"), "vectordot");
  addEdge(vnorm, vdot, "a");
  addEdge(vconst, vdot, "b");

  // Logic/comparisons
  const gt = addNode(nid("gt"), "greaterthan");
  addEdge(clamp, gt, "lhs");
  addEdge(remap, gt, "rhs");

  const lt = addNode(nid("lt"), "lessthan");
  addEdge(sin, lt, "lhs");
  addEdge(clamp, lt, "rhs");

  const and = addNode(nid("and"), "and");
  addEdge(gt, and, "lhs");
  addEdge(lt, and, "rhs");

  const not = addNode(nid("not"), "not");
  addEdge(lt, not, "in");

  // Time + oscillator + transitions
  const time = addNode(nid("time"), "time");
  const osc = addNode(nid("osc"), "oscillator", { frequency: 1.0 });
  addEdge(constFreq, osc, "frequency");

  const slew = addNode(nid("slew"), "slew", { max_rate: 5.0 });
  addEdge(osc, slew, "in");

  const damp = addNode(nid("damp"), "damp", { half_life: 0.2 });
  addEdge(remap, damp, "in");

  const spring = addNode(nid("spring"), "spring", {
    stiffness: 80.0,
    damping: 12.0,
    mass: 1.0,
  });
  addEdge(clamp, spring, "in");

  // If
  const sel = addNode(nid("sel"), "if");
  addEdge(and, sel, "cond");
  addEdge(spring, sel, "then");
  addEdge(damp, sel, "else");

  // Outputs
  const outScalar = addNode(nid("out_scalar"), "output", {
    path: pid("out_scalar"),
  });
  addEdge(sel, outScalar, "in");

  const outVec = addNode(nid("out_vec"), "output", { path: pid("out_vec") });
  addEdge(vnorm, outVec, "in");

  const outBool = addNode(nid("out_bool"), "output", { path: pid("out_bool") });
  addEdge(not, outBool, "in");

  // chain exit
  const exitId = remap;
  const entryTarget = sum;

  return { nodes, edges, entryTarget, entryPort: "operand_3", exitId };
}

function kitchenChain(blocks, basePath = "bench") {
  const b = Math.max(1, blocks);
  const nodes = [];
  const edges = [];
  let prev = null;
  for (let i = 0; i < b; i++) {
    const { nodes: n, edges: e, entryTarget, entryPort, exitId } = kitchenBlock(
      i,
      basePath,
    );
    if (prev) {
      e.push({
        from: { node_id: prev, output: "out" },
        to: { node_id: entryTarget, input: entryPort },
      });
    }
    nodes.push(...n);
    edges.push(...e);
    prev = exitId;
  }
  return { nodes, edges };
}

function makeGraph(size) {
  const spec = kitchenChain(size);
  const g = WasmGraph.new ? WasmGraph.new() : new WasmGraph();
  g.load_graph(JSON.stringify(spec));
  return { g, spec };
}

function registerInputs(g, size) {
  const paths = [];
  for (let i = 0; i < size; i += 1) {
    paths.push(`kitchen/${i}/a`, `kitchen/${i}/b`);
  }
  const idx = g.register_input_paths(paths);
  g.prepare_input_slots(idx, Array(paths.length).fill(null));
  return { idx, paths };
}

function runSample() {
  const { g } = makeGraph(SIZE);
  const { idx, paths } = registerInputs(g, SIZE);
  const vals = new Float32Array(paths.length);

  // Warm to build plans/caches outside measured loop.
  vals.fill(0);
  g.stage_inputs_slots(idx, vals);
  g.set_time(0);
  g.step(DT);
  g.eval_all();

  const start = now();
  for (let s = 0; s < STEPS; s += 1) {
    vals.fill(s);
    g.stage_inputs_slots(idx, vals);
    g.set_time(s * DT);
    g.step(DT); // ensure positive dt inside runtime
    g.eval_all();
  }
  return (now() - start) * 1000 / STEPS; // microseconds per step
}

function computeSignature() {
  const { g } = makeGraph(SIZE);
  const { idx, paths } = registerInputs(g, SIZE);
  const vals = new Float32Array(paths.length);

  // Warm once
  vals.fill(0);
  g.stage_inputs_slots(idx, vals);
  g.set_time(0);
  g.step(DT);
  g.eval_all();

  // Deterministic sample step
  vals.fill(1);
  g.stage_inputs_slots(idx, vals);
  g.set_time(DT);
  g.step(DT);
  const outStr = g.eval_all();
  const outObj = JSON.parse(outStr);
  return signature(outObj);
}

function main() {
  const sig = computeSignature();
  const samples = [];
  for (let i = 0; i < SAMPLES; i += 1) {
    samples.push(runSample());
  }
  const med = median(samples);
  console.log(
    `staged_kitchen_debug size=${SIZE} steps=${STEPS} samples=${SAMPLES} ` +
      `median_per_step_us=${med.toFixed(3)}`,
  );
  console.log(`output_signature_sha256=${sig}`);
}

main();
