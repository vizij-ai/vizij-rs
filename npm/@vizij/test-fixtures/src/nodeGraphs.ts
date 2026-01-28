import {
  nodeGraphEntry,
  loadFixture,
  manifest,
  readFixture,
  resolveFixturePath,
  type NodeGraphManifestEntry,
} from "./shared.js";

/**
 * Parsed node graph fixture bundle.
 *
 * @typeParam TSpec - Graph spec payload type.
 * @typeParam TStage - Optional stage payload type.
 */
export interface NodeGraphSpecFixture<TSpec = unknown, TStage = unknown> {
  spec: TSpec;
  stage?: TStage | null;
}

function entry(name: string): NodeGraphManifestEntry {
  return nodeGraphEntry(name);
}

/**
 * List all node-graph fixture names in the manifest.
 *
 * @returns Array of fixture keys.
 */
export function nodeGraphNames(): string[] {
  return Object.keys(manifest()["node-graphs"]);
}

/**
 * Load a node-graph spec fixture as raw JSON text.
 *
 * @param name - Fixture key from the manifest.
 * @returns Raw JSON string for the graph spec.
 * @throws If the name is not present in the manifest or the file is missing.
 */
export function nodeGraphSpecJson(name: string): string {
  return readFixture(entry(name).spec);
}

/**
 * Load a node-graph spec fixture and parse it.
 *
 * @param name - Fixture key from the manifest.
 * @returns Parsed graph spec.
 * @throws If the name is not present in the manifest or the JSON is invalid.
 */
export function nodeGraphSpec<T = unknown>(name: string): T {
  return loadFixture<T>(entry(name).spec);
}

/**
 * Resolve a node-graph spec fixture path.
 *
 * @param name - Fixture key from the manifest.
 * @returns Absolute path to the graph spec fixture.
 * @throws If the name is not present in the manifest.
 */
export function nodeGraphSpecPath(name: string): string {
  return resolveFixturePath(entry(name).spec);
}

/**
 * Load an optional node-graph stage fixture as raw JSON text.
 *
 * @param name - Fixture key from the manifest.
 * @returns Raw JSON string or `null` when no stage entry exists.
 */
export function nodeGraphStageJson(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? readFixture(stage) : null;
}

/**
 * Load an optional node-graph stage fixture and parse it.
 *
 * @param name - Fixture key from the manifest.
 * @returns Parsed stage payload or `null` when no stage entry exists.
 */
export function nodeGraphStage<T = unknown>(name: string): T | null {
  const stage = entry(name).stage;
  return stage ? loadFixture<T>(stage) : null;
}

/**
 * Resolve an optional node-graph stage fixture path.
 *
 * @param name - Fixture key from the manifest.
 * @returns Absolute path or `null` when no stage entry exists.
 */
export function nodeGraphStagePath(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? resolveFixturePath(stage) : null;
}
