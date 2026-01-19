import {
  nodeGraphEntry,
  loadFixture,
  manifest,
  readFixture,
  resolveFixturePath,
  type NodeGraphManifestEntry,
} from "./shared.browser.js";

/** Parsed node-graph fixture bundle. */
export interface NodeGraphSpecFixture<TSpec = unknown, TStage = unknown> {
  spec: TSpec;
  stage?: TStage | null;
}

function entry(name: string): NodeGraphManifestEntry {
  return nodeGraphEntry(name);
}

/**
 * List all node-graph fixture names in the bundled manifest.
 *
 * @example
 * nodeGraphNames(); // ["oscillator-basics", "vector-playground", ...]
 */
export function nodeGraphNames(): string[] {
  return Object.keys(manifest()["node-graphs"]);
}

/**
 * Load a node-graph spec fixture as raw JSON text from the bundle.
 *
 * @throws If the fixture key is missing from the manifest.
 * @example
 * const json = nodeGraphSpecJson("oscillator-basics");
 */
export function nodeGraphSpecJson(name: string): string {
  return readFixture(entry(name).spec);
}

/**
 * Load a node-graph spec fixture and parse it as JSON.
 *
 * @throws If the fixture key is missing or the JSON is invalid.
 * @example
 * const spec = nodeGraphSpec("oscillator-basics");
 */
export function nodeGraphSpec<T = unknown>(name: string): T {
  return loadFixture<T>(entry(name).spec);
}

/**
 * Resolve a node-graph spec fixture name to a bundled "fixtures/..." path.
 *
 * @throws If the fixture key is missing from the manifest.
 * @example
 * const path = nodeGraphSpecPath("oscillator-basics");
 */
export function nodeGraphSpecPath(name: string): string {
  return resolveFixturePath(entry(name).spec);
}

/**
 * Node-graph stage fixtures are not bundled in the browser build.
 *
 * Always returns null.
 * @example
 * const stageJson = nodeGraphStageJson("oscillator-basics"); // null
 */
export function nodeGraphStageJson(_name: string): string | null {
  return null;
}

/**
 * Node-graph stage fixtures are not bundled in the browser build.
 *
 * Always returns null.
 * @example
 * const stage = nodeGraphStage("oscillator-basics"); // null
 */
export function nodeGraphStage<T = unknown>(_name: string): T | null {
  return null;
}

/**
 * Node-graph stage fixtures are not bundled in the browser build.
 *
 * Always returns null.
 * @example
 * const stagePath = nodeGraphStagePath("oscillator-basics"); // null
 */
export function nodeGraphStagePath(_name: string): string | null {
  return null;
}
