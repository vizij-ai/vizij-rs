import { animationEntry, loadFixture, manifest, readFixture, resolveFixturePath } from "./shared.js";

/** JSON shape for an animation fixture payload. */
export type AnimationFixture<T = unknown> = T;

/**
 * List all animation fixture names in the manifest.
 *
 * @example
 * animationNames(); // ["simple-walk", "run-cycle", ...]
 */
export function animationNames(): string[] {
  return Object.keys(manifest().animations);
}

/**
 * Load an animation fixture as raw JSON text.
 *
 * @throws If the name is not present in the manifest or the file is missing.
 * @example
 * const json = animationJson("simple-walk");
 */
export function animationJson(name: string): string {
  return readFixture(animationEntry(name));
}

/**
 * Load an animation fixture and parse it as JSON.
 *
 * @throws If the name is not present in the manifest or the JSON is invalid.
 * @example
 * const animation = animationFixture("simple-walk");
 */
export function animationFixture<T = unknown>(name: string): AnimationFixture<T> {
  return loadFixture<T>(animationEntry(name));
}

/**
 * Resolve an animation fixture name to an absolute path.
 *
 * @throws If the name is not present in the manifest.
 * @example
 * const path = animationPath("simple-walk");
 */
export function animationPath(name: string): string {
  return resolveFixturePath(animationEntry(name));
}

/**
 * Absolute path to the animations fixtures directory.
 *
 * @example
 * const root = animationsRoot();
 */
export function animationsRoot(): string {
  return resolveFixturePath("animations");
}

/**
 * Deprecated alias for animationsRoot.
 *
 * @deprecated Use animationsRoot instead.
 */
export const fixturesDirectory = animationsRoot();
