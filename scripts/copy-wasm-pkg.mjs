#!/usr/bin/env node

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

async function copyPkg(packageName) {
  const packageRoot = path.join(repoRoot, "npm", "@vizij", packageName);
  const pkgSrc = path.join(packageRoot, "pkg");
  const pkgDest = path.join(packageRoot, "dist", "pkg");

  await fs.rm(pkgDest, { recursive: true, force: true }).catch(() => {});
  await fs.cp(pkgSrc, pkgDest, { recursive: true });
}

Promise.all(packages.map(copyPkg)).catch((err) => {
  console.error("[copy-wasm-pkg] Failed to copy pkg directory:", err);
  process.exit(1);
});
