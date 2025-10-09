import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
let fixturesRootCache = null;
let manifestCache = null;
function locateFixturesRoot() {
    if (fixturesRootCache) {
        return fixturesRootCache;
    }
    let current = dirname(fileURLToPath(import.meta.url));
    for (let i = 0; i < 10; i += 1) {
        const candidate = resolve(current, "fixtures/manifest.json");
        if (existsSync(candidate)) {
            const root = resolve(current, "fixtures");
            fixturesRootCache = root;
            return root;
        }
        const parent = resolve(current, "..");
        if (parent === current) {
            break;
        }
        current = parent;
    }
    throw new Error("Unable to locate fixtures/manifest.json relative to @vizij/test-fixtures");
}
export function fixturesRoot() {
    return locateFixturesRoot();
}
export function manifest() {
    if (!manifestCache) {
        const raw = readFileSync(resolve(locateFixturesRoot(), "manifest.json"), "utf8");
        manifestCache = JSON.parse(raw);
    }
    return manifestCache;
}
export function resolveFixturePath(relPath) {
    return resolve(locateFixturesRoot(), relPath);
}
export function readFixture(relPath) {
    return readFileSync(resolveFixturePath(relPath), "utf8");
}
export function loadFixture(relPath) {
    return JSON.parse(readFixture(relPath));
}
export function animationEntry(name) {
    const entry = manifest().animations[name];
    if (!entry) {
        throw new Error(`Unknown animation fixture: ${name}`);
    }
    return entry;
}
export function nodeGraphEntry(name) {
    const entry = manifest()["node-graphs"][name];
    if (!entry) {
        throw new Error(`Unknown node-graph fixture: ${name}`);
    }
    return entry;
}
export function orchestrationEntry(name) {
    const entry = manifest().orchestrations[name];
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
