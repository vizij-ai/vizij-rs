import { animationEntry, loadFixture, manifest, readFixture, resolveFixturePath } from "./shared.js";

export type AnimationFixture<T = unknown> = T;

export function animationNames(): string[] {
  return Object.keys(manifest().animations);
}

export function animationJson(name: string): string {
  return readFixture(animationEntry(name));
}

export function animationFixture<T = unknown>(name: string): AnimationFixture<T> {
  return loadFixture<T>(animationEntry(name));
}

export function animationPath(name: string): string {
  return resolveFixturePath(animationEntry(name));
}

export function animationsRoot(): string {
  return resolveFixturePath("animations");
}

export const fixturesDirectory = animationsRoot();
