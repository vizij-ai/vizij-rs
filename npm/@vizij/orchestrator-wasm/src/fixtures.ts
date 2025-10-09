import bundle from "./generated/fixtures-bundle.js";
import type { AnimationSetup, GraphRegistrationConfig, GraphSubscriptions } from "./types.js";

type FixturesManifest = typeof bundle.manifest;
type NodeGraphManifestEntry = {
  spec: string;
  stage?: string;
};
type OrchestrationManifestEntry = string | { path: string };

const manifest: FixturesManifest = bundle.manifest;
const files = bundle.files as Record<string, string>;

function animationsMap(): Record<string, string> {
  return (manifest.animations ?? {}) as Record<string, string>;
}

function nodeGraphMap(): Record<string, NodeGraphManifestEntry> {
  return (manifest["node-graphs"] ?? {}) as Record<string, NodeGraphManifestEntry>;
}

function orchestrationMap(): Record<string, OrchestrationManifestEntry> {
  return (manifest.orchestrations ?? {}) as Record<string, OrchestrationManifestEntry>;
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

function animationEntry(name: string): string {
  const rel = animationsMap()[name];
  if (!rel) {
    throw new Error(`Unknown animation fixture: ${name}`);
  }
  return rel;
}

function nodeGraphEntry(name: string): NodeGraphManifestEntry {
  const value = nodeGraphMap()[name];
  if (!value || typeof value !== "object" || typeof value.spec !== "string") {
    throw new Error(`Unknown node-graph fixture: ${name}`);
  }
  return value as NodeGraphManifestEntry;
}

function orchestrationEntry(name: string): OrchestrationManifestEntry {
  const value = orchestrationMap()[name];
  if (!value || (typeof value !== "string" && typeof value !== "object")) {
    throw new Error(`Unknown orchestration fixture: ${name}`);
  }
  return value as OrchestrationManifestEntry;
}

function orchestrationRelPath(name: string): string {
  const entry = orchestrationEntry(name);
  if (typeof entry === "string") {
    return entry;
  }
  if (entry && typeof entry === "object" && typeof entry.path === "string") {
    return entry.path;
  }
  throw new Error(`Orchestration entry '${name}' did not provide a descriptor path`);
}

export async function loadAnimationFixture(name: string): Promise<AnimationSetup["animation"]> {
  return loadFixture<AnimationSetup["animation"]>(animationEntry(name));
}

function asGraphSubscriptions(value: unknown): GraphSubscriptions | undefined {
  if (!value || typeof value !== "object") {
    return undefined;
  }
  const record = value as Record<string, unknown>;
  const inputs = Array.isArray(record.inputs) ? (record.inputs as string[]) : undefined;
  const outputs = Array.isArray(record.outputs) ? (record.outputs as string[]) : undefined;
  const mirrorWrites =
    typeof record.mirrorWrites === "boolean" ? (record.mirrorWrites as boolean) : undefined;

  if (!inputs && !outputs && typeof mirrorWrites === "undefined") {
    return undefined;
  }

  return {
    inputs,
    outputs,
    mirrorWrites,
  };
}

export async function loadNodeGraphConfig(name: string): Promise<GraphRegistrationConfig> {
  const raw = loadFixture<unknown>(nodeGraphEntry(name).spec);
  if (raw && typeof raw === "object") {
    const record = raw as Record<string, unknown>;

    if ("spec" in record && record.spec && typeof record.spec === "object") {
      const config: GraphRegistrationConfig = {
        spec: record.spec as GraphRegistrationConfig["spec"],
      };
      if (typeof record.id === "string" && record.id.length > 0) {
        config.id = record.id;
      }
      const subs = asGraphSubscriptions(record.subs);
      if (subs) {
        config.subs = subs;
      }
      return config;
    }

    if ("nodes" in record) {
      return {
        spec: record as GraphRegistrationConfig["spec"],
      };
    }
  }
  throw new Error(`Node-graph fixture '${name}' did not contain a GraphSpec-compatible payload`);
}

export async function loadNodeGraphSpec(name: string): Promise<GraphRegistrationConfig> {
  return loadNodeGraphConfig(name);
}

/** List orchestration fixture keys available via the embedded manifest. */
export async function listOrchestrationFixtures(): Promise<string[]> {
  return Object.keys(orchestrationMap());
}

/** Load the raw descriptor JSON value for the given orchestration fixture key. */
export async function loadOrchestrationDescriptor<T = unknown>(name: string): Promise<T> {
  return loadFixture<T>(orchestrationRelPath(name));
}

/** Load the orchestration descriptor as a JSON string. */
export async function loadOrchestrationJson(name: string): Promise<string> {
  return readFixture(orchestrationRelPath(name));
}

/**
 * Load a complete orchestration bundle containing the descriptor, resolved animation,
 * graph spec, and optional staged inputs. Useful for bootstrapping integration tests.
 */
type PipelineDescriptor = {
  animation: string;
  graph: string;
  initial_inputs?: Array<{ path: string; value: unknown }>;
  steps?: Array<{ delta: number; expect: Record<string, unknown> }>;
  [key: string]: unknown;
};

export async function loadOrchestrationBundle(
  name: string,
): Promise<{
  descriptor: PipelineDescriptor;
  animation: AnimationSetup["animation"];
  graphSpec: GraphRegistrationConfig;
}> {
  const descriptor = await loadOrchestrationDescriptor<PipelineDescriptor>(name);
  if (!descriptor || typeof descriptor !== "object") {
    throw new Error(`Orchestration descriptor '${name}' did not resolve to an object`);
  }

  if (typeof descriptor.animation !== "string") {
    throw new Error(`Orchestration '${name}' descriptor is missing animation reference`);
  }
  if (typeof descriptor.graph !== "string") {
    throw new Error(`Orchestration '${name}' descriptor is missing graph reference`);
  }

  const animation = await loadAnimationFixture(descriptor.animation);
  const graphConfig = await loadNodeGraphConfig(descriptor.graph);

  return {
    descriptor,
    animation,
    graphSpec: graphConfig,
  };
}
