#!/usr/bin/env node
/**
 * Stage 2 WASM perf harness (Node).
 *
 * - Uses canonical, hashed fixtures from fixtures/perf_scenarios.
 * - Emits per-scenario rows with env/build/ABI metadata into
 *   vizij_docs/current_documentation/perf_baselines.md.
 * - VERIFY_ONLY=1 will only recompute signatures and timing deltas (no append).
 * - UPDATE_GOLDEN=1 will refresh fixtures/perf_scenarios/goldens.json.
 * - SMOKE=1 runs the minimal set for CI (signatures + order-of-magnitude timing).
 */

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { execSync } from "node:child_process";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const RESULTS_PATH = path.resolve(
  ROOT,
  "..",
  "vizij_docs/current_documentation/perf_baselines.md",
);
const META_PATH = path.join(ROOT, "fixtures/perf_scenarios/index.json");
const GOLDEN_PATH = path.join(ROOT, "fixtures/perf_scenarios/goldens.json");
const ENV_LABEL = "node-wasm";
const DEFAULT_DT = 1 / 60;
const DEFAULT_VARIANCE = Number(process.env.VARIANCE_PCT ?? "0.35");
const ORDER_MAG_FACTOR = Number(process.env.ORDER_MAGNITUDE ?? "10");
const VERIFY_ONLY =
  process.env.VERIFY_ONLY === "1" || process.argv.includes("--verify");
const UPDATE_GOLDEN =
  process.env.UPDATE_GOLDEN === "1" || process.argv.includes("--update-golden");
const SMOKE = process.env.SMOKE === "1" || process.argv.includes("--smoke");
const now = () => performance.now();

const meta = JSON.parse(readFileSync(META_PATH, "utf8"));
const goldens = JSON.parse(readFileSync(GOLDEN_PATH, "utf8"));
const SMOKE_IDS = new Set([
  "graph-smoke",
  "graph-defaults-only",
  "animation-mixed-small",
  "orchestrator-merged-blend",
]);

function stableStringify(value) {
  if (typeof value === "bigint") return `"${value}n"`;
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(stableStringify).join(",")}]`;
  const keys = Object.keys(value).sort();
  return `{${keys.map((k) => `${JSON.stringify(k)}:${stableStringify(value[k])}`).join(",")}}`;
}

function hashValue(val) {
  return createHash("sha256").update(stableStringify(val)).digest("hex");
}

function hashFile(p) {
  return createHash("sha256").update(readFileSync(p)).digest("hex");
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[mid - 1] + sorted[mid]) / 2
    : sorted[mid];
}

function ensureDoc() {
  if (!readFileSyncSafe(RESULTS_PATH)) {
    const header = `# Performance Baselines
<!-- tags: status=tracking; topics=performance,benchmarking -->

Tables are append-only. Use \`UPDATE_GOLDEN=1 node scripts/run_perf_baselines_wasm.mjs\` to refresh signatures/medians when you intentionally change a fixture or ABI. \`SMOKE=1 VERIFY_ONLY=1\` is used in CI for order-of-magnitude checks.

## Node/WASM
| date | env | commit | wasm_abi | build | bench | scenario | samples | warmup_ticks | steps | dt | median_us | signature | delta_vs_golden | note |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
`;
    writeFileSync(RESULTS_PATH, header);
  }
}

function readFileSyncSafe(p) {
  try {
    return readFileSync(p, "utf8");
  } catch {
    return null;
  }
}

function resolveSpec(entry) {
  const full = path.resolve(ROOT, entry.spec);
  const raw = readFileSync(full, "utf8");
  const sha = hashFile(full);
  if (entry.sha256 && entry.sha256 !== sha) {
    throw new Error(
      `Spec hash mismatch for ${entry.id}: expected ${entry.sha256}, got ${sha}`,
    );
  }
  return { spec: JSON.parse(raw), sha };
}

function collectInputPaths(spec) {
  return (spec.nodes ?? [])
    .filter((n) => n?.type === "input" && n.params?.path)
    .map((n) => String(n.params.path));
}

function asGraphSpec(raw) {
  if (raw?.nodes) return raw;
  if (raw?.spec?.nodes) return raw.spec;
  return raw;
}

function buildHotPaths(entry, spec) {
  if (Array.isArray(entry.paths) && entry.paths.length) return entry.paths;
  if (Array.isArray(entry.pathsPerBlock)) {
    const inputs = collectInputPaths(spec);
    const blockCount = Math.max(
      1,
      Math.round(inputs.filter((p) => p.includes("/a")).length),
    );
    const out = [];
    for (let i = 0; i < blockCount; i += 1) {
      entry.pathsPerBlock.forEach((tpl) => {
        out.push(tpl.replace("{i}", `${i}`));
      });
    }
    return out;
  }
  return collectInputPaths(spec);
}

async function loadGraphWasm() {
  try {
    return await import("@vizij/node-graph-wasm");
  } catch {
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
  } catch {
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
  } catch {
    const local = new URL(
      "../npm/@vizij/orchestrator-wasm/dist/orchestrator-wasm/src/index.js",
      import.meta.url,
    );
    return await import(local);
  }
}

function applyGraphInputs(graph, entry, dt, stepIdx, hotPaths) {
  if (!hotPaths.length) return;
  const vals = new Float32Array(hotPaths.length);
  const base = stepIdx;
  if (entry.mode === "ramp-hotpaths") {
    hotPaths.forEach((_, i) => {
      if (i === 0) vals[i] = stepIdx * dt;
      else if (i === 1) vals[i] = 1.5;
      else if (i === 2) vals[i] = 0.25;
      else vals[i] = base + i * 0.01;
    });
  } else if (entry.mode === "kitchen") {
    hotPaths.forEach((_, i) => {
      vals[i] = (base % 8) + i * 0.005;
    });
  }
  if (typeof graph.stageInputs === "function") {
    graph.stageInputs(hotPaths, vals);
  } else {
    hotPaths.forEach((p, i) => graph.stageInput(p, vals[i]));
  }
}

async function runGraphScenario(entry, abiInfo) {
  const resolved = resolveSpec(entry);
  const graphSpec = asGraphSpec(resolved.spec);
  const sha = resolved.sha;
  const { init, Graph, abi_version } = await loadGraphWasm();
  await init();
  const hotPaths = buildHotPaths(entry, graphSpec);
  const steps = entry.steps ?? 60;
  const warmup = entry.warmup ?? 0;
  const samples = entry.samples ?? 5;
  const dt = entry.dt ?? DEFAULT_DT;

  const perSample = [];
  let signature = "";

  for (let s = 0; s < samples; s++) {
    const g = new Graph();
    g.loadGraph(graphSpec, hotPaths.length ? { hotPaths, epsilon: 0 } : undefined);
    for (let w = 0; w < warmup; w++) {
      applyGraphInputs(g, entry, dt, w, hotPaths);
      g.setTime(w * dt);
      g.step(dt);
      g.evalAll();
    }
    let lastEval = null;
    const t0 = now();
    for (let i = 0; i < steps; i++) {
      applyGraphInputs(g, entry, dt, i, hotPaths);
      g.setTime(i * dt);
      g.step(dt);
      lastEval = g.evalAll();
    }
    const usPerStep = ((now() - t0) * 1000) / steps;
    signature = hashValue({ outputs: lastEval, hotPaths, steps, dt });
    perSample.push(usPerStep);
  }

  abiInfo.graph = abi_version?.() ?? abiInfo.graph ?? null;

  return {
    id: entry.id,
    bench: "graph_eval",
    median_us: median(perSample),
    signature,
    spec_sha: sha,
    samples,
    warmup,
    steps,
    dt,
  };
}

async function runAnimationScenario(entry, abiInfo) {
  const { spec, sha } = resolveSpec(entry);
  const { init, Engine, abi_version } = await loadAnimWasm();
  await init();
  const steps = entry.steps ?? 120;
  const warmup = entry.warmup ?? 0;
  const samples = entry.samples ?? 5;
  const dt = entry.dt ?? DEFAULT_DT;

  const perSample = [];
  let signature = "";

  for (let s = 0; s < samples; s++) {
    const engine = new Engine();
    engine.loadAnimation(spec, { format: "stored" });
    const player = engine.createPlayer("bench");
    engine.addInstance(player, spec.id ?? "anim", {
      time_scale: 1,
      weight: 1,
      enabled: true,
      start_offset: 0,
    });
    let outputs = null;
    for (let w = 0; w < warmup; w++) {
      outputs = engine.updateValues(dt);
    }
    const t0 = now();
    for (let i = 0; i < steps; i++) {
      outputs = engine.updateValues(dt);
    }
    const usPerStep = ((now() - t0) * 1000) / steps;
    signature = hashValue({ outputs, steps, dt });
    perSample.push(usPerStep);
  }

  abiInfo.animation = abi_version?.() ?? abiInfo.animation ?? null;

  return {
    id: entry.id,
    bench: "animation_step",
    median_us: median(perSample),
    signature,
    spec_sha: sha,
    samples,
    warmup,
    steps,
    dt,
  };
}

function loadGraphSpecFromFile(relPath) {
  const full = path.resolve(ROOT, relPath);
  const parsed = JSON.parse(readFileSync(full, "utf8"));
  return { spec: asGraphSpec(parsed), sha: hashFile(full) };
}

async function runOrchestratorScenario(entry, abiInfo) {
  const { spec: orchCfg, sha } = resolveSpec(entry);
  const { init, Orchestrator, abi_version } = await loadOrchWasm();
  await init();
  const steps = entry.steps ?? 60;
  const warmup = entry.warmup ?? 0;
  const samples = entry.samples ?? 5;
  const dt = entry.dt ?? DEFAULT_DT;

  const graphs = (orchCfg.graphs ?? []).map((g) => {
    const loaded = loadGraphSpecFromFile(g.spec);
    const inputs = collectInputPaths(loaded.spec);
    return {
      id: g.id ?? g.spec,
      ...loaded,
      inputs,
    };
  });
  const hotInputs = new Set(
    orchCfg.hotInputs === "all-input-nodes"
      ? graphs.flatMap((g) => g.inputs)
      : [],
  );

  const anims = (orchCfg.animations ?? []).map((a) => {
    const loaded = loadGraphSpecFromFile(a.spec);
    return {
      id: a.id ?? a.spec,
      spec: loaded.spec,
      sha: loaded.sha,
      bind: a.bind ?? graphs[0]?.id ?? "graph",
    };
  });

  const perSample = [];
  let signature = "";

  for (let s = 0; s < samples; s++) {
    const orch = new Orchestrator({ schedule: orchCfg.schedule ?? "SinglePass" });
    const stepFn =
      typeof orch.stepDelta === "function"
        ? (dt, token) => orch.stepDelta(dt, token)
        : (dt) => orch.step(dt);
    if (hotInputs.size && typeof orch.setHotInputs === "function") {
      orch.setHotInputs([...hotInputs], { epsilon: 0 });
    }

    const graphCfgs = graphs.map((g) => ({
      id: g.id,
      spec: g.spec,
      subs: g.inputs.length ? { inputs: g.inputs, mirrorWrites: false } : undefined,
    }));

    if (graphCfgs.length === 1) {
      orch.registerGraph(graphCfgs[0]);
    } else {
      orch.registerMergedGraph({
        id: "merged",
        graphs: graphCfgs,
        strategy: orchCfg.mergeStrategy ?? { outputs: "error", intermediate: "blend" },
      });
    }

    anims.forEach((a, idx) => {
      orch.registerAnimation({
        id: a.id ?? `anim-${idx}`,
        setup: {
          animation: a.spec,
          player: { name: a.id ?? `player-${idx}`, loop_mode: "loop" },
          instance: { weight: 1, time_scale: 1 },
        },
      });
    });

    const inputPaths = [...hotInputs];
    let frame = null;
    let version = 0n;

    for (let w = 0; w < warmup; w++) {
      if (inputPaths.length && typeof orch.setInputsSmart === "function") {
        const vals = new Float32Array(inputPaths.length).fill(w);
        orch.setInputsSmart(inputPaths, vals);
      }
      frame = stepFn(dt, version);
      if (frame?.version !== undefined) version = BigInt(frame.version);
      else version = version + 1n;
    }

    const t0 = now();
    for (let i = 0; i < steps; i++) {
      if (inputPaths.length && typeof orch.setInputsSmart === "function") {
        const vals = new Float32Array(inputPaths.length);
        for (let j = 0; j < inputPaths.length; j++) {
          vals[j] = (i % 7) + j * 0.01;
        }
        orch.setInputsSmart(inputPaths, vals);
      }
      frame = stepFn(dt, version);
      if (frame?.version !== undefined) version = BigInt(frame.version);
      else version = version + 1n;
    }
    const usPerStep = ((now() - t0) * 1000) / steps;
    signature = hashValue({ frame, steps, dt });
    perSample.push(usPerStep);
  }

  abiInfo.orchestrator = abi_version?.() ?? abiInfo.orchestrator ?? null;

  const sigSha = hashValue({
    orchestrator_spec: sha,
    graph_shas: graphs.map((g) => g.sha),
    anim_shas: anims.map((a) => a.sha),
    signature,
  });

  return {
    id: entry.id,
    bench: "orchestrator_tick",
    median_us: median(perSample),
    signature: sigSha,
    spec_sha: sha,
    samples,
    warmup,
    steps,
    dt,
  };
}

function evaluateAgainstGolden(result) {
  const golden = goldens[result.id] ?? {};
  const tolerance = golden.tolerance_pct ?? DEFAULT_VARIANCE;
  const sigMatch = golden.signature
    ? golden.signature === result.signature
    : null;
  const pctDelta =
    golden.median_us && golden.median_us > 0
      ? (result.median_us - golden.median_us) / golden.median_us
      : null;
  const varianceWarn =
    typeof pctDelta === "number" && Math.abs(pctDelta) > tolerance;
  const oomWarn =
    typeof pctDelta === "number" && Math.abs(pctDelta) > ORDER_MAG_FACTOR;
  return { sigMatch, pctDelta, varianceWarn, oomWarn, golden, tolerance };
}

function updateGolden(result) {
  goldens[result.id] = {
    signature: result.signature,
    median_us: result.median_us,
    tolerance_pct: goldens[result.id]?.tolerance_pct ?? DEFAULT_VARIANCE,
  };
}

function appendRows(rows, abiInfo) {
  ensureDoc();
  const timestamp = new Date().toISOString().replace("T", " ").replace("Z", "");
  let commit = "unknown";
  try {
    commit = execSync("git rev-parse --short HEAD", { cwd: ROOT })
      .toString()
      .trim();
  } catch {
    /* fall back to unknown */
  }
  const abiString = `g${abiInfo.graph ?? "?"}-a${abiInfo.animation ?? "?"}-o${abiInfo.orchestrator ?? "?"}`;
  const buildType = process.env.VIZIJ_BUILD_TYPE ?? "release";

  const lines = rows
    .map((r) => {
      const goldenEval = evaluateAgainstGolden(r);
      const delta =
        typeof goldenEval.pctDelta === "number"
          ? `${(goldenEval.pctDelta * 100).toFixed(1)}%`
          : "";
      const noteParts = [];
      if (goldenEval.sigMatch === false) noteParts.push("signature drift");
      if (goldenEval.varianceWarn) noteParts.push(`> ${goldenEval.tolerance ?? DEFAULT_VARIANCE} var`);
      const note = noteParts.join("; ");
      return `| ${timestamp} | ${ENV_LABEL} | ${commit} | ${abiString} | ${buildType} | ${r.bench} | ${r.id} | ${r.samples} | ${r.warmup} | ${r.steps} | ${r.dt} | ${r.median_us.toFixed(3)} | ${r.signature.slice(0, 12)} | ${delta} | ${note} |`;
    })
    .join("\n");
  appendFileSync(RESULTS_PATH, `${lines}\n`);
  return lines;
}

function selectScenarios() {
  const pick = (arr) => (SMOKE ? arr.filter((e) => SMOKE_IDS.has(e.id)) : arr);
  return {
    graph: pick(meta.graph ?? []),
    animation: pick(meta.animation ?? []),
    orchestrator: pick(meta.orchestrator ?? []),
  };
}

async function main() {
  const abiInfo = { graph: null, animation: null, orchestrator: null };
  const scenarios = selectScenarios();
  const results = [];

  for (const entry of scenarios.graph) {
    if (VERIFY_ONLY || SMOKE) console.log(`Running graph:${entry.id}`);
    results.push(await runGraphScenario(entry, abiInfo));
  }
  for (const entry of scenarios.animation) {
    if (VERIFY_ONLY || SMOKE) console.log(`Running animation:${entry.id}`);
    results.push(await runAnimationScenario(entry, abiInfo));
  }
  for (const entry of scenarios.orchestrator) {
    if (VERIFY_ONLY || SMOKE) console.log(`Running orchestrator:${entry.id}`);
    results.push(await runOrchestratorScenario(entry, abiInfo));
  }

  if (UPDATE_GOLDEN) {
    results.forEach(updateGolden);
    writeFileSync(GOLDEN_PATH, JSON.stringify(goldens, null, 2));
    console.log("Updated goldens:", GOLDEN_PATH);
  }

  const warnings = [];
  results.forEach((r) => {
    const evalRes = evaluateAgainstGolden(r);
    if (evalRes.sigMatch === false) {
      warnings.push(`${r.id}: signature drift`);
    }
    if (evalRes.oomWarn) {
      warnings.push(`${r.id}: >${ORDER_MAG_FACTOR}x delta vs golden`);
    }
  });

  if (!VERIFY_ONLY && !SMOKE) {
    const lines = appendRows(results, abiInfo);
    console.log("WASM perf results (appended to perf_baselines.md):");
    console.log(lines);
  } else {
    console.log("VERIFY ONLY / SMOKE results:");
    results.forEach((r) => {
      const ev = evaluateAgainstGolden(r);
      console.log(
        `${r.id}: median ${r.median_us.toFixed(3)} µs, signature ${r.signature.slice(0, 12)}${ev.sigMatch === false ? " (DRIFT)" : ""}${ev.varianceWarn ? " (VARIANCE)" : ""}`,
      );
    });
  }

  if (warnings.length) {
    warnings.forEach((w) => console.warn("WARN:", w));
    if (VERIFY_ONLY || SMOKE) {
      process.exitCode = 1;
    }
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
