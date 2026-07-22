#!/usr/bin/env node

import { spawn } from "node:child_process";
import { applyWorkspaceManifestUpdates, restoreWorkspaceManifests } from "./prepare-publish-manifests.mjs";

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit" });
    child.on("close", (code) => {
      if (code === 0) {
        resolve();
      } else {
        reject(new Error(`Command "${command} ${args.join(" ")}" exited with ${code}`));
      }
    });
    child.on("error", reject);
  });
}

/**
 * Wasm build target for each wasm wrapper package, for selective publishes.
 * Non-wasm packages (value-json, wasm-loader, …) need no wasm build step.
 */
const WASM_TARGETS = {
  animation: "animation",
  "node-graph": "graph",
  "orchestrator-wasm": "orchestrator",
  runtime: "arora-web",
};

async function main() {
  // PUBLISH_PACKAGE (a name under @vizij/, e.g. "runtime") publishes
  // that one package: build only what it needs, then `pnpm publish` it.
  // Without it, the full pipeline runs and `changeset publish` releases every
  // package whose version is not on the registry yet.
  const only = process.env.PUBLISH_PACKAGE?.trim();
  await applyWorkspaceManifestUpdates();
  try {
    if (only) {
      const target = WASM_TARGETS[only];
      if (target) {
        await run("pnpm", ["run", "build:wasm", "--", target]);
      }
      await run("pnpm", ["run", "build:shared"]);
      await run("pnpm", ["--filter", `@vizij/${only}`, "run", "build"]);
      await run("pnpm", [
        "--filter",
        `@vizij/${only}`,
        "publish",
        "--access",
        "public",
        "--no-git-checks",
      ]);
    } else {
      await run("pnpm", ["run", "build:wasm"]);
      await run("pnpm", ["run", "build:shared"]);
      await run("pnpm", ["changeset", "publish"]);
    }
  } finally {
    await restoreWorkspaceManifests();
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
