import {
  nodeGraphEntry,
  loadFixture,
  manifest,
  readFixture,
  resolveFixturePath,
  type NodeGraphManifestEntry,
} from "./shared.js";

/** Parsed node graph fixture bundle. */
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
 * @example
 * nodeGraphNames(); // ["oscillator-basics", "vector-playground", ...]
 */
export function nodeGraphNames(): string[] {
  return Object.keys(manifest()["node-graphs"]);
}

/**
 * Load a node-graph spec fixture as raw JSON text.
 *
 * @throws If the name is not present in the manifest or the file is missing.
 * @example
 * const json = nodeGraphSpecJson("oscillator-basics");
 */
export function nodeGraphSpecJson(name: string): string {
  return readFixture(entry(name).spec);
}

/**
 * Load a node-graph spec fixture and parse it.
 *
 * @throws If the name is not present in the manifest or the JSON is invalid.
 * @example
 * const spec = nodeGraphSpec("oscillator-basics");
 */
export function nodeGraphSpec<T = unknown>(name: string): T {
  return loadFixture<T>(entry(name).spec);
}

/**
 * Resolve a node-graph spec fixture path.
 *
 * @throws If the name is not present in the manifest.
 * @example
 * const path = nodeGraphSpecPath("oscillator-basics");
 */
export function nodeGraphSpecPath(name: string): string {
  return resolveFixturePath(entry(name).spec);
}

/**
 * Load an optional node-graph stage fixture as raw JSON text.
 *
 * Returns null when the fixture does not define a stage entry.
 * @example
 * const json = nodeGraphStageJson("oscillator-basics");
 */
export function nodeGraphStageJson(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? readFixture(stage) : null;
}

/**
 * Load an optional node-graph stage fixture and parse it.
 *
 * Returns null when the fixture does not define a stage entry.
 * @example
 * const stage = nodeGraphStage("oscillator-basics");
 */
export function nodeGraphStage<T = unknown>(name: string): T | null {
  const stage = entry(name).stage;
  return stage ? loadFixture<T>(stage) : null;
}

/**
 * Resolve an optional node-graph stage fixture path.
 *
 * Returns null when the fixture does not define a stage entry.
 * @example
 * const path = nodeGraphStagePath("oscillator-basics");
 */
export function nodeGraphStagePath(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? resolveFixturePath(stage) : null;
}
