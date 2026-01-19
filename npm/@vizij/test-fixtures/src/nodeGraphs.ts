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

/** List all node-graph fixture names in the manifest. */
export function nodeGraphNames(): string[] {
  return Object.keys(manifest()["node-graphs"]);
}

/** Load a node-graph spec fixture as raw JSON text. */
export function nodeGraphSpecJson(name: string): string {
  return readFixture(entry(name).spec);
}

/** Load a node-graph spec fixture and parse it. */
export function nodeGraphSpec<T = unknown>(name: string): T {
  return loadFixture<T>(entry(name).spec);
}

/** Resolve a node-graph spec fixture path. */
export function nodeGraphSpecPath(name: string): string {
  return resolveFixturePath(entry(name).spec);
}

/** Load an optional node-graph stage fixture as raw JSON text. */
export function nodeGraphStageJson(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? readFixture(stage) : null;
}

/** Load an optional node-graph stage fixture and parse it. */
export function nodeGraphStage<T = unknown>(name: string): T | null {
  const stage = entry(name).stage;
  return stage ? loadFixture<T>(stage) : null;
}

/** Resolve an optional node-graph stage fixture path. */
export function nodeGraphStagePath(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? resolveFixturePath(stage) : null;
}
