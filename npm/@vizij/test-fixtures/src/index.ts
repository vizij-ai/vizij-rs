/**
 * Browser/Node-friendly access to shared Vizij JSON fixtures.
 *
 * Re-exported modules surface helpers for animations, graphs, and orchestrations.
 */
export * as animations from "./animations.js";
export * as nodeGraphs from "./nodeGraphs.js";
export * as orchestrations from "./orchestrations.js";
export { fixturesRoot, manifest, resolveFixturePath } from "./shared.js";
