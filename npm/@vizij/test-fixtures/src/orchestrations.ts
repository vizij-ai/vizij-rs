import {
  orchestrationEntry,
  orchestrationPath,
  loadFixture,
  manifest,
  readFixture,
} from "./shared.js";
import { animationFixture } from "./animations.js";
import { nodeGraphSpec } from "./nodeGraphs.js";

/** Stage entry for preloading values into an orchestration run. */
export interface StageEntry {
  path: string;
  value: unknown;
  shape?: unknown;
}

/** Input seed for graph bindings within an orchestration. */
export type GraphSeed =
  | string
  | {
      fixture: string;
      id?: string;
      subs?: Record<string, unknown>;
      mirrorWrites?: boolean;
      stage?: StageEntry[];
    };

/** Input seed for animation bindings within an orchestration. */
export type AnimationSeed =
  | string
  | {
      fixture: string;
      id?: string;
      setup?: Record<string, unknown>;
      player?: Record<string, unknown>;
      instance?: Record<string, unknown>;
    };

/** Merge strategy overrides per merged-graph seed. */
export interface MergeStrategySeed {
  outputs?: string;
  intermediate?: string;
}

/** Descriptor for a merged-graph entry in an orchestration. */
export interface MergedGraphSeed {
  id: string;
  graphs: GraphSeed[];
  strategy?: MergeStrategySeed;
}

/** Minimal shape for orchestration descriptor JSON. */
export interface PipelineDescriptor {
  description?: string;
  schedule?: string;
  animations?: AnimationSeed[];
  graphs?: GraphSeed[];
  mergedGraphs?: MergedGraphSeed[];
  initial_inputs?: StageEntry[];
  steps?: Array<{ delta: number; expect: Record<string, unknown> }>;
  [key: string]: unknown;
}

/** Loaded graph binding with resolved config and staging data. */
export interface OrchestrationGraphBinding<TConfig = Record<string, unknown>> {
  key: string;
  id?: string;
  config: TConfig;
  mirrorWrites: boolean;
  stage: StageEntry[];
}

/** Merge strategy options for merged graphs. */
export type MergeStrategy = "error" | "namespace" | "blend";

/** Resolved merge strategies for a merged-graph binding. */
export interface OrchestrationMergedGraphStrategy {
  outputs: MergeStrategy;
  intermediate: MergeStrategy;
}

/** Loaded merged-graph binding with resolved graph configs. */
export interface OrchestrationMergedGraphBinding<TConfig = Record<string, unknown>> {
  id: string;
  graphs: Array<OrchestrationGraphBinding<TConfig>>;
  strategy: OrchestrationMergedGraphStrategy;
}

/** Loaded animation binding with resolved fixture payloads. */
export interface OrchestrationAnimationBinding<
  TAnimation = Record<string, unknown>,
  TSetup = Record<string, unknown>,
> {
  key: string;
  id?: string;
  animation: TAnimation;
  setup: TSetup;
}

/** Fully loaded orchestration bundle with resolved fixture payloads. */
export interface OrchestrationBundle<
  TDescriptor extends PipelineDescriptor = PipelineDescriptor,
  TAnimation = Record<string, unknown>,
  TGraphSpec = Record<string, unknown>,
> {
  descriptor: TDescriptor;
  animations: Array<OrchestrationAnimationBinding<TAnimation>>;
  graphs: Array<OrchestrationGraphBinding<TGraphSpec>>;
  mergedGraphs: Array<OrchestrationMergedGraphBinding<TGraphSpec>>;
  initialInputs: StageEntry[];
}

/** List all orchestration fixture names in the manifest. */
export function orchestrationNames(): string[] {
  return Object.keys(manifest().orchestrations);
}

/** Load an orchestration descriptor fixture as raw JSON. */
export function orchestrationJson(name: string): string {
  const entry = orchestrationEntry(name);
  if (typeof entry === "string") {
    return readFixture(entry);
  }
  return readFixture(entry.path);
}

/** Load and parse an orchestration descriptor fixture. */
export function orchestrationDescriptor<T = unknown>(name: string): T {
  const entry = orchestrationEntry(name);
  const rel = typeof entry === "string" ? entry : entry.path;
  return loadFixture<T>(rel);
}

/** Resolve an orchestration descriptor path to an absolute path. */
export function orchestrationDescriptorPath(name: string): string {
  return orchestrationPath(orchestrationEntry(name));
}

interface NormalizedAnimationSeed {
  fixture: string;
  id?: string;
  setup?: Record<string, unknown>;
  player?: Record<string, unknown>;
  instance?: Record<string, unknown>;
}

interface NormalizedGraphSeed {
  fixture: string;
  id?: string;
  subs?: Record<string, unknown>;
  mirrorWrites: boolean;
  stage: StageEntry[];
}

interface NormalizedMergedGraphSeed {
  id: string;
  graphs: NormalizedGraphSeed[];
  strategy: MergeStrategySeed;
}

function toStageEntries(entries?: StageEntry[]): StageEntry[] {
  if (!entries) return [];
  return entries.map((entry) => ({ ...entry }));
}

function normalizeAnimationSeeds(seeds: AnimationSeed[] | undefined): NormalizedAnimationSeed[] {
  return (seeds ?? []).map((seed) =>
    typeof seed === "string"
      ? { fixture: seed }
      : {
          fixture: seed.fixture,
          id: seed.id,
          setup: seed.setup,
          player: seed.player,
          instance: seed.instance,
        },
  );
}

function normalizeGraphSeeds(seeds: GraphSeed[] | undefined): NormalizedGraphSeed[] {
  return (seeds ?? []).map((seed) =>
    typeof seed === "string"
      ? { fixture: seed, mirrorWrites: false, stage: [] as StageEntry[] }
      : {
          fixture: seed.fixture,
          id: seed.id,
          subs: seed.subs,
          mirrorWrites: seed.mirrorWrites ?? false,
          stage: toStageEntries(seed.stage),
        },
  );
}

function normalizeMergedGraphSeeds(
  seeds: MergedGraphSeed[] | undefined,
): NormalizedMergedGraphSeed[] {
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
  if (normalized === "blend" || normalized === "blend-equal-weights") return "blend";
  throw new Error(`Orchestration '${name}' provided unknown merge strategy '${value}' for ${field}`);
}

function loadGraphBinding<TGraphSpec = Record<string, unknown>>(
  seed: NormalizedGraphSeed,
  name: string,
): OrchestrationGraphBinding<TGraphSpec> {
  const config = nodeGraphSpec(seed.fixture) as Record<string, unknown> as TGraphSpec;
  if (seed.id && typeof config === "object" && config !== null) {
    (config as Record<string, unknown>).id = seed.id;
  }
  if (seed.subs && typeof config === "object" && config !== null) {
    (config as Record<string, unknown>).subs = seed.subs;
  }
  if (typeof config === "object" && config !== null) {
    (config as Record<string, unknown>).mirror_writes = seed.mirrorWrites;
  }
  return {
    key: seed.fixture,
    id: seed.id,
    config,
    mirrorWrites: seed.mirrorWrites,
    stage: toStageEntries(seed.stage),
  };
}

function loadMergedGraphBinding<TGraphSpec = Record<string, unknown>>(
  seed: NormalizedMergedGraphSeed,
  name: string,
): OrchestrationMergedGraphBinding<TGraphSpec> {
  if (!seed.graphs.length) {
    throw new Error(`Orchestration '${name}' merged graph '${seed.id}' is missing component graphs`);
  }
  const graphs = seed.graphs.map((graph) => loadGraphBinding<TGraphSpec>(graph, name));
  const strategy: OrchestrationMergedGraphStrategy = {
    outputs: toMergeStrategy(seed.strategy.outputs, "outputs", name),
    intermediate: toMergeStrategy(seed.strategy.intermediate, "intermediate", name),
  };
  return {
    id: seed.id,
    graphs,
    strategy,
  };
}

function loadAnimationBinding(
  seed: NormalizedAnimationSeed,
  index: number,
  name: string,
): OrchestrationAnimationBinding {
  const animation = animationFixture(seed.fixture) as Record<string, unknown>;
  const setup =
    seed.setup ??
    {
      animation,
      player: seed.player ?? {
        name: index === 0 ? "fixture-player" : `fixture-player-${index}`,
        loop_mode: "loop",
      },
      ...(seed.instance ? { instance: seed.instance } : {}),
    };
  return {
    key: seed.fixture,
    id: seed.id,
    animation,
    setup,
  };
}

/**
 * Load an orchestration fixture and resolve all animation/graph dependencies.
 *
 * Throws when the fixture is missing required animation or graph references.
 */
export function loadOrchestrationBundle(
  name: string,
): OrchestrationBundle<PipelineDescriptor> {
  const descriptor = orchestrationDescriptor<PipelineDescriptor>(name);
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

  const animationBindings = animations.map((seed, idx) => loadAnimationBinding(seed, idx, name));
  const graphBindings = graphs.map((seed) => loadGraphBinding(seed, name));
  const mergedBindings = mergedGraphs.map((seed) => loadMergedGraphBinding(seed, name));

  const initialInputs: StageEntry[] = [
    ...(descriptor.initial_inputs ?? []),
    ...graphBindings.flatMap((binding) => toStageEntries(binding.stage)),
    ...mergedBindings.flatMap((merged) =>
      merged.graphs.flatMap((binding) => toStageEntries(binding.stage)),
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
