/**
 * Optional passes a face's GLB goes through when it is imported, before it
 * becomes a face bundle, run in WebAssembly.
 *
 * Await `init()` once, then call a pass. Each pass returns the rewritten
 * file (or `null` when it changed nothing) and a report of what it did.
 */
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";

/** The wasm interface version this wrapper is written against. */
const EXPECTED_ABI = 1;

interface WasmResult {
  readonly bytes: Uint8Array<ArrayBuffer> | undefined;
  readonly reportJson: string;
  free(): void;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  abi_version: () => number;
  optimizeMorphTriangles: (glb: Uint8Array) => WasmResult;
}

const bindingCache: { current: WasmBindings | null } = { current: null };

function toWasmBindgenInitOptions(initArg: unknown): { module_or_path: unknown } {
  if (
    initArg &&
    typeof initArg === "object" &&
    "module_or_path" in (initArg as Record<string, unknown>)
  ) {
    return initArg as { module_or_path: unknown };
  }
  return { module_or_path: initArg };
}

const loadBindingsImpl =
  typeof window === "undefined"
    ? loadWasmBindings
    : (loadWasmBindingsBrowser as typeof loadWasmBindings);

export type InitInput = LoaderInitInput;

let initPromise: Promise<void> | null = null;

/**
 * Loads the wasm module once; later calls reuse it. In a browser the
 * module is fetched next to this file; elsewhere pass its bytes or URL.
 */
export function init(input?: InitInput): Promise<void> {
  if (!initPromise) {
    initPromise = loadBindingsImpl<WasmBindings>(
      {
        cache: bindingCache,
        importModule: () => import("../../pkg/vizij_import_wasm.js"),
        defaultWasmUrl: () =>
          new URL("../../pkg/vizij_import_wasm_bg.wasm", import.meta.url).toString(),
        init: async (module: unknown, initArg: unknown) => {
          await (module as WasmBindings).default(toWasmBindgenInitOptions(initArg));
        },
        getBindings: (module: unknown) => module as WasmBindings,
        expectedAbi: EXPECTED_ABI,
        getAbiVersion: (bindings) => Number(bindings.abi_version()),
      },
      input,
    ).then(() => undefined);
    // A failed load is not cached, so a later call can retry it.
    initPromise.catch(() => {
      initPromise = null;
    });
  }
  return initPromise;
}

function bindings(): WasmBindings {
  if (!bindingCache.current) {
    throw new Error("Call init() from @vizij/import before running a pass.");
  }
  return bindingCache.current;
}

/** What the morph-triangle pass did with one part of a mesh. */
export type MorphTrianglesOutcome = "retriangulated" | "kept" | "skipped";

/**
 * The share of one shape's area drawn with reversed winding, before and
 * after the pass; `null` for a shape with no area. `shape` is `rest` or the
 * morph target's name.
 */
export interface ShapeFold {
  shape: string;
  before: number | null;
  after: number | null;
}

/** One polygon of a morphed mesh, or a primitive the pass could not read. */
export interface MorphTrianglesPart {
  mesh: number;
  meshName: string | null;
  primitive: number;
  vertices: number;
  outcome: MorphTrianglesOutcome;
  /** Why the part was left alone, when `outcome` is `skipped`. */
  skipReason: string | null;
  folds: ShapeFold[];
}

export interface MorphTrianglesReport {
  parts: MorphTrianglesPart[];
}

export interface MorphTrianglesResult {
  /**
   * The rewritten GLB, or `null` when the pass changed nothing. A fresh
   * copy backed by its own `ArrayBuffer`, so it can go straight into a
   * `Blob` or `File`.
   */
  bytes: Uint8Array<ArrayBuffer> | null;
  report: MorphTrianglesReport;
}

/**
 * Re-triangulates every part of a GLB modelled as one large polygon (a
 * mouth, a pupil) so its triangles stay unfolded across its morph targets.
 * Vertices, triangle counts and the glTF JSON are unchanged; only index
 * data is rewritten. Throws when the input is not a GLB.
 */
export function optimizeMorphTriangles(glb: Uint8Array): MorphTrianglesResult {
  const result = bindings().optimizeMorphTriangles(glb);
  try {
    return {
      bytes: result.bytes ?? null,
      report: JSON.parse(result.reportJson) as MorphTrianglesReport,
    };
  } finally {
    result.free();
  }
}

/**
 * The shapes, per part, that still fold by more than `threshold` (a share
 * of the shape's area) after the pass: shapes no triangulation of the
 * part's vertices can follow, which need remodelling.
 */
export function stillFolding(
  report: MorphTrianglesReport,
  threshold = 0.01,
): Array<{ part: MorphTrianglesPart; folds: ShapeFold[] }> {
  return report.parts
    .map((part) => ({
      part,
      folds: part.folds.filter((fold) => (fold.after ?? 0) > threshold),
    }))
    .filter((entry) => entry.folds.length > 0);
}
