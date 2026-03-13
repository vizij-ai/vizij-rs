/**
 * Browser-facing entrypoint for `@vizij/test-fixtures`.
 *
 * Re-exports the browser bundle readers for animations, node graphs, and orchestrations.
 */
export * as animations from "./animations.browser.js";
export * as nodeGraphs from "./nodeGraphs.browser.js";
export * as orchestrations from "./orchestrations.browser.js";
export { fixturesRoot, manifest, resolveFixturePath } from "./shared.browser.js";
