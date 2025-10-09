#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const TARGETS = new Map([
  ["animation", "build-animation-wasm.mjs"],
  ["graph", "build-graph-wasm.mjs"],
  ["orchestrator", "build-orchestrator-wasm.mjs"],
]);

function runBuild(target) {
  const script = TARGETS.get(target);
  if (!script) {
    throw new Error(
      `Unknown wasm build target "${target}". Expected one of: ${[
        ...TARGETS.keys(),
      ].join(", ")}.`,
    );
  }
  const scriptPath = resolve(__dirname, script);
  const result = spawnSync("node", [scriptPath], { stdio: "inherit" });
  if (result.status !== 0) {
    process.exitCode = result.status ?? 1;
    throw new Error(`Wasm build failed for target "${target}".`);
  }
}

function parseTargets(args) {
  const explicit = args.filter((arg) => !arg.startsWith("-"));
  if (explicit.length > 0) {
    return explicit;
  }
  const fromFlag = args
    .filter((arg) => arg.startsWith("--target="))
    .map((arg) => arg.slice("--target=".length));
  if (fromFlag.length > 0) {
    return fromFlag;
  }
  return [...TARGETS.keys()];
}

function main() {
  const [, , ...argv] = process.argv;
  const targets = parseTargets(argv);
  for (const target of targets) {
    runBuild(target);
  }
}

main();
