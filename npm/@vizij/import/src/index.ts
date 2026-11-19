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
const EXPECTED_ABI = 4;

interface WasmResult {
  readonly bytes: Uint8Array<ArrayBuffer> | undefined;
  readonly reportJson: string;
  free(): void;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  abi_version: () => number;
  optimizeMorphTriangles: (glb: Uint8Array, mode: string) => WasmResult;
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

/**
 * How far the morph-triangle pass may go for a part. Both change index data
 * only. `safe` rewrites a part only when no checked shape — rest, each
 * target and each pair of targets, at full and half weight, the default
 * pose — comes out visibly worse; it fixes less. `overall` rewrites whenever
 * the checked shapes improve overall, and the report shows any that got
 * worse.
 */
export type MorphTrianglesMode = "safe" | "overall";

/** Which shape of a part a {@link ShapeFold} measures. */
export type ShapeKind =
  | "rest"
  | "target"
  | "targetHalfway"
  | "blend"
  | "blendHalfway"
  | "defaultPose";

/** What the morph-triangle pass did with one part of a mesh. */
/**
 * `kept`: no triangulation folds noticeably less. `declined`: in safe mode,
 * one that folds less overall exists but would make some checked shape
 * visibly worse; overall mode would write it.
 */
export type MorphTrianglesOutcome =
  | "retriangulated"
  | "kept"
  | "declined"
  | "skipped";

/**
 * The area one shape's triangles draw with reversed winding, before and
 * after the pass, as a multiple of the shape's own area (above 1 when the
 * wrongly drawn area exceeds the shape, as with a pupil squashed to a thin
 * arc); `null` for a shape with no area. `shape` names it: `rest`, a
 * target's name, two names joined by ` + ` for a blend, either followed by
 * ` (halfway)` at half weight, or `default pose`; `kind` classifies it.
 */
export interface ShapeFold {
  shape: string;
  kind: ShapeKind;
  /**
   * Whether the rewrite changed this shape's wrongly drawn area by more
   * than safe mode's tolerance (1% of the part's largest shape), for the
   * worse or the better; both false when the part was not rewritten. This is
   * the rule safe mode applies, so judge "visibly worse" by it.
   */
  visiblyWorse: boolean;
  visiblyBetter: boolean;
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
 * data is rewritten. `mode` sets how far it may go. Throws when the input is
 * not a GLB.
 */
export function optimizeMorphTriangles(
  glb: Uint8Array,
  mode: MorphTrianglesMode,
): MorphTrianglesResult {
  const result = bindings().optimizeMorphTriangles(glb, mode);
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
 * The shapes, per part, still drawn with more reversed area than
 * `threshold` (relative to the shape's own area) after the pass. In a part
 * the pass rewrote or kept, no triangulation of its vertices follows such a
 * shape together with the part's other shapes, so reshaping the source
 * model is what fixes it; in a part safe mode `declined`, overall mode may
 * improve it. The caller chooses the threshold for its audience.
 */
export function stillFolding(
  report: MorphTrianglesReport,
  threshold: number,
): Array<{ part: MorphTrianglesPart; folds: ShapeFold[] }> {
  return report.parts
    .map((part) => ({
      part,
      folds: part.folds.filter((fold) => (fold.after ?? 0) > threshold),
    }))
    .filter((entry) => entry.folds.length > 0);
}
