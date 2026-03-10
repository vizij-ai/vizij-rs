/**
 * Node-facing entrypoint for `@vizij/test-fixtures`.
 *
 * Re-exports the domain helpers for animations, node graphs, and orchestrations together with
 * the shared manifest/path helpers.
 */
export * as animations from "./animations.js";
export * as nodeGraphs from "./nodeGraphs.js";
export * as orchestrations from "./orchestrations.js";
export { fixturesRoot, manifest, resolveFixturePath } from "./shared.js";
