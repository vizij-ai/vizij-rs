import bundle from "./generated/fixtures-bundle.js";
import type { AnimationSetup, GraphRegistrationConfig, GraphSubscriptions } from "./types.js";

type FixturesManifest = typeof bundle.manifest;
type NodeGraphManifestEntry = {
  spec: string;
  stage?: string;
};
type OrchestrationManifestEntry = string | { path: string };

interface StageEntry {
  path: string;
  value: unknown;
  shape?: unknown;
}

type GraphSeed =
  | string
  | {
      fixture: string;
      id?: string;
      subs?: GraphSubscriptions;
      mirrorWrites?: boolean;
      stage?: StageEntry[];
    };

type AnimationSeed =
  | string
  | {
      fixture: string;
      id?: string;
      setup?: AnimationSetup;
      player?: AnimationSetup["player"];
      instance?: AnimationSetup["instance"];
    };

interface MergeStrategySeed {
  outputs?: string;
  intermediate?: string;
}

interface MergedGraphSeed {
  id: string;
  graphs: GraphSeed[];
  strategy?: MergeStrategySeed;
}

interface PipelineDescriptor {
  description?: string;
  schedule?: string;
  animations?: AnimationSeed[];
  graphs?: GraphSeed[];
  mergedGraphs?: MergedGraphSeed[];
  initial_inputs?: StageEntry[];
  steps?: Array<{ delta: number; expect: Record<string, unknown> }>;
  [key: string]: unknown;
}

type GraphBinding = {
  key: string;
  id?: string;
  config: GraphRegistrationConfig;
  mirrorWrites: boolean;
  stage: StageEntry[];
};

type MergeStrategy = "error" | "namespace" | "blend" | "add" | "default-blend";

type MergedGraphBinding = {
  id: string;
  graphs: GraphBinding[];
  strategy: {
    outputs: MergeStrategy;
    intermediate: MergeStrategy;
  };
};

type AnimationBinding = {
  key: string;
  id?: string;
  animation: AnimationSetup["animation"];
  setup: AnimationSetup;
};

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
  return relPath.replace(/^[.][\\/]/, "").replace(/\\/g, "/");
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

/**
 * Load an animation fixture payload from the embedded bundle.
 *
 * @throws If the fixture key is missing from the manifest.
 */
export async function loadAnimationFixture(name: string): Promise<AnimationSetup["animation"]> {
  return loadFixture<AnimationSetup["animation"]>(animationEntry(name));
}

function cloneStage(entries?: StageEntry[]): StageEntry[] {
  if (!entries) return [];
  return entries.map((entry) => ({ ...entry }));
}

function normalizeAnimationSeeds(seeds: AnimationSeed[] | undefined) {
  return (seeds ?? []).map((seed) =>
    typeof seed === "string"
      ? { fixture: seed }
      : seed,
  );
}

function normalizeGraphSeeds(seeds: GraphSeed[] | undefined) {
  return (seeds ?? []).map((seed) =>
    typeof seed === "string"
      ? { fixture: seed, mirrorWrites: false, stage: [] as StageEntry[] }
      : {
          fixture: seed.fixture,
          id: seed.id,
          subs: seed.subs,
          mirrorWrites: seed.mirrorWrites ?? false,
          stage: cloneStage(seed.stage),
        },
  );
}

function normalizeMergedGraphSeeds(seeds: MergedGraphSeed[] | undefined) {
  return (seeds ?? []).map((seed) => ({
    id: seed.id,
    graphs: normalizeGraphSeeds(seed.graphs),
    strategy: seed.strategy ?? {},
  }));
}

function toMergeStrategy(value: string | undefined, field: string, name: string): MergeStrategy {
  if (!value) return "error";
  const normalized = value.toLowerCase();
  if (normalized === "error") return "error";
  if (normalized === "namespace") return "namespace";
  if (normalized === "blend" || normalized === "blend-equal-weights" || normalized === "blend_equal_weights" || normalized === "blend_equal") {
    return "blend";
  }
  if (
    normalized === "add" ||
    normalized === "sum" ||
    normalized === "blend_sum" ||
    normalized === "blend-sum" ||
    normalized === "additive"
  ) {
    return "add";
  }
  if (
    normalized === "default-blend" ||
    normalized === "default_blend" ||
    normalized === "blend-default" ||
    normalized === "blend_weights" ||
    normalized === "blend-weights" ||
    normalized === "weights"
  ) {
    return "default-blend";
  }
  throw new Error(`Orchestration '${name}' provided unknown merge strategy '${value}' for ${field}`);
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

/**
 * Load a node-graph fixture and normalize it into a graph registration config.
 *
 * @throws If the fixture is missing or does not contain a GraphSpec-compatible payload.
 */
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

/**
 * Load a node-graph fixture and return its registration config.
 *
 * Alias for {@link loadNodeGraphConfig}.
 */
export async function loadNodeGraphSpec(name: string): Promise<GraphRegistrationConfig> {
  return loadNodeGraphConfig(name);
}

/** List orchestration fixture keys available via the embedded manifest. */
export async function listOrchestrationFixtures(): Promise<string[]> {
  return Object.keys(orchestrationMap());
}

/**
 * Load the parsed descriptor JSON value for the given orchestration fixture key.
 *
 * @throws If the fixture key is missing or the JSON cannot be parsed.
 */
export async function loadOrchestrationDescriptor<T = unknown>(name: string): Promise<T> {
  return loadFixture<T>(orchestrationRelPath(name));
}

/**
 * Load the orchestration descriptor as a JSON string.
 *
 * @throws If the fixture key is missing from the manifest.
 */
export async function loadOrchestrationJson(name: string): Promise<string> {
  return readFixture(orchestrationRelPath(name));
}

function mergeSubscriptions(
  base: GraphSubscriptions | undefined,
  override: GraphSubscriptions | undefined,
  mirrorWrites: boolean,
): GraphSubscriptions {
  const subs: GraphSubscriptions = {
    inputs: base?.inputs ? [...base.inputs] : undefined,
    outputs: base?.outputs ? [...base.outputs] : undefined,
    mirrorWrites,
  };
  if (override?.inputs) subs.inputs = [...override.inputs];
  if (override?.outputs) subs.outputs = [...override.outputs];
  if (typeof override?.mirrorWrites === "boolean") {
    subs.mirrorWrites = override.mirrorWrites;
  }
  return subs;
}

async function loadGraphBinding(seed: {
  fixture: string;
  id?: string;
  subs?: GraphSubscriptions;
  mirrorWrites: boolean;
  stage: StageEntry[];
}): Promise<GraphBinding> {
  const baseConfig = await loadNodeGraphConfig(seed.fixture);
  const config: GraphRegistrationConfig = {
    ...baseConfig,
    spec: baseConfig.spec,
  };
  if (seed.id) {
    config.id = seed.id;
  }
  const mergedSubs = mergeSubscriptions(baseConfig.subs, seed.subs, seed.mirrorWrites);
  if (mergedSubs.inputs || mergedSubs.outputs || typeof mergedSubs.mirrorWrites === "boolean") {
    config.subs = mergedSubs;
  }
  return {
    key: seed.fixture,
    id: seed.id,
    config,
    mirrorWrites: mergedSubs.mirrorWrites ?? false,
    stage: cloneStage(seed.stage),
  };
}

async function loadMergedGraphBinding(seed: {
  id: string;
  graphs: Array<{
    fixture: string;
    id?: string;
    subs?: GraphSubscriptions;
    mirrorWrites: boolean;
    stage: StageEntry[];
  }>;
  strategy: MergeStrategySeed;
}): Promise<MergedGraphBinding> {
  if (!seed.graphs.length) {
    throw new Error(`Orchestration '${seed.id}' merged graph is missing component graphs`);
  }
  const graphs = await Promise.all(seed.graphs.map((graph) => loadGraphBinding(graph)));
  return {
    id: seed.id,
    graphs,
    strategy: {
      outputs: toMergeStrategy(seed.strategy.outputs, "outputs", seed.id),
      intermediate: toMergeStrategy(seed.strategy.intermediate, "intermediate", seed.id),
    },
  };
}

async function loadAnimationBinding(
  seed: AnimationSeed,
  index: number,
): Promise<AnimationBinding> {
  const normalized =
    typeof seed === "string"
      ? { fixture: seed }
      : seed;
  const animation = await loadAnimationFixture(normalized.fixture);
  const setup =
    normalized.setup ??
    {
      animation,
      player: normalized.player ?? {
        name: index === 0 ? "fixture-player" : `fixture-player-${index}`,
        loop_mode: "loop",
      },
      ...(normalized.instance ? { instance: normalized.instance } : {}),
    };
  return {
    key: normalized.fixture,
    id: normalized.id,
    animation,
    setup,
  };
}

/**
 * Load an orchestration fixture and resolve all animation/graph dependencies.
 *
 * @throws If the orchestration fixture is missing required animation or graph references.
 */
/**
 * Load an orchestration fixture and resolve all animation/graph dependencies.
 *
 * @throws If the orchestration fixture is missing required animation or graph references.
 */
export async function loadOrchestrationBundle(
  name: string,
): Promise<{
  descriptor: PipelineDescriptor;
  animations: AnimationBinding[];
  graphs: GraphBinding[];
  mergedGraphs: MergedGraphBinding[];
  initialInputs: StageEntry[];
}> {
  const descriptor = await loadOrchestrationDescriptor<PipelineDescriptor>(name);
  if (!descriptor || typeof descriptor !== "object") {
    throw new Error(`Orchestration descriptor '${name}' did not resolve to an object`);
  }

  const animations = normalizeAnimationSeeds(descriptor.animations);
  const graphs = normalizeGraphSeeds(descriptor.graphs);
  const mergedGraphs = normalizeMergedGraphSeeds(
    (descriptor as Record<string, unknown>).mergedGraphs as MergedGraphSeed[] | undefined ??
      (descriptor as Record<string, unknown>).merged_graphs as MergedGraphSeed[] | undefined,
  );

  if (!animations.length) {
    throw new Error(`Orchestration '${name}' descriptor is missing animation references`);
  }
  if (!graphs.length && !mergedGraphs.length) {
    throw new Error(`Orchestration '${name}' descriptor is missing graph references`);
  }

  const animationBindings = await Promise.all(
    animations.map((seed, idx) => loadAnimationBinding(seed, idx)),
  );
  const graphBindings = await Promise.all(graphs.map((seed) => loadGraphBinding(seed)));
  const mergedBindings = await Promise.all(
    mergedGraphs.map((seed) => loadMergedGraphBinding(seed)),
  );

  const initialInputs: StageEntry[] = [
    ...(descriptor.initial_inputs ? cloneStage(descriptor.initial_inputs) : []),
    ...graphBindings.flatMap((binding) => cloneStage(binding.stage)),
    ...mergedBindings.flatMap((merged) =>
      merged.graphs.flatMap((binding) => cloneStage(binding.stage)),
    ),
  ];

  return {
    descriptor,
    animations: animationBindings,
    graphs: graphBindings,
    mergedGraphs: mergedBindings,
    initialInputs,
  };
}
