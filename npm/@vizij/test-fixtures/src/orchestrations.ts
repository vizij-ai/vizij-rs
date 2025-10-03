import {
  orchestrationEntry,
  orchestrationPath,
  loadFixture,
  manifest,
  readFixture,
} from "./shared.js";
import { animationFixture } from "./animations.js";
import { nodeGraphSpec, nodeGraphStage } from "./nodeGraphs.js";

export interface PipelineDescriptor {
  description?: string;
  animation: string;
  graph: string;
  initial_inputs?: Array<{ path: string; value: unknown }>;
  steps?: Array<{ delta: number; expect: Record<string, unknown> }>;
  [key: string]: unknown;
}

export interface OrchestrationBundle<
  TDescriptor extends PipelineDescriptor = PipelineDescriptor,
  TAnimation = unknown,
  TGraphSpec = unknown,
  TGraphStage = unknown,
> {
  descriptor: TDescriptor;
  animation: TAnimation;
  graphSpec: TGraphSpec;
  graphStage?: TGraphStage | null;
}

export function orchestrationNames(): string[] {
  return Object.keys(manifest().orchestrations);
}

export function orchestrationJson(name: string): string {
  const entry = orchestrationEntry(name);
  if (typeof entry === "string") {
    return readFixture(entry);
  }
  return readFixture(entry.path);
}

export function orchestrationDescriptor<T = unknown>(name: string): T {
  const entry = orchestrationEntry(name);
  const rel = typeof entry === "string" ? entry : entry.path;
  return loadFixture<T>(rel);
}

export function orchestrationDescriptorPath(name: string): string {
  return orchestrationPath(orchestrationEntry(name));
}

export function loadOrchestrationBundle(
  name: string,
): OrchestrationBundle<PipelineDescriptor> {
  const descriptor = orchestrationDescriptor<PipelineDescriptor>(name);
  if (!descriptor || typeof descriptor !== "object") {
    throw new Error(`Orchestration descriptor '${name}' did not resolve to an object`);
  }
  const animation = animationFixture(descriptor.animation) as Record<string, unknown>;
  const graphSpec = nodeGraphSpec(descriptor.graph) as Record<string, unknown>;
  const graphStage = nodeGraphStage(descriptor.graph);
  return {
    descriptor,
    animation,
    graphSpec,
    graphStage,
  };
}
