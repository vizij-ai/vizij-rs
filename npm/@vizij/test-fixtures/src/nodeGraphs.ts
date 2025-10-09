import {
  nodeGraphEntry,
  loadFixture,
  manifest,
  readFixture,
  resolveFixturePath,
  type NodeGraphManifestEntry,
} from "./shared.js";

export interface NodeGraphSpecFixture<TSpec = unknown, TStage = unknown> {
  spec: TSpec;
  stage?: TStage | null;
}

function entry(name: string): NodeGraphManifestEntry {
  return nodeGraphEntry(name);
}

export function nodeGraphNames(): string[] {
  return Object.keys(manifest()["node-graphs"]);
}

export function nodeGraphSpecJson(name: string): string {
  return readFixture(entry(name).spec);
}

export function nodeGraphSpec<T = unknown>(name: string): T {
  return loadFixture<T>(entry(name).spec);
}

export function nodeGraphSpecPath(name: string): string {
  return resolveFixturePath(entry(name).spec);
}

export function nodeGraphStageJson(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? readFixture(stage) : null;
}

export function nodeGraphStage<T = unknown>(name: string): T | null {
  const stage = entry(name).stage;
  return stage ? loadFixture<T>(stage) : null;
}

export function nodeGraphStagePath(name: string): string | null {
  const stage = entry(name).stage;
  return stage ? resolveFixturePath(stage) : null;
}
