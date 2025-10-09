import bundle from "./generated/fixtures-bundle.js";
import type { GraphSpec } from "./types.js";

type FixturesManifest = typeof bundle.manifest;
type NodeGraphManifestEntry = {
  spec: string;
  stage?: string;
};

const manifest: FixturesManifest = bundle.manifest;
const files = bundle.files as Record<string, string>;

function nodeGraphMap(): Record<string, NodeGraphManifestEntry> {
  return (manifest["node-graphs"] ?? {}) as Record<string, NodeGraphManifestEntry>;
}

function normalizeRelPath(relPath: string): string {
  return relPath.replace(/^[.][/\\]/, "").replace(/\\/g, "/");
}

function readFixture(relPath: string): string {
  const normalized = normalizeRelPath(relPath);
  const raw = files[normalized];
  if (typeof raw !== "string") {
    throw new Error(`Fixture '${relPath}' was not embedded in this build`);
  }
  return raw;
}

function loadFixture<T>(relPath: string): T {
  return JSON.parse(readFixture(relPath)) as T;
}

function entry(name: string): NodeGraphManifestEntry {
  const value = nodeGraphMap()[name];
  if (!value || typeof value !== "object" || typeof value.spec !== "string") {
    throw new Error(`Unknown node-graph fixture: ${name}`);
  }
  return value as NodeGraphManifestEntry;
}

/** List the node-graph fixture keys available via the embedded manifest. */
export async function listNodeGraphFixtures(): Promise<string[]> {
  return Object.keys(nodeGraphMap());
}

/** Load the GraphSpec for the given shared fixture key. */
export async function loadNodeGraphSpec(name: string): Promise<GraphSpec> {
  const raw = loadFixture<unknown>(entry(name).spec);
  if (raw && typeof raw === "object") {
    const record = raw as Record<string, unknown>;
    if ("nodes" in record) {
      return raw as GraphSpec;
    }
    if ("spec" in record && record.spec && typeof record.spec === "object") {
      return record.spec as GraphSpec;
    }
  }
  throw new Error(`Fixture '${name}' did not contain a GraphSpec-compatible payload`);
}

/** Load the GraphSpec JSON string for the given shared fixture key. */
export async function loadNodeGraphSpecJson(name: string): Promise<string> {
  return readFixture(entry(name).spec);
}

/** Load any staged input bundle associated with the node-graph fixture. */
export async function loadNodeGraphStage<T = unknown>(name: string): Promise<T | null> {
  const stagePath = entry(name).stage;
  if (!stagePath) return null;
  return loadFixture<T>(stagePath);
}

/**
 * Load both the GraphSpec and optional stage inputs for a shared node-graph fixture.
 * Returns `{ spec }`; stage data is no longer bundled with fixtures and should be provided per usage site.
 */
export async function loadNodeGraphBundle(name: string): Promise<{ spec: GraphSpec }> {
  const spec = await loadNodeGraphSpec(name);
  return { spec };
}
