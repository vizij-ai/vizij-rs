/**
 * Browser-bundle node-graph fixture helpers for `@vizij/test-fixtures`.
 */
import {
  nodeGraphEntry,
  loadFixture,
  manifest,
  readFixture,
  resolveFixturePath,
  type NodeGraphManifestEntry,
} from "./shared.browser.js";

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

export function nodeGraphStageJson(_name: string): string | null {
  return null;
}

export function nodeGraphStage<T = unknown>(_name: string): T | null {
  return null;
}

export function nodeGraphStagePath(_name: string): string | null {
  return null;
}
