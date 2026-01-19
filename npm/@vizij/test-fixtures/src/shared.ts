import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Manifest entry for a node-graph fixture (spec + optional stage path). */
export interface NodeGraphManifestEntry {
  spec: string;
  stage?: string;
}

/** Manifest entry for orchestration fixtures. */
export type OrchestrationManifestEntry = string | { path: string };

/** Contents of fixtures/manifest.json. */
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

/**
 * Absolute path to the fixtures directory.
 *
 * @throws If the package cannot locate `fixtures/manifest.json` relative to
 *   the installed module location.
 * @example
 * const root = fixturesRoot();
 */
export function fixturesRoot(): string {
  return locateFixturesRoot();
}

/**
 * Parsed fixtures manifest (cached after first load).
 *
 * @throws When the manifest cannot be read or parsed.
 * @example
 * const data = manifest();
 */
export function manifest(): FixturesManifest {
  if (!manifestCache) {
    const raw = readFileSync(resolve(locateFixturesRoot(), "manifest.json"), "utf8");
    manifestCache = JSON.parse(raw) as FixturesManifest;
  }
  return manifestCache;
}

/**
 * Resolve a fixtures-relative path to an absolute path.
 *
 * This does not validate that the path exists on disk.
 * @example
 * const abs = resolveFixturePath("animations/simple-walk.json");
 */
export function resolveFixturePath(relPath: string): string {
  return resolve(locateFixturesRoot(), relPath);
}

/**
 * Read fixture JSON from disk as a string.
 *
 * @throws When the fixture file is missing.
 * @example
 * const raw = readFixture("animations/simple-walk.json");
 */
export function readFixture(relPath: string): string {
  return readFileSync(resolveFixturePath(relPath), "utf8");
}

/**
 * Load fixture JSON from disk and parse it.
 *
 * @throws When the fixture file is missing or contains invalid JSON.
 * @example
 * const payload = loadFixture("animations/simple-walk.json");
 */
export function loadFixture<T>(relPath: string): T {
  return JSON.parse(readFixture(relPath)) as T;
}

/**
 * Resolve a named animation fixture to its manifest path.
 *
 * @throws If the animation fixture key is missing.
 * @example
 * const relPath = animationEntry("simple-walk");
 */
export function animationEntry(name: string): string {
  const entry = manifest().animations[name];
  if (!entry) {
    throw new Error(`Unknown animation fixture: ${name}`);
  }
  return entry;
}

/**
 * Resolve a named node-graph fixture to its manifest entry.
 *
 * @throws If the node-graph fixture key is missing.
 * @example
 * const entry = nodeGraphEntry("oscillator-basics");
 */
export function nodeGraphEntry(name: string): NodeGraphManifestEntry {
  const entry = manifest()["node-graphs"][name];
  if (!entry) {
    throw new Error(`Unknown node-graph fixture: ${name}`);
  }
  return entry;
}

/**
 * Resolve a named orchestration fixture to its manifest entry.
 *
 * @throws If the orchestration fixture key is missing.
 * @example
 * const entry = orchestrationEntry("simple-orchestration");
 */
export function orchestrationEntry(name: string): OrchestrationManifestEntry {
  const entry = manifest().orchestrations[name];
  if (!entry) {
    throw new Error(`Unknown orchestration fixture: ${name}`);
  }
  return entry;
}

/**
 * Resolve an orchestration manifest entry into an absolute path.
 *
 * @example
 * const abs = orchestrationPath(orchestrationEntry("simple-orchestration"));
 */
export function orchestrationPath(entry: OrchestrationManifestEntry): string {
  if (typeof entry === "string") {
    return resolveFixturePath(entry);
  }
  return resolveFixturePath(entry.path);
}
