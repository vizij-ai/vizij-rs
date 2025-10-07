import bundle from "./generated/browser-fixtures.json";
const data = bundle;
const manifestCache = data.manifest;
function normalizeRelPath(relPath) {
    return relPath.replace(/^[.][/\\]/, "").replace(/\\/g, "/");
}
function bundledFixture(relPath) {
    const key = normalizeRelPath(relPath);
    const raw = data.files[key];
    if (typeof raw !== "string") {
        throw new Error(`Fixture '${relPath}' was not included in the browser fixtures bundle`);
    }
    return raw;
}
export function fixturesRoot() {
    return "fixtures";
}
export function manifest() {
    return manifestCache;
}
export function resolveFixturePath(relPath) {
    const normalized = normalizeRelPath(relPath);
    return `fixtures/${normalized}`;
}
export function readFixture(relPath) {
    return bundledFixture(relPath);
}
export function loadFixture(relPath) {
    return JSON.parse(bundledFixture(relPath));
}
export function animationEntry(name) {
    const entry = manifestCache.animations[name];
    if (!entry) {
        throw new Error(`Unknown animation fixture: ${name}`);
    }
    return entry;
}
export function nodeGraphEntry(name) {
    const entry = manifestCache["node-graphs"][name];
    if (!entry) {
        throw new Error(`Unknown node-graph fixture: ${name}`);
    }
    return entry;
}
export function orchestrationEntry(name) {
    const entry = manifestCache.orchestrations[name];
    if (!entry) {
        throw new Error(`Unknown orchestration fixture: ${name}`);
    }
    return entry;
}
export function orchestrationPath(entry) {
    if (typeof entry === "string") {
        return resolveFixturePath(entry);
    }
    return resolveFixturePath(entry.path);
}
