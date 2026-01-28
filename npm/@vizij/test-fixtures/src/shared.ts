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
 * @returns Absolute path to the fixtures root.
 * @throws If the package cannot locate `fixtures/manifest.json` relative to
 *   the installed module location.
 */
export function fixturesRoot(): string {
  return locateFixturesRoot();
}

/**
 * Parsed fixtures manifest (cached after first load).
 *
 * @returns Parsed manifest JSON.
 * @throws Error when the manifest cannot be read or parsed.
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
 *
 * @param relPath - Path relative to the fixtures root.
 * @returns Absolute path to the fixture.
 */
export function resolveFixturePath(relPath: string): string {
  return resolve(locateFixturesRoot(), relPath);
}

/**
 * Read fixture JSON from disk as a string.
 *
 * @param relPath - Path relative to the fixtures root.
 * @returns Raw JSON string.
 * @throws Error when the fixture file is missing.
 */
export function readFixture(relPath: string): string {
  return readFileSync(resolveFixturePath(relPath), "utf8");
}

/**
 * Load fixture JSON from disk and parse it.
 *
 * @param relPath - Path relative to the fixtures root.
 * @returns Parsed JSON payload.
 * @throws Error when the fixture file is missing or contains invalid JSON.
 */
export function loadFixture<T>(relPath: string): T {
  return JSON.parse(readFixture(relPath)) as T;
}

/**
 * Resolve a named animation fixture to its manifest path.
 *
 * @param name - Fixture key from the manifest.
 * @returns Manifest path relative to the fixtures root.
 * @throws Error if the animation fixture key is missing.
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
 * @param name - Fixture key from the manifest.
 * @returns Manifest entry for the graph (spec + optional stage).
 * @throws Error if the node-graph fixture key is missing.
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
 * @param name - Fixture key from the manifest.
 * @returns Manifest entry for the orchestration.
 * @throws Error if the orchestration fixture key is missing.
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
 * @param entry - Manifest entry string or { path } object.
 * @returns Absolute path to the orchestration descriptor.
 */
export function orchestrationPath(entry: OrchestrationManifestEntry): string {
  if (typeof entry === "string") {
    return resolveFixturePath(entry);
  }
  return resolveFixturePath(entry.path);
}
