import {
  animationEntry,
  loadFixture,
  manifest,
  readFixture,
  resolveFixturePath,
} from "./shared.browser.js";

/** JSON shape for an animation fixture payload. */
export type AnimationFixture<T = unknown> = T;

/** List all animation fixture names in the bundled manifest. */
export function animationNames(): string[] {
  return Object.keys(manifest().animations);
}

/**
 * Load an animation fixture as raw JSON text from the bundle.
 *
 * @throws If the fixture key is missing from the manifest.
 */
export function animationJson(name: string): string {
  return readFixture(animationEntry(name));
}

/**
 * Load an animation fixture and parse it as JSON.
 *
 * @throws If the fixture key is missing or the JSON is invalid.
 */
export function animationFixture<T = unknown>(name: string): AnimationFixture<T> {
  return loadFixture<T>(animationEntry(name));
}

/**
 * Resolve an animation fixture name to a bundled "fixtures/..." path.
 *
 * @throws If the fixture key is missing from the manifest.
 */
export function animationPath(name: string): string {
  return resolveFixturePath(animationEntry(name));
}

/** Bundled root for animation fixture paths. */
export function animationsRoot(): string {
  return resolveFixturePath("animations");
}

/** Deprecated alias for animationsRoot. */
export const fixturesDirectory = animationsRoot();
