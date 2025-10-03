import { orchestrationEntry, orchestrationPath, loadFixture, manifest, readFixture, } from "./shared.js";
import { animationFixture } from "./animations.js";
import { nodeGraphSpec, nodeGraphStage } from "./nodeGraphs.js";
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
export function loadOrchestrationBundle(name) {
    const descriptor = orchestrationDescriptor(name);
    if (!descriptor || typeof descriptor !== "object") {
        throw new Error(`Orchestration descriptor '${name}' did not resolve to an object`);
    }
    const animation = animationFixture(descriptor.animation);
    const graphSpec = nodeGraphSpec(descriptor.graph);
    const graphStage = nodeGraphStage(descriptor.graph);
    return {
        descriptor,
        animation,
        graphSpec,
        graphStage,
    };
}
