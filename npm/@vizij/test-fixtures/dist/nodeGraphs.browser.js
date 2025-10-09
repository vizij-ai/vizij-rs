import { nodeGraphEntry, loadFixture, manifest, readFixture, resolveFixturePath, } from "./shared.browser.js";
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
export function nodeGraphStageJson(_name) {
    return null;
}
export function nodeGraphStage(_name) {
    return null;
}
export function nodeGraphStagePath(_name) {
    return null;
}
