#!/usr/bin/env node

import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, "..");

const TRACKED_FILES = [
  "npm/@vizij/node-graph-wasm/src/metadata/registry.json",
  "npm/@vizij/node-graph-wasm/src/metadata/registry.ts",
  "npm/@vizij/node-graph-wasm/dist/src/metadata/registry.json",
];

async function run(command, args, options = {}) {
  await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(command, args, {
      cwd: repoRoot,
      stdio: "inherit",
      env: process.env,
      ...options,
    });
    child.on("exit", (code) => {
      if (code === 0) {
        resolvePromise();
      } else {
        rejectPromise(
          new Error(`${command} ${args.join(" ")} exited with code ${code}`),
        );
      }
    });
    child.on("error", rejectPromise);
  });
}

async function hasRegistryDiff(): Promise<boolean> {
  return await new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(
      "git",
      ["diff", "--quiet", "--", ...TRACKED_FILES],
      {
        cwd: repoRoot,
        stdio: "ignore",
      },
    );
    child.on("exit", (code) => {
      if (code === 0) {
        resolvePromise(false);
      } else if (code === 1) {
        resolvePromise(true);
      } else {
        rejectPromise(
          new Error(`git diff exited with unexpected code ${code}`),
        );
      }
    });
    child.on("error", rejectPromise);
  });
}

async function main() {
  console.log("[verify-node-registry] regenerating registry artifacts…");
  await run("node", ["scripts/generate-node-registry.mjs"]);

  const dirty = await hasRegistryDiff();
  if (!dirty) {
    console.log("[verify-node-registry] registry artifacts are up to date.");
    return;
  }

  console.error(
    "\n[verify-node-registry] registry artifacts are out of date. Run:",
  );
  console.error("  node scripts/generate-node-registry.mjs");
  console.error("and commit the resulting changes.\n");
  await run("git", ["--no-pager", "diff", "--", ...TRACKED_FILES], {
    stdio: "inherit",
  });
  process.exitCode = 1;
}

main().catch((error) => {
  console.error("[verify-node-registry] Failed:", error);
  process.exit(1);
});
