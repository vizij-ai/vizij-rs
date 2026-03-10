/**
 * Embedded animation fixture helpers for `@vizij/animation-wasm`.
 *
 * The functions in this module read the generated fixture bundle shipped with the package so
 * browser and Node tests can load canonical animation payloads without reading the repo files.
 */
import bundle from "./generated/fixtures-bundle.js";
import type { StoredAnimation } from "./types.js";

type FixturesManifest = typeof bundle.manifest;

const manifest: FixturesManifest = bundle.manifest;
const files = bundle.files as Record<string, string>;

function animationsMap(): Record<string, string> {
  return (manifest.animations ?? {}) as Record<string, string>;
}

function normalizeRelPath(relPath: string): string {
  return relPath.replace(/^[.][/\\]/, "").replace(/\\/g, "/");
}

function animationEntry(name: string): string {
  const rel = animationsMap()[name];
  if (!rel) {
    throw new Error(`Unknown animation fixture: ${name}`);
  }
  return rel;
}

function readFixture(relPath: string): string {
  const normalized = normalizeRelPath(relPath);
  const raw = files[normalized];
  if (typeof raw !== "string") {
    throw new Error(`Fixture '${relPath}' was not embedded in this build`);
  }
  return raw;
}

function loadFixture<T>(relPath: string): T {
  return JSON.parse(readFixture(relPath)) as T;
}

/** List the animation fixture keys available via the embedded manifest. */
export async function listAnimationFixtures(): Promise<string[]> {
  return Object.keys(manifest.animations ?? {});
}

/** Load the StoredAnimation-compatible fixture payload for the given key. */
export async function loadAnimationFixture<T = StoredAnimation>(name: string): Promise<T> {
  return loadFixture<T>(animationEntry(name));
}

/** Load the raw JSON text for the given animation fixture key. */
export async function loadAnimationJson(name: string): Promise<string> {
  return readFixture(animationEntry(name));
}

/** Resolve the virtual fixtures/ path for the given animation fixture key. */
export async function resolveAnimationPath(name: string): Promise<string> {
  return `fixtures/${normalizeRelPath(animationEntry(name))}`;
}
