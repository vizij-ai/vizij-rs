import type {
  FixturesManifest,
  NodeGraphManifestEntry,
  OrchestrationManifestEntry,
} from "./shared.js";
import bundle from "./generated/browser-fixtures.json";

interface BrowserFixturesBundle {
  manifest: FixturesManifest;
  files: Record<string, string>;
}

const data = bundle as BrowserFixturesBundle;
const manifestCache = data.manifest;

function normalizeRelPath(relPath: string): string {
  return relPath.replace(/^[.][/\\]/, "").replace(/\\/g, "/");
}

function bundledFixture(relPath: string): string {
  const key = normalizeRelPath(relPath);
  const raw = data.files[key];
  if (typeof raw !== "string") {
    throw new Error(`Fixture '${relPath}' was not included in the browser fixtures bundle`);
  }
  return raw;
}

export type { FixturesManifest, NodeGraphManifestEntry, OrchestrationManifestEntry };

/** Bundled root used for browser fixture paths. */
export function fixturesRoot(): string {
  return "fixtures";
}

/** Cached fixture manifest from the embedded bundle. */
export function manifest(): FixturesManifest {
  return manifestCache;
}

/** Resolve a fixtures-relative path to a bundled "fixtures/..." path. */
export function resolveFixturePath(relPath: string): string {
  const normalized = normalizeRelPath(relPath);
  return `fixtures/${normalized}`;
}

/**
 * Read a fixture JSON file from the embedded bundle.
 *
 * @throws If the fixture path was not bundled.
 */
export function readFixture(relPath: string): string {
  return bundledFixture(relPath);
}

/**
 * Load a fixture JSON file from the embedded bundle and parse it.
 *
 * @throws If the fixture path was not bundled or JSON parsing fails.
 */
export function loadFixture<T>(relPath: string): T {
  return JSON.parse(bundledFixture(relPath)) as T;
}

/**
 * Resolve a named animation fixture to its manifest path.
 *
 * @throws If the animation fixture key is missing.
 */
export function animationEntry(name: string): string {
  const entry = manifestCache.animations[name];
  if (!entry) {
    throw new Error(`Unknown animation fixture: ${name}`);
  }
  return entry;
}

/**
 * Resolve a named node-graph fixture to its manifest entry.
 *
 * @throws If the node-graph fixture key is missing.
 */
export function nodeGraphEntry(name: string): NodeGraphManifestEntry {
  const entry = manifestCache["node-graphs"][name];
  if (!entry) {
    throw new Error(`Unknown node-graph fixture: ${name}`);
  }
  return entry;
}

/**
 * Resolve a named orchestration fixture to its manifest entry.
 *
 * @throws If the orchestration fixture key is missing.
 */
export function orchestrationEntry(name: string): OrchestrationManifestEntry {
  const entry = manifestCache.orchestrations[name];
  if (!entry) {
    throw new Error(`Unknown orchestration fixture: ${name}`);
  }
  return entry;
}

/** Resolve an orchestration manifest entry into a bundled "fixtures/..." path. */
export function orchestrationPath(entry: OrchestrationManifestEntry): string {
  if (typeof entry === "string") {
    return resolveFixturePath(entry);
  }
  return resolveFixturePath(entry.path);
}
