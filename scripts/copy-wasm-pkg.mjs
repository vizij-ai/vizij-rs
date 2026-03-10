#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..");

const packages = process.argv.slice(2);

if (packages.length === 0) {
  console.error("Usage: node scripts/copy-wasm-pkg.mjs <package> [package...]");
  process.exit(1);
}

const bootstrapScripts = {
  "animation-wasm": "build:wasm:animation",
  "node-graph-wasm": "build:wasm:graph",
  "orchestrator-wasm": "build:wasm:orchestrator",
};

async function ensurePkgExists(packageName, pkgSrc) {
  try {
    await fs.access(pkgSrc);
    return;
  } catch {
    const bootstrapScript = bootstrapScripts[packageName];
    if (!bootstrapScript) {
      throw new Error(
        `Missing pkg/ for ${packageName}, and no bootstrap script is registered for it.`,
      );
    }

    console.warn(
      `[copy-wasm-pkg] Missing ${path.relative(repoRoot, pkgSrc)}. Running \`pnpm run ${bootstrapScript}\` first.`,
    );
    execFileSync("pnpm", ["run", bootstrapScript], {
      cwd: repoRoot,
      stdio: "inherit",
    });

    await fs.access(pkgSrc);
  }
}

async function copyPkg(packageName) {
  const packageRoot = path.join(repoRoot, "npm", "@vizij", packageName);
  const pkgSrc = path.join(packageRoot, "pkg");
  const pkgDest = path.join(packageRoot, "dist", "pkg");

  await ensurePkgExists(packageName, pkgSrc);
  await fs.rm(pkgDest, { recursive: true, force: true }).catch(() => {});
  await fs.cp(pkgSrc, pkgDest, { recursive: true });
}

Promise.all(packages.map(copyPkg)).catch((err) => {
  console.error("[copy-wasm-pkg] Failed to copy pkg directory:", err);
  process.exit(1);
});
