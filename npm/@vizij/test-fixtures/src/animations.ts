import { animationEntry, loadFixture, manifest, readFixture, resolveFixturePath } from "./shared.js";

/** JSON shape for an animation fixture payload. */
export type AnimationFixture<T = unknown> = T;

/** List all animation fixture names in the manifest. */
export function animationNames(): string[] {
  return Object.keys(manifest().animations);
}

/** Load an animation fixture as raw JSON text. */
export function animationJson(name: string): string {
  return readFixture(animationEntry(name));
}

/** Load an animation fixture and parse it as JSON. */
export function animationFixture<T = unknown>(name: string): AnimationFixture<T> {
  return loadFixture<T>(animationEntry(name));
}

/** Resolve an animation fixture name to an absolute path. */
export function animationPath(name: string): string {
  return resolveFixturePath(animationEntry(name));
}

/** Absolute path to the animations fixtures directory. */
export function animationsRoot(): string {
  return resolveFixturePath("animations");
}

/** Deprecated alias for animationsRoot. */
export const fixturesDirectory = animationsRoot();
