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

export function fixturesRoot(): string {
  return "fixtures";
}

export function manifest(): FixturesManifest {
  return manifestCache;
}

export function resolveFixturePath(relPath: string): string {
  const normalized = normalizeRelPath(relPath);
  return `fixtures/${normalized}`;
}

export function readFixture(relPath: string): string {
  return bundledFixture(relPath);
}

export function loadFixture<T>(relPath: string): T {
  return JSON.parse(bundledFixture(relPath)) as T;
}

export function animationEntry(name: string): string {
  const entry = manifestCache.animations[name];
  if (!entry) {
    throw new Error(`Unknown animation fixture: ${name}`);
  }
  return entry;
}

export function nodeGraphEntry(name: string): NodeGraphManifestEntry {
  const entry = manifestCache["node-graphs"][name];
  if (!entry) {
    throw new Error(`Unknown node-graph fixture: ${name}`);
  }
  return entry;
}

export function orchestrationEntry(name: string): OrchestrationManifestEntry {
  const entry = manifestCache.orchestrations[name];
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
