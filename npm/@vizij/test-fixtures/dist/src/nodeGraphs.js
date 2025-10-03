import { nodeGraphEntry, loadFixture, manifest, readFixture, resolveFixturePath, } from "./shared.js";
function entry(name) {
    return nodeGraphEntry(name);
}
export function nodeGraphNames() {
    return Object.keys(manifest()["node-graphs"]);
}
export function nodeGraphSpecJson(name) {
    return readFixture(entry(name).spec);
}
export function nodeGraphSpec(name) {
    return loadFixture(entry(name).spec);
}
export function nodeGraphSpecPath(name) {
    return resolveFixturePath(entry(name).spec);
}
export function nodeGraphStageJson(name) {
    const stage = entry(name).stage;
    return stage ? readFixture(stage) : null;
}
export function nodeGraphStage(name) {
    const stage = entry(name).stage;
    return stage ? loadFixture(stage) : null;
}
export function nodeGraphStagePath(name) {
    const stage = entry(name).stage;
    return stage ? resolveFixturePath(stage) : null;
}
