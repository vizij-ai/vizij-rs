/**
 * Browser-bundle orchestration fixture helpers for `@vizij/test-fixtures`.
 */
import { orchestrationEntry, orchestrationPath, loadFixture, manifest, readFixture, } from "./shared.browser.js";
import { animationFixture } from "./animations.browser.js";
import { nodeGraphSpec } from "./nodeGraphs.browser.js";
export function orchestrationNames() {
    return Object.keys(manifest().orchestrations);
}
export function orchestrationJson(name) {
    const entry = orchestrationEntry(name);
    if (typeof entry === "string") {
        return readFixture(entry);
    }
    return readFixture(entry.path);
}
export function orchestrationDescriptor(name) {
    const entry = orchestrationEntry(name);
    const rel = typeof entry === "string" ? entry : entry.path;
    return loadFixture(rel);
}
export function orchestrationDescriptorPath(name) {
    return orchestrationPath(orchestrationEntry(name));
}
function toStageEntries(entries) {
    if (!entries)
        return [];
    return entries.map((entry) => ({ ...entry }));
}
function normalizeAnimationSeeds(seeds) {
    return (seeds ?? []).map((seed) => typeof seed === "string"
        ? { fixture: seed }
        : {
            fixture: seed.fixture,
            id: seed.id,
            setup: seed.setup,
            player: seed.player,
            instance: seed.instance,
        });
}
function normalizeGraphSeeds(seeds) {
    return (seeds ?? []).map((seed) => typeof seed === "string"
        ? { fixture: seed, mirrorWrites: false, stage: [] }
        : {
            fixture: seed.fixture,
            id: seed.id,
            subs: seed.subs,
            mirrorWrites: seed.mirrorWrites ?? false,
            stage: toStageEntries(seed.stage),
        });
}
function normalizeMergedGraphSeeds(seeds) {
    return (seeds ?? []).map((seed) => ({
        id: seed.id,
        graphs: normalizeGraphSeeds(seed.graphs),
        strategy: seed.strategy ?? {},
    }));
}
function toMergeStrategy(value, field, name) {
    if (!value)
        return "error";
    const normalized = value.toLowerCase();
    if (normalized === "error")
        return "error";
    if (normalized === "namespace")
        return "namespace";
    if (normalized === "blend" || normalized === "blend-equal-weights")
        return "blend";
    throw new Error(`Orchestration '${name}' provided unknown merge strategy '${value}' for ${field}`);
}
function loadGraphBinding(seed, name) {
    const config = nodeGraphSpec(seed.fixture);
    if (seed.id && typeof config === "object" && config !== null) {
        config.id = seed.id;
    }
    if (seed.subs && typeof config === "object" && config !== null) {
        config.subs = seed.subs;
    }
    if (typeof config === "object" && config !== null) {
        config.mirror_writes = seed.mirrorWrites;
    }
    return {
        key: seed.fixture,
        id: seed.id,
        config,
        mirrorWrites: seed.mirrorWrites,
        stage: toStageEntries(seed.stage),
    };
}
function loadMergedGraphBinding(seed, name) {
    if (!seed.graphs.length) {
        throw new Error(`Orchestration '${name}' merged graph '${seed.id}' is missing component graphs`);
    }
    const graphs = seed.graphs.map((graph) => loadGraphBinding(graph, name));
    const strategy = {
        outputs: toMergeStrategy(seed.strategy.outputs, "outputs", name),
        intermediate: toMergeStrategy(seed.strategy.intermediate, "intermediate", name),
    };
    return {
        id: seed.id,
        graphs,
        strategy,
    };
}
function loadAnimationBinding(seed, index, name) {
    const animation = animationFixture(seed.fixture);
    const setup = seed.setup ??
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
export function loadOrchestrationBundle(name) {
    const descriptor = orchestrationDescriptor(name);
    if (!descriptor || typeof descriptor !== "object") {
        throw new Error(`Orchestration descriptor '${name}' did not resolve to an object`);
    }
    const animations = normalizeAnimationSeeds(descriptor.animations);
    const graphs = normalizeGraphSeeds(descriptor.graphs);
    const mergedGraphs = normalizeMergedGraphSeeds(descriptor.mergedGraphs);
    if (!animations.length) {
        throw new Error(`Orchestration '${name}' descriptor is missing animation references`);
    }
    if (!graphs.length && !mergedGraphs.length) {
        throw new Error(`Orchestration '${name}' descriptor is missing graph references`);
    }
    const animationBindings = animations.map((seed, idx) => loadAnimationBinding(seed, idx, name));
    const graphBindings = graphs.map((seed) => loadGraphBinding(seed, name));
    const mergedBindings = mergedGraphs.map((seed) => loadMergedGraphBinding(seed, name));
    const initialInputs = [
        ...(descriptor.initial_inputs ?? []),
        ...graphBindings.flatMap((binding) => toStageEntries(binding.stage)),
        ...mergedBindings.flatMap((merged) => merged.graphs.flatMap((binding) => toStageEntries(binding.stage))),
    ];
    return {
        descriptor,
        animations: animationBindings,
        graphs: graphBindings,
        mergedGraphs: mergedBindings,
        initialInputs,
    };
}
