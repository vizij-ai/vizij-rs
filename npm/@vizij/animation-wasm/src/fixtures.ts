import type { StoredAnimation } from "./types.js";

type AnimationModule = typeof import("@vizij/test-fixtures")["animations"];

let loader: Promise<AnimationModule> | null = null;

function fixturesModule(): Promise<AnimationModule> {
  if (!loader) {
    loader = import("@vizij/test-fixtures")
      .then((mod) => mod.animations)
      .catch((err) => {
        throw new Error(
          `Failed to load @vizij/test-fixtures. Install the workspace package alongside @vizij/animation-wasm to access shared samples. Original error: ${err instanceof Error ? err.message : String(err)}`,
        );
      });
  }
  return loader;
}

/** List the animation fixture keys available via the shared manifest. */
export async function listAnimationFixtures(): Promise<string[]> {
  const module = await fixturesModule();
  return module.animationNames();
}

/** Load the StoredAnimation-compatible fixture payload for the given key. */
export async function loadAnimationFixture<T = StoredAnimation>(name: string): Promise<T> {
  const module = await fixturesModule();
  return module.animationFixture<T>(name);
}

/** Load the raw JSON text for the given animation fixture key. */
export async function loadAnimationJson(name: string): Promise<string> {
  const module = await fixturesModule();
  return module.animationJson(name);
}

/** Resolve the filesystem path for the given animation fixture key. */
export async function resolveAnimationPath(name: string): Promise<string> {
  const module = await fixturesModule();
  return module.animationPath(name);
}
