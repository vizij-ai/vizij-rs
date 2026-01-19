/**
 * Browser-friendly access to shared Vizij JSON fixtures.
 *
 * @example
 * const names = animations.animationNames();
 */
export * as animations from "./animations.browser.js";
export * as nodeGraphs from "./nodeGraphs.browser.js";
export * as orchestrations from "./orchestrations.browser.js";
export { fixturesRoot, manifest, resolveFixturePath } from "./shared.browser.js";
