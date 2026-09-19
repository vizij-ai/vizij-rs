#!/usr/bin/env node

import { spawn } from "node:child_process";
import { promises as fs } from "node:fs";
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
  runtime: "runtime",
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
      // A prerelease (3.0.0-alpha.0) publishes under its own dist-tag, so
      // `latest` keeps pointing at the last release.
      const { version } = JSON.parse(
        await fs.readFile(`npm/@vizij/${only}/package.json`, "utf8"),
      );
      const prerelease = /^\d+\.\d+\.\d+-([a-z]+)/i.exec(version);
      await run("pnpm", [
        "--filter",
        `@vizij/${only}`,
        "publish",
        "--access",
        "public",
        "--no-git-checks",
        ...(prerelease ? ["--tag", prerelease[1]] : []),
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
