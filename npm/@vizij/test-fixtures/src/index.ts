/**
 * Browser/Node-friendly access to shared Vizij JSON fixtures.
 *
 * @example
 * const names = animations.animationNames();
 */
export * as animations from "./animations.js";
export * as nodeGraphs from "./nodeGraphs.js";
export * as orchestrations from "./orchestrations.js";
export { fixturesRoot, manifest, resolveFixturePath } from "./shared.js";
