import {
  orchestrationEntry,
  orchestrationPath,
  loadFixture,
  manifest,
  readFixture,
} from "./shared.js";
import { animationFixture } from "./animations.js";
import { nodeGraphSpec } from "./nodeGraphs.js";

/**
 * Stage entry for preloading values into an orchestration run.
 *
 * @example
 * const entry: StageEntry = { path: "rig/pose", value: { float: 1 } };
 */
export interface StageEntry {
  path: string;
  value: unknown;
  shape?: unknown;
}

/**
 * Input seed for graph bindings within an orchestration.
 *
 * @example
 * const seed: GraphSeed = "oscillator-basics";
 */
export type GraphSeed =
  | string
  | {
      fixture: string;
      id?: string;
      subs?: Record<string, unknown>;
      mirrorWrites?: boolean;
      stage?: StageEntry[];
    };

/**
 * Input seed for animation bindings within an orchestration.
 *
 * @example
 * const seed: AnimationSeed = { fixture: "simple-walk", player: { loop_mode: "loop" } };
 */
export type AnimationSeed =
  | string
  | {
      fixture: string;
      id?: string;
      setup?: Record<string, unknown>;
      player?: Record<string, unknown>;
      instance?: Record<string, unknown>;
    };

/**
 * Merge strategy overrides per merged-graph seed.
 *
 * @example
 * const strategy: MergeStrategySeed = { outputs: "namespace" };
 */
export interface MergeStrategySeed {
  outputs?: string;
  intermediate?: string;
}

/**
 * Descriptor for a merged-graph entry in an orchestration.
 *
 * @example
 * const merged: MergedGraphSeed = { id: "rig", graphs: ["rig-core", "rig-face"] };
 */
export interface MergedGraphSeed {
  id: string;
  graphs: GraphSeed[];
  strategy?: MergeStrategySeed;
}

/**
 * Minimal shape for orchestration descriptor JSON.
 *
 * @example
 * const descriptor: PipelineDescriptor = { animations: ["simple-walk"], graphs: ["oscillator-basics"] };
 */
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

/**
 * Loaded graph binding with resolved config and staging data.
 *
 * @example
 * const binding: OrchestrationGraphBinding = {
 *   key: "oscillator-basics",
 *   config: { nodes: [], edges: [] },
 *   mirrorWrites: false,
 *   stage: [],
 * };
 */
export interface OrchestrationGraphBinding<TConfig = Record<string, unknown>> {
  key: string;
  id?: string;
  config: TConfig;
  mirrorWrites: boolean;
  stage: StageEntry[];
}

/**
 * Merge strategy options for merged graphs.
 *
 * @example
 * const mode: MergeStrategy = "namespace";
 */
export type MergeStrategy = "error" | "namespace" | "blend";

/**
 * Resolved merge strategies for a merged-graph binding.
 *
 * @example
 * const strategy: OrchestrationMergedGraphStrategy = { outputs: "blend", intermediate: "error" };
 */
export interface OrchestrationMergedGraphStrategy {
  outputs: MergeStrategy;
  intermediate: MergeStrategy;
}

/**
 * Loaded merged-graph binding with resolved graph configs.
 *
 * @example
 * const merged: OrchestrationMergedGraphBinding = {
 *   id: "rig",
 *   graphs: [],
 *   strategy: { outputs: "error", intermediate: "error" },
 * };
 */
export interface OrchestrationMergedGraphBinding<TConfig = Record<string, unknown>> {
  id: string;
  graphs: Array<OrchestrationGraphBinding<TConfig>>;
  strategy: OrchestrationMergedGraphStrategy;
}

/**
 * Loaded animation binding with resolved fixture payloads.
 *
 * @example
 * const binding: OrchestrationAnimationBinding = {
 *   key: "simple-walk",
 *   animation: {},
 *   setup: {},
 * };
 */
export interface OrchestrationAnimationBinding<
  TAnimation = Record<string, unknown>,
  TSetup = Record<string, unknown>,
> {
  key: string;
  id?: string;
  animation: TAnimation;
  setup: TSetup;
}

/**
 * Fully loaded orchestration bundle with resolved fixture payloads.
 *
 * @example
 * const bundle = loadOrchestrationBundle("simple-orchestration");
 */
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

/**
 * List all orchestration fixture names in the manifest.
 *
 * @example
 * orchestrationNames(); // ["simple-orchestration", ...]
 */
export function orchestrationNames(): string[] {
  return Object.keys(manifest().orchestrations);
}

/**
 * Load an orchestration descriptor fixture as raw JSON.
 *
 * @throws If the name is not present in the manifest or the file is missing.
 * @example
 * const json = orchestrationJson("simple-orchestration");
 */
export function orchestrationJson(name: string): string {
  const entry = orchestrationEntry(name);
  if (typeof entry === "string") {
    return readFixture(entry);
  }
  return readFixture(entry.path);
}

/**
 * Load and parse an orchestration descriptor fixture.
 *
 * @throws If the name is not present in the manifest or the JSON is invalid.
 * @example
 * const descriptor = orchestrationDescriptor("simple-orchestration");
 */
export function orchestrationDescriptor<T = unknown>(name: string): T {
  const entry = orchestrationEntry(name);
  const rel = typeof entry === "string" ? entry : entry.path;
  return loadFixture<T>(rel);
}

/**
 * Resolve an orchestration descriptor path to an absolute path.
 *
 * @throws If the name is not present in the manifest.
 * @example
 * const path = orchestrationDescriptorPath("simple-orchestration");
 */
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
 * @throws When the fixture is missing required animation or graph references.
 * @example
 * const bundle = loadOrchestrationBundle("simple-orchestration");
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
