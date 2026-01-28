import { animationEntry, loadFixture, manifest, readFixture, resolveFixturePath } from "./shared.js";

/**
 * JSON shape for an animation fixture payload.
 *
 * @typeParam T - Parsed JSON payload type for the fixture.
 */
export type AnimationFixture<T = unknown> = T;

/**
 * List all animation fixture names in the manifest.
 *
 * @returns Array of fixture keys.
 */
export function animationNames(): string[] {
  return Object.keys(manifest().animations);
}

/**
 * Load an animation fixture as raw JSON text.
 *
 * @param name - Fixture key from the manifest.
 * @returns Raw JSON string for the fixture.
 * @throws If the name is not present in the manifest or the file is missing.
 */
export function animationJson(name: string): string {
  return readFixture(animationEntry(name));
}

/**
 * Load an animation fixture and parse it as JSON.
 *
 * @param name - Fixture key from the manifest.
 * @returns Parsed fixture payload.
 * @throws If the name is not present in the manifest or the JSON is invalid.
 */
export function animationFixture<T = unknown>(name: string): AnimationFixture<T> {
  return loadFixture<T>(animationEntry(name));
}

/**
 * Resolve an animation fixture name to an absolute path.
 *
 * @param name - Fixture key from the manifest.
 * @returns Absolute path to the fixture file.
 * @throws If the name is not present in the manifest.
 */
export function animationPath(name: string): string {
  return resolveFixturePath(animationEntry(name));
}

/**
 * Absolute path to the animations fixtures directory.
 *
 * @returns Absolute path to the animations root directory.
 */
export function animationsRoot(): string {
  return resolveFixturePath("animations");
}

/**
 * Deprecated alias for {@link animationsRoot}.
 *
 * @deprecated Use {@link animationsRoot} instead.
 */
export const fixturesDirectory = animationsRoot();
