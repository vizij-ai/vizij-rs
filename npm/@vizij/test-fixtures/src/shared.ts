import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export interface NodeGraphManifestEntry {
  spec: string;
  stage?: string;
}

export type OrchestrationManifestEntry = string | { path: string };

export interface FixturesManifest {
  animations: Record<string, string>;
  "node-graphs": Record<string, NodeGraphManifestEntry>;
  orchestrations: Record<string, OrchestrationManifestEntry>;
}

let fixturesRootCache: string | null = null;
let manifestCache: FixturesManifest | null = null;

function locateFixturesRoot(): string {
  if (fixturesRootCache) {
    return fixturesRootCache;
  }

  let current = dirname(fileURLToPath(import.meta.url));
  for (let i = 0; i < 10; i += 1) {
    const candidate = resolve(current, "fixtures/manifest.json");
    if (existsSync(candidate)) {
      const root = resolve(current, "fixtures");
      fixturesRootCache = root;
      return root;
    }
    const parent = resolve(current, "..");
    if (parent === current) {
      break;
    }
    current = parent;
  }
  throw new Error("Unable to locate fixtures/manifest.json relative to @vizij/test-fixtures");
}

export function fixturesRoot(): string {
  return locateFixturesRoot();
}

export function manifest(): FixturesManifest {
  if (!manifestCache) {
    const raw = readFileSync(resolve(locateFixturesRoot(), "manifest.json"), "utf8");
    manifestCache = JSON.parse(raw) as FixturesManifest;
  }
  return manifestCache;
}

export function resolveFixturePath(relPath: string): string {
  return resolve(locateFixturesRoot(), relPath);
}

export function readFixture(relPath: string): string {
  return readFileSync(resolveFixturePath(relPath), "utf8");
}

export function loadFixture<T>(relPath: string): T {
  return JSON.parse(readFixture(relPath)) as T;
}

export function animationEntry(name: string): string {
  const entry = manifest().animations[name];
  if (!entry) {
    throw new Error(`Unknown animation fixture: ${name}`);
  }
  return entry;
}

export function nodeGraphEntry(name: string): NodeGraphManifestEntry {
  const entry = manifest()["node-graphs"][name];
  if (!entry) {
    throw new Error(`Unknown node-graph fixture: ${name}`);
  }
  return entry;
}

export function orchestrationEntry(name: string): OrchestrationManifestEntry {
  const entry = manifest().orchestrations[name];
  if (!entry) {
    throw new Error(`Unknown orchestration fixture: ${name}`);
  }
  return entry;
}

export function orchestrationPath(entry: OrchestrationManifestEntry): string {
  if (typeof entry === "string") {
    return resolveFixturePath(entry);
  }
  return resolveFixturePath(entry.path);
}
