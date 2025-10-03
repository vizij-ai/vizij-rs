import { animationEntry, loadFixture, manifest, readFixture, resolveFixturePath } from "./shared.js";
export function animationNames() {
    return Object.keys(manifest().animations);
}
export function animationJson(name) {
    return readFixture(animationEntry(name));
}
export function animationFixture(name) {
    return loadFixture(animationEntry(name));
}
export function animationPath(name) {
    return resolveFixturePath(animationEntry(name));
}
export function animationsRoot() {
    return resolveFixturePath("animations");
}
export const fixturesDirectory = animationsRoot();
