#!/usr/bin/env node
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const packageRoot = path.resolve(__dirname, "..");
const fixturesRoot = path.resolve(packageRoot, "../../..", "fixtures");
const manifestPath = path.join(fixturesRoot, "manifest.json");
const outputDir = path.join(packageRoot, "src", "generated");
const outputPath = path.join(outputDir, "browser-fixtures.json");

async function main() {
  const manifestRaw = await fs.readFile(manifestPath, "utf8");
  const manifest = JSON.parse(manifestRaw);

  const paths = new Set();

  for (const rel of Object.values(manifest.animations ?? {})) {
    paths.add(rel);
  }

  for (const entry of Object.values(manifest["node-graphs"] ?? {})) {
    if (entry?.spec) paths.add(entry.spec);
    if (entry?.stage) paths.add(entry.stage);
  }

  for (const entry of Object.values(manifest.orchestrations ?? {})) {
    if (typeof entry === "string") {
      paths.add(entry);
    } else if (entry && typeof entry === "object" && entry.path) {
      paths.add(entry.path);
    }
  }

  const files = {};
  for (const relPath of paths) {
    if (!relPath) continue;
    const normalized = relPath.replace(/^[.][/\\]/, "").replace(/\\/g, "/");
    const absolute = path.join(fixturesRoot, normalized);
    const raw = await fs.readFile(absolute, "utf8");
    files[normalized] = raw;
  }

  await fs.mkdir(outputDir, { recursive: true });
  const payload = {
    manifest,
    files,
  };
  await fs.writeFile(outputPath, JSON.stringify(payload, null, 2) + "\n", "utf8");
}

main().catch((err) => {
  console.error("[generate-browser-fixtures] Failed to materialize fixtures:", err);
  process.exit(1);
});
