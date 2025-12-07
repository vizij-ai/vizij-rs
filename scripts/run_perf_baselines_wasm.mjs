#!/usr/bin/env node
/**
 * Stage 2 WASM perf harness (Node).
 *
 * Mirrors a subset of the native benches using the packaged wasm bindings:
 *  - graph: simple-gain-offset fixture
 *  - animation: simple-scalar-ramp fixture
 *  - orchestrator: scalar-ramp-pipeline orchestration fixture
 *
 * Outputs markdown rows (append-only) and logs a summary table.
 *
 * Prereqs:
 *   pnpm run build:wasm   (ensures pkg/ + dist/ exist for the wasm packages)
 *   pnpm install          (bindings are workspace modules)
 */
import { performance } from "node:perf_hooks";
import { appendFileSync } from "node:fs";
import path from "node:path";

let toValueJSON;
try {
  ({ toValueJSON } = await import("@vizij/value-json"));
} catch (err) {
  const local = new URL("../npm/@vizij/value-json/dist/index.js", import.meta.url);
  ({ toValueJSON } = await import(local));
}

async function loadGraphWasm() {
  try {
    return await import("@vizij/node-graph-wasm");
  } catch (err) {
    const local = new URL(
      "../npm/@vizij/node-graph-wasm/dist/node-graph-wasm/src/index.js",
      import.meta.url,
    );
    return await import(local);
  }
}

async function loadAnimWasm() {
  try {
    return await import("@vizij/animation-wasm");
  } catch (err) {
    const local = new URL(
      "../npm/@vizij/animation-wasm/dist/animation-wasm/src/index.js",
      import.meta.url,
    );
    return await import(local);
  }
}

async function loadOrchWasm() {
  try {
    return await import("@vizij/orchestrator-wasm");
  } catch (err) {
    const local = new URL(
      "../npm/@vizij/orchestrator-wasm/dist/orchestrator-wasm/src/index.js",
      import.meta.url,
    );
    return await import(local);
  }
}

const RESULTS_PATH = path.resolve(
  process.cwd(),
  "..",
  "vizij_docs/current_documentation/perf_baselines.md",
);

const DT = 1 / 60;
const GRAPH_STEPS = 100;
const ANIM_STEPS = 100;
const ORCH_STEPS = 100;
const SAMPLES = 10;

// Kitchen defaults (align with Stage 1)
const DEFAULT_KITCHEN_SIZES = [1, 50, 500];
const DEFAULT_AMORT_STEPS = [500, 100, 100];
const DEFAULT_AMORT_SAMPLES = [100, 50, 10]; // kept for parity; we still use median of SAMPLES for Node harness

const STAGED_KITCHEN_SIZE = 500;
const STAGED_STEPS = 50;
const STAGED_SAMPLES = 5;
const RUN_ONLY_STAGED = process.env.RUN_STAGED_ONLY === "1";

const now = () => performance.now();

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[mid - 1] + sorted[mid]) / 2
    : sorted[mid];
}

const fmtNumber = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 3,
  maximumFractionDigits: 3,
});
function fmt(us) {
  return fmtNumber.format(us);
}

async function timeSamples(samples, fn) {
  // Measures the wall time of fn() for each sample (fn includes setup).
  const out = [];
  for (let i = 0; i < samples; i++) {
    const start = now();
    await fn();
    out.push((now() - start) * 1000); // microseconds
  }
  return median(out);
}

async function medianOf(samples, fn) {
  // Uses fn's returned microseconds; fn itself is not timed by this helper.
  const out = [];
  for (let i = 0; i < samples; i++) {
    out.push(await fn());
  }
  return median(out);
}

async function benchGraph() {
  const { init: initGraph, Graph, loadNodeGraphSpec } = await loadGraphWasm();
  await initGraph();
  const spec = await loadNodeGraphSpec("simple-gain-offset");

  const cold = await timeSamples(SAMPLES, async () => {
    const g = new Graph();
    g.loadGraph(spec);
    g.setTime(0);
    g.step(DT);
    g.evalAll();
  });

  const amort = await medianOf(SAMPLES, async () => {
    const g = new Graph();
    g.loadGraph(spec);
    g.setTime(0);
    // Warm once so plan/layout construction is outside the timed region.
    g.step(DT);
    g.evalAll();

    const t0 = now();
    for (let i = 0; i < GRAPH_STEPS; i++) {
      g.setTime(i * DT);
      g.step(DT);
      g.evalAll();
    }
    return ((now() - t0) * 1000) / GRAPH_STEPS;
  });

  return [
    ["vizij-graph-core-wasm", "graph_eval", "cold/simple-gain-offset", cold],
    [
      "vizij-graph-core-wasm",
      "graph_eval",
      "amortized_per_step/simple-gain-offset",
      amort,
    ],
  ];
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

function parseList(env, fallback) {
  const raw = process.env[env];
  if (!raw) return fallback;
  const parsed = raw
    .split(",")
    .map((s) => Number(s.trim()))
    .filter((n) => Number.isFinite(n) && n > 0);
  return parsed.length ? parsed : fallback;
}

async function benchGraphKitchen() {
  const { init: initGraph, Graph } = await loadGraphWasm();
  await initGraph();
  const sizes = parseList("GRAPH_KITCHEN_SIZES", DEFAULT_KITCHEN_SIZES);
  const stepsList = parseList("GRAPH_AMORT_STEPS", DEFAULT_AMORT_STEPS);
  const len = Math.min(sizes.length, stepsList.length);
  const rows = [];

  for (let i = 0; i < len; i++) {
    const size = sizes[i];
    const steps = Math.max(1, stepsList[i]);
    const name = `kitchen-${size}-blocks`;
    const spec = kitchenChain(size);

    const cold = await timeSamples(SAMPLES, async () => {
      const g = new Graph();
      g.loadGraph(spec);
      g.setTime(0);
      g.step(DT);
      g.evalAll();
    });

    const amort = await medianOf(SAMPLES, async () => {
      const g = new Graph();
      g.loadGraph(spec);
      g.setTime(0);
      // Warm once so plan build is excluded.
      g.step(DT);
      g.evalAll();
      const start = now();
      for (let s = 0; s < steps; s++) {
        g.setTime(s * DT);
        g.step(DT);
        g.evalAll();
      }
      return ((now() - start) * 1000) / steps;
    });

    rows.push([
      "vizij-graph-core-wasm",
      "graph_eval",
      `cold/${name}`,
      cold,
    ]);
    rows.push([
      "vizij-graph-core-wasm",
      "graph_eval",
      `amortized_per_step/${name}`,
      amort,
    ]);
  }

  return rows;
}

async function benchGraphKitchenStaged() {
  const { init: initGraph, Graph } = await loadGraphWasm();
  await initGraph();
  const size = STAGED_KITCHEN_SIZE;
  const steps = STAGED_STEPS;
  const spec = kitchenChain(size);
  const paths = [];
  for (let i = 0; i < size; i += 1) {
    paths.push(`kitchen/${i}/a`, `kitchen/${i}/b`);
  }
  const nodeIds = spec.nodes.map((n) => n.id);

  const runVariant = async (label, useF32) => {
    const samples = Math.min(SAMPLES, STAGED_SAMPLES);
    const med = await medianOf(samples, async () => {
      const g = new Graph();
      g.loadGraph(spec);
      g.setTime(0);
      // Warm once to build plan/cache outside timed loop.
      g.step(DT);
      g.evalAll();
      const buf = new Float32Array(1);
      const start = now();
      for (let s = 0; s < steps; s += 1) {
        const val = s;
        if (useF32) {
          if (typeof g.inner?.stage_input_f32 !== "function") {
            throw new Error("stage_input_f32 not available on wasm binding");
          }
          buf[0] = val;
          for (const path of paths) {
            g.inner.stage_input_f32(path, buf);
          }
        } else {
          for (const path of paths) {
            g.stageInput(path, val);
          }
        }
        g.setTime(s * DT);
        g.step(DT);
        g.evalAll();
        // Touch outputs in batch to include outbound marshalling cost.
        if (typeof g.inner?.get_outputs_batch === "function") {
          g.inner.get_outputs_batch(nodeIds);
        }
      }
      return (now() - start) * 1000 / steps;
    });

    return [
      "vizij-graph-core-wasm",
      "graph_eval",
      `amortized_per_step/kitchen-500-staged-${label}`,
      med,
    ];
  };

  const rows = [];
  rows.push(await runVariant("json", false));
  try {
    const { Graph } = await loadGraphWasm();
    if (Graph && typeof Graph.prototype.stageInputsByIndex === "function") {
      const samples = Math.min(SAMPLES, STAGED_SAMPLES);
      const med = await medianOf(samples, async () => {
        const g = new Graph();
        g.loadGraph(spec);
        g.setTime(0);
        const indices = g.registerInputPaths(paths);
        if (typeof g.prepareInputSlots === "function") {
          g.prepareInputSlots(indices);
        }
        const vals = new Float32Array(paths.length);
        // Warm once to build plan/cache outside timed loop.
        vals.fill(0);
        if (typeof g.stageInputsBySlot === "function") {
          g.stageInputsBySlot(indices, vals);
        } else {
          g.stageInputsByIndex(indices, vals);
        }
        g.step(DT);
        g.evalAll();

        const start = now();
        for (let s = 0; s < steps; s += 1) {
          vals.fill(s);
          if (typeof g.stageInputsBySlot === "function") {
            g.stageInputsBySlot(indices, vals);
          } else {
            g.stageInputsByIndex(indices, vals);
          }
          g.setTime(s * DT);
          g.step(DT);
          g.evalAll();
          if (typeof g.inner?.get_outputs_batch === "function") {
            g.inner.get_outputs_batch(nodeIds);
          }
        }
        return ((now() - start) * 1000) / steps;
      });
      rows.push([
        "vizij-graph-core-wasm",
        "graph_eval",
        `amortized_per_step/kitchen-500-staged-f32-batch`,
        med,
      ]);
    } else {
      console.warn("[staged-kitchen] stageInputs not available on wrapper; batch variant skipped.");
    }
  } catch (err) {
    console.warn("[staged-kitchen] f32 batch variant skipped:", err?.message ?? err);
  }
  return rows.filter(Boolean);
}

async function benchAnimation() {
  const { init: initAnim, Engine } = await loadAnimWasm();
  await initAnim();

  const animDefs = [
    { label: "mixed-16x32", tracks: 16, keys: 32 },
    { label: "mixed-64x16", tracks: 64, keys: 16 },
    { label: "mixed-256x8", tracks: 256, keys: 8 },
  ];

  const results = [];

  const makeAnim = ({ tracks, keys }) => ({
    id: `${tracks}x${keys}`,
    name: `${tracks}x${keys}`,
    tracks: Array.from({ length: tracks }, (_, i) => {
      const last = keys - 1;
      const points = Array.from({ length: keys }, (_, k) => {
        const t = k / last;
        const value =
          i % 3 === 0
            ? k
            : i % 3 === 1
              ? { x: t, y: t * 0.5, z: t * 0.25 }
              : { x: 0, y: 0, z: t, w: 1 };
        const transitions =
          k === 0
            ? { out: { x: 0, y: 0 } }
            : k === last
              ? { in: { x: 1, y: 1 } }
              : { in: { x: 0.5, y: 0.5 }, out: { x: 0.5, y: 0.5 } };
        return { id: `k${k}`, stamp: t, value, transitions };
      });
      return {
        id: `track_${i}`,
        name: `Track ${i}`,
        animatableId: `bench/${i}`,
        points,
        settings: {},
      };
    }),
    groups: {},
    duration: 2000,
  });

  for (const def of animDefs) {
    const anim = makeAnim(def);

    const cold = await timeSamples(SAMPLES, async () => {
      const engine = new Engine();
      engine.loadAnimation(anim, { format: "stored" });
      const player = engine.createPlayer("bench");
      engine.addInstance(player, anim.id, {
        time_scale: 1.0,
        weight: 1.0,
        enabled: true,
        start_offset: 0,
      });
      engine.updateValues(DT);
    });

    const amort = await medianOf(SAMPLES, async () => {
      const engine = new Engine();
      engine.loadAnimation(anim, { format: "stored" });
      const player = engine.createPlayer("bench");
      engine.addInstance(player, anim.id, {
        time_scale: 1.0,
        weight: 1.0,
        enabled: true,
        start_offset: 0,
      });
      // Warm once to build any internal tables.
      engine.updateValues(DT);
      const start = now();
      for (let i = 0; i < ANIM_STEPS; i++) {
        engine.updateValues(DT);
      }
      return ((now() - start) * 1000) / ANIM_STEPS;
    });

    results.push([
      "vizij-animation-core-wasm",
      "animation_step",
      `cold/${def.label}`,
      cold,
    ]);
    results.push([
      "vizij-animation-core-wasm",
      "animation_step",
      `amortized_per_step/${def.label}`,
      amort,
    ]);
  }

  return results;
}

async function benchOrchestrator() {
  const { init: initOrch, Orchestrator } = await loadOrchWasm();
  await initOrch();
  const results = [];

  // Shared kitchen graph builder
  const graphCfg = (id, blocks, basePath) => ({
    id,
    spec: kitchenChain(blocks, basePath),
    subs: {
      inputs: Array.from({ length: blocks }, (_, i) => `${basePath}/in_a_${i}`),
      outputs: Array.from({ length: blocks }, (_, i) => `${basePath}/out_scalar_${i}`),
      mirrorWrites: false,
    },
  });

  // Synthetic animation (matches graph inputs)
  const synthAnim = (tracks, keys, basePath, blocks) => {
    const last = keys - 1;
    const tracksArr = Array.from({ length: tracks }, (_, i) => {
      const points = Array.from({ length: keys }, (_, k) => {
        const t = k / last;
        const value =
          i % 3 === 0
            ? k
            : i % 3 === 1
              ? { x: t, y: t * 0.5, z: t * 0.25 }
              : { x: 0, y: 0, z: t, w: 1 };
        return {
          id: `k${k}`,
          stamp: t,
          value,
          transitions: { in: k ? { x: 0.5, y: 0.5 } : undefined, out: k === last ? undefined : { x: 0.5, y: 0.5 } },
        };
      });
      return {
        id: `track_${i}`,
        name: `Track ${i}`,
        animatableId: `${basePath}/in_a_${i % blocks}`,
        points,
        settings: {},
      };
    });
    return {
      id: `${tracks}x${keys}-${basePath}`,
      name: `${tracks}x${keys}`,
      tracks: tracksArr,
      groups: {},
      duration: 2000,
    };
  };

  // Case 1: single 20b graph, one 64x16 anim
  const caseBlend = {
    label: "20bx64tx16k-blend",
    graphs: [graphCfg("bench-graph-blend", 20, "bench/blend")],
    anims: [synthAnim(64, 16, "bench/blend", 20)],
  };

  // Case 2: merged two 20b graphs, two 64x16 anims
  const caseMerged = {
    label: "merged-2x20b-2x64tx16k",
    graphs: [
      graphCfg("bench-graph-g1", 20, "bench/merged/g1"),
      graphCfg("bench-graph-g2", 20, "bench/merged/g2"),
    ],
    anims: [
      synthAnim(64, 16, "bench/merged/g1", 20),
      synthAnim(64, 16, "bench/merged/g2", 20),
    ],
  };

  for (const kase of [caseBlend, caseMerged]) {
    const makeOrch = () => {
      const orch = new Orchestrator({ schedule: "SinglePass" });
      // Graphs (merge when multiple)
      if (kase.graphs.length === 1) {
        orch.registerGraph(kase.graphs[0]);
      } else {
        orch.registerMergedGraph({
          id: "merged",
          graphs: kase.graphs,
          strategy: { outputs: "error", intermediate: "blend" },
        });
      }
      // Anims
      kase.anims.forEach((anim, idx) => {
        orch.registerAnimation({
          id: anim.id ?? `anim-${idx}`,
          setup: { animation: anim, player: { name: `player-${idx}`, loop_mode: "loop" } },
        });
      });
      // Initial inputs: seed graph inputs with zeros to avoid unset paths
      kase.graphs.forEach((g) => {
        g.subs.inputs.forEach((p) => orch.setInput(p, toValueJSON(0), null));
      });
      return orch;
    };

    const cold = await timeSamples(SAMPLES, async () => {
      const orch = makeOrch();
      orch.step(DT);
    });

    const amort = await medianOf(SAMPLES, async () => {
      const orch = makeOrch();
      // Warm once to build plans before timing.
      orch.step(DT);
      const start = now();
      for (let i = 0; i < ORCH_STEPS; i++) {
        orch.step(DT);
      }
      return ((now() - start) * 1000) / ORCH_STEPS;
    });

    results.push([
      "vizij-orchestrator-core-wasm",
      "orchestrator_tick",
      `cold/${kase.label}`,
      cold,
    ]);
    results.push([
      "vizij-orchestrator-core-wasm",
      "orchestrator_tick",
      `amortized_per_step/${kase.label}`,
      amort,
    ]);
  }

  return results;
}

function appendRows(rows) {
  const timestamp = new Date().toISOString().replace("T", " ").replace("Z", "");
  const lines = rows
    .map(
      ([crate, bench, caseName, us]) =>
        `| ${timestamp} | ${crate} | ${bench} | ${caseName} | ${fmt(us)} | µs |`,
    )
    .join("\n");
  appendFileSync(RESULTS_PATH, `${lines}\n`);
  return lines;
}

async function main() {
  const results = [];
  if (!RUN_ONLY_STAGED) {
    results.push(...(await benchGraph()));
    results.push(...(await benchGraphKitchen()));
    results.push(...(await benchAnimation()));
    results.push(...(await benchOrchestrator()));
  }
  results.push(...(await benchGraphKitchenStaged()));
  const lines = appendRows(results);
  console.log("WASM perf results (appended to perf_baselines.md):");
  console.log(lines);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
