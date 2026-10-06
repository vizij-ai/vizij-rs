/**
 * Stable ESM entrypoint for `@vizij/runtime`.
 *
 * Vizijs in the browser: one wasm module holding the Bevy view and the
 * Arora device (the `vizij` crate's `web` module). A page {@link mount}s its
 * canvas once — one App for the page's lifetime — then {@link loadVizij}s as
 * many Vizijs as it shows: each is a {@link Runtime} of its own, JS-paced,
 * drawn into the rectangle of the canvas the page {@link placeVizij}s it in.
 * Values cross the boundary in the normalized `ValueJSON` vocabulary shared
 * with the other Vizij packages.
 *
 * Typical use:
 * ```ts
 * import { init, mount, loadVizij, placeVizijIn, whenReady } from "@vizij/runtime";
 *
 * await init();
 * await mount("#vizijs");
 * const quori = await loadVizij("quori", glbBytes);
 * placeVizijIn("quori", document.getElementById("slot")!, canvas);
 * await whenReady("quori");
 * quori.run(); // the device paces itself from here on
 * quori.setValue(quori.path("standard/vizij/expression/happy"), 1);
 * const run = await quori.spawn({ id: SAY_ID, args: [...] });
 * ```
 *
 * A host with its own clock skips `run()` and calls `quori.step(dtMs)` per
 * animation frame instead. {@link startRuntime} gives a device with no Vizij
 * (a graph on a store, nothing drawn) — a bench, or a graph run in Node.
 *
 * An authoring page edits what it shows in place: {@link drainPicks}
 * reports presses on elements and misses, {@link setSelection} glows the
 * selected elements, {@link Runtime.hold} keeps rig outputs against the
 * device, {@link setStaticFeature} sets a static feature without a reload,
 * and a structural edit reloads the Vizij from its exported GLB bytes with
 * {@link loadVizij}, the previous scene drawing until the new one is ready.
 */
import { toValueJSON, type ValueJSON, type ValueInput } from "@vizij/value-json";
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";
import { play as defaultPlay, type Play } from "./audio.js";

/** A Vizij graph spec, as an object or already-serialized JSON. */
export type GraphSpecInput = object | string;

/**
 * A standard mapping Vizij ships — a graph that implements one profile in
 * terms of another, so a face responds to an external face standard (e.g.
 * ROS4HRI's keys onto Vizij's own controls). Listed by {@link mappings}; its
 * graph is fetched with {@link mapping}.
 */
export interface Mapping {
  id: string;
  title: string;
  description: string;
}

/** @deprecated Renamed {@link Mapping}: the graph was never a profile. */
export type StandardProfile = Mapping;

/**
 * One path in a profile, with its type and constraints. The shape is arora's
 * `KeyInfo` — `value_type` an arora type name (`f32`, `str`, `struct`, …),
 * `default_value` an arora value (`{ f32: 0 }`) — so a profile round-trips
 * through the same descriptor the WS registry and the standalone app speak.
 */
export interface ProfileKey {
  path: string;
  /** The key's role for the party implementing the profile: `input` for a key a caller commands. */
  kind?: string;
  value_type?: string;
  min?: number;
  max?: number;
  default_value?: unknown;
  /** Standard metadata: the FACS action unit, ARKit blendshape, and tier. */
  meta?: { au?: number; arkit?: string; tier?: string };
}

/**
 * Where a profile's paths live: `device` — absolute, one instance per device
 * (`standard/ros4hri/*`); `face` — relative to one face, addressed with its
 * rig prefix (`rig/<faceId>/`).
 */
export type ProfileScope = "device" | "face";

/**
 * A profile — an interface: the set of store paths one party exposes to
 * another, each with its type, range, and default. Listed by
 * {@link profiles}; fetched in full with {@link profile}.
 *
 * Distinct from a {@link Mapping}, the graph that implements one profile in
 * terms of another.
 */
export interface Profile {
  id: string;
  version: string;
  title: string;
  description: string;
  scope: ProfileScope;
  keys: ProfileKey[];
}

/** A profile as listed by {@link profiles} — the summary, without the keys. */
export interface ProfileSummary {
  id: string;
  version: string;
  title: string;
  description: string;
  scope: ProfileScope;
  /** How many paths the profile declares. */
  keys: number;
}

/**
 * A skill Vizij ships — a spawnable task-run behavior (a graph fragment the
 * device grafts per goal), served to ROS as a standard action (e.g. the
 * look_at gaze skill behind `/skill/look_at`). Listed by {@link skills}; its
 * canonical fragment is fetched with {@link skillSource}.
 */
export interface Skill {
  id: string;
  title: string;
  description: string;
  /** The skill's parameter names, in declared order — the fragment's
   * `task/<param>` inputs and the described signature's arguments. */
  parameters: string[];
}

/**
 * A spec-level graph edit — `{ upsert_nodes, remove_nodes, upsert_edges,
 * remove_edges }` in the Vizij spec vocabulary, or a JSON string of the same.
 * Every edge incident to an upserted node must appear in `upsert_edges`.
 */
export type GraphEditsInput = object | string;

/**
 * An Arora `Call`, as an object (or already-serialized JSON): the
 * `module_id` and function `id` it calls, and `args` as `{ id, value }`
 * pairs in the Arora `Value` vocabulary.
 */
export type RuntimeCall = object | string;

/** What a runtime call resolves to: the returned Arora `Value`. */
export interface RuntimeCallResult {
  ret: unknown;
  mutated?: unknown[];
}

/**
 * A task run, as {@link Runtime.spawn} and a task method's
 * {@link Runtime.invoke} resolve it — the handle any Arora client is given:
 * \`run\`, the run's id, which {@link Runtime.halt} takes; \`status\`, the key
 * holding its {@link RunStatus} (terminal once it ends); and the \`feedback\`
 * keys it updates while it runs, the \`result\` keys it writes when it ends,
 * and the \`update\` keys a client writes to steer it. Keys are store paths.
 */
export interface RunHandle {
  run: string;
  status: string;
  feedback: string[];
  result: string[];
  update: string[];
}

/** One of a Vizij's programs, as {@link describe} lists it. */
export interface Program {
  id: string;
  label: string | null;
  /** The program's graph spec: {@link behaviorValue} makes it
   * \`run_behavior\`'s \`behavior\` argument. */
  graph: object;
}

/**
 * The key prefix of the interpreter's \`run_behavior\` runs:
 * \`<BEHAVIOR_RUNS><run id>/status\` holds a run's {@link RunStatus} and
 * \`<BEHAVIOR_RUNS><run id>/name\` the name it runs under — a program's id
 * for a program. \`listKeys(BEHAVIOR_RUNS)\` lists every run the device has
 * started; an ended run's keys stay, its status terminal.
 */
export const BEHAVIOR_RUNS =
  "arora/tasks/61726f72-6100-0000-0000-000000000001/d5343617-a887-47da-82ea-9fcbe858f5c6/";

/** A task run's lifecycle status, read off its `status` key. */
export type RunStatus = "running" | "success" | "failure";

const RUN_STATUS_VARIANTS: Record<string, RunStatus> = {
  "acd79ec6-0c44-401a-82f8-5da5422d3eec": "running",
  "766e9e9a-446d-4e46-83e6-14b7ca101169": "success",
  "2468f46c-bb60-425c-9a4d-9ad326ccc7e2": "failure",
};

/**
 * The status a run's `status` key holds — the value `readValues` returns
 * for `handle.status` — or `undefined` while the key is unset or holds
 * something else. A halted run reads `failure`: it did not reach its goal.
 */
export function runStatus(value: unknown): RunStatus | undefined {
  const variant = (value as { enum?: { variant_id?: string } } | null)?.enum?.variant_id;
  return variant ? RUN_STATUS_VARIANTS[variant] : undefined;
}

/** A pointer press on a Vizij: the slot whose rectangle it was in, and the
 * element under the pointer by the id its GLB's RobotData declares — `null`
 * for a press that hit no element (a miss). */
export interface Pick {
  vizijId: string;
  elementId: string | null;
}

/** A rectangle of the canvas, in CSS pixels from its top-left corner. */
export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** How a Vizij's framed bounds fit its rectangle: whole and letterboxed
 * (`contain`), filling it and cropped (`cover`), or filling it and
 * distorted (`stretch`). */
export type Fit = "contain" | "cover" | "stretch";

/** Options for {@link mount}: how every Vizij is rendered. `fit` and `zoom`
 * are every Vizij's defaults; {@link setView} overrides them for one. */
export interface MountOptions {
  /** `RRGGBB` clear colour of each Vizij's rectangle; absent, the canvas
   * stays transparent wherever nothing is drawn. */
  background?: string;
  /** How a Vizij fits its rectangle: `contain` (default), `cover`, `stretch`. */
  fit?: Fit;
  /** Magnification after the fit: one factor, or `[x, y]`. */
  zoom?: number | [number, number];
  /** three.js-style ambient intensity (default π/2). */
  ambient?: number;
  /** Render pure albedo. */
  unlit?: boolean;
}

/** A world rectangle in the face's plane, the shape {@link describe} reports
 * the authored `rootBounds` in. */
export interface Bounds {
  center: { x: number; y: number };
  size: { x: number; y: number };
}

/**
 * A tone-mapping curve, by the names the authoring app stores in a bundle:
 * `none` (colors as shaded, the default), `agx` (Blender's AgX), `aces`
 * (ACES Filmic) or `neutral` (Khronos PBR Neutral) — three.js's curves.
 */
export type ToneMapping = "none" | "agx" | "aces" | "neutral";

/** One Vizij's view, for {@link setView}: its framing over the page's
 * {@link MountOptions}, and its tone mapping. */
export interface VizijView {
  /** The world rectangle the camera frames, in place of the GLB's authored
   * `rootBounds`. Positive sizes. */
  bounds?: Bounds;
  /** How the bounds fit the Vizij's rectangle; the page's when absent. */
  fit?: Fit;
  /** Magnification after the fit, one factor or `[x, y]`; the page's when
   * absent. */
  zoom?: number | [number, number];
  /** The curve the Vizij's colors go through; `none` when absent. */
  toneMapping?: ToneMapping;
}

/**
 * An Arora module as a device loads it: its header as JSON plus its wasm
 * executable — the two arguments of `arora-web`'s `withModule(headerJson,
 * executable)`, carried together. `@vizij/animation-module`'s
 * `loadAnimationModule()` returns one. Loaded as a guest, its functions are
 * reachable by id from {@link Runtime.call} and from the graph's
 * `ExternalFunction` nodes, like the host-linked modules'.
 */
export interface AroraModule {
  headerJson: string;
  wasmBytes: Uint8Array;
}

/** Options for {@link loadVizij}: the composition, as {@link composeVizij}'s,
 * the program that runs from load, whether the bundle's neutral pose is
 * staged at load (default `true`; the rest module's `reset` returns to it at
 * any time), the speech — `audio` is the playback hook the `say` provider hands
 * its audio to: the page's Web Audio player by default, `false` for a device
 * that plays no speech — and `speechApiUrl` the TTS deployment, and the wasm
 * modules to load into the device as guests. */
export interface VizijOptions extends Omit<ComposeVizijOptions, "animations" | "program"> {
  /** The program that runs from load: `"auto"` (the bundle's active program
   * — default), `"none"`, or a program id. It runs beside the face's graph
   * as a \`run_behavior\` run named after the program, as any client starts
   * one — not composed into the graph as {@link composeVizij} composes it —
   * so a page finds it under {@link BEHAVIOR_RUNS} and halts it like any
   * other run. */
  program?: string;
  /** Load the face's own animations (its bundle's `animations`) into its
   * device: each on a player named after it, its instance at weight 0 —
   * silent until a client gives it a weight and plays it. Default `true`;
   * `false` leaves the device's animation module empty. Unlike
   * {@link composeVizij}'s option of the same name, it does not choose
   * whether the animation source is composed: a Vizij always composes it. */
  animations?: boolean;
  stageNeutral?: boolean;
  audio?: Play | false;
  speechApiUrl?: string;
  modules?: AroraModule[];
}

/** A pose a face's bundle declares: a named set of rig input values, driven
 * through its weight at `path("poses/<id>.weight")`. */
export interface Pose {
  id: string;
  name: string | null;
  description: string | null;
  /** The ids of the {@link PoseGroup}s the pose belongs to, in the bundle's
   * order: its `groupIds`, its `groupId`, and the group whose `path` (or id)
   * its `group` names. */
  groupIds: string[];
  /** Rig input id → the value the pose sets it to at full weight. */
  values: Record<string, number>;
}

/** A group of poses a face's bundle declares — e.g. visemes or emotions. */
export interface PoseGroup {
  id: string;
  name: string | null;
  /** The group's path segment (`visemes`, `emotions`). */
  path: string | null;
}

/** An input of a face's rig, with its range and default — what a generic
 * slider or a face control is built from. */
export interface RigInput {
  id: string | null;
  /** Relative to the face's rig prefix, without a leading slash:
   * `runtime.path(input.path)` is its store key. */
  path: string;
  label: string | null;
  group: string | null;
  defaultValue: number | null;
  min: number | null;
  max: number | null;
}

/** A point of a {@link AnimationTrack}. */
export interface AnimationKeyframe {
  /** Seconds from the animation's start. */
  time: number;
  value: number;
  /** Overrides the track's interpolation for the segment from this point. */
  interpolation: string | null;
}

/** One animated channel of a {@link Animation}. */
export interface AnimationTrack {
  /** The rig-relative path the track drives (`gaze/left_right`,
   * `poses/<id>.weight`). */
  channel: string;
  /** `linear`, `step` or `cubic`, as authored. */
  interpolation: string | null;
  /** In time order. */
  keyframes: AnimationKeyframe[];
}

/**
 * An animation a face's bundle carries (an entry of its `animations`).
 * `@vizij/runtime`'s `Animation`, which shadows the DOM's Web Animations
 * `Animation` where it is imported.
 */
export interface Animation {
  id: string;
  name: string | null;
  /** Seconds: the animation's declared duration, or its last keyframe's time
   * when it declares none. */
  duration: number;
  tracks: AnimationTrack[];
}

/** What {@link describe} reads from a GLB without loading it. */
export interface VizijDescription {
  /** The `faceId` the GLB's bundle declares, if any. */
  faceId: string | null;
  /** The prefix the Vizij's own paths live under (`rig/<faceId>/`). */
  rigPrefix: string;
  /** The bounds the view frames: the authored `rootBounds`, or for a GLB
   * declaring none, its scene's bounding box in the XY plane; `null` for a
   * GLB without a mesh. */
  rootBounds: { center: { x: number; y: number }; size: { x: number; y: number } } | null;
  elements: {
    id: string;
    name: string;
    kind: string;
    material: string | null;
    morphTargets: string[];
  }[];
  /** animatable UUID → the node it drives and the feature (`translation`,
   * `rotation`, `scale`, `color`, `opacity`, …, a component of one by
   * Semio Studio's per-axis name — `translation.x`, `rotation.r`,
   * `color.g` — or a morph target's name). */
  animatables: Record<string, { node: string; feature: string }>;
  graphs: { kind: string }[];
  /** The motion-graph programs the Vizij can run: each one's id, its label
   * when it carries one, and its graph, as authored. */
  programs: Program[];
  /** The program the bundle boots playing: `metadata.activeMotionGraphId`,
   * or the first of `metadata.activeMotionGraphIds`. */
  activeProgramId: string | null;
  /** Rig input id → its neutral value. */
  neutralInputs: Record<string, number>;
  poses: Pose[];
  poseGroups: PoseGroup[];
  /** The inputs the rig declares. */
  rigInputs: RigInput[];
  animations: Animation[];
  /** The bundle's `metadata`, as authored — open-ended: `faceId`, the speech
   * configuration (`speechConfig`: `voice`, `visemeGroupId`,
   * `emotionGroupId`, …), `activeMotionGraphId` / `activeMotionGraphIds`,
   * exporter details. `null` when the bundle has none. */
  metadata: Record<string, unknown> | null;
}

/** An animation as the animation module's `load_animation` takes it, from
 * {@link Runtime.moduleAnimation}. */
export interface ModuleAnimation {
  id: string;
  name: string | null;
  /** Seconds. */
  duration: number;
  /** The module's `AnimationClip`, as an Arora `Value`: `load_animation`'s
   * `clip` argument. */
  moduleAnimation: object;
}

/** A key the device holds or describes, as {@link Runtime.listKeys} lists
 * it — as every Arora bridge lists keys: its path, and in \`__meta\` what the
 * device's store says of it (the shape it holds, its range and unit, the
 * value it rests at, whether a client may write it). */
export interface KeyInfo {
  path: string;
  __meta: Record<string, unknown>;
}

/** A method the device describes, as {@link Runtime.describeMethods} lists
 * it — as every Arora bridge lists methods — plus the module exporting it:
 * what {@link Runtime.invoke} calls by name. */
export interface MethodDescription {
  /** Its name. */
  path: string;
  /** Its parameters, in declaration order. */
  params?: { name: string; param_type: unknown; required: boolean }[];
  return_type?: unknown;
  description?: string;
  /** Whether invoking it starts a run (it returns a task status). */
  task: boolean;
  /** The exporting module's id: {@link Runtime.invoke}'s \`moduleId\` when
   * two modules export one name. */
  module: string;
}

interface WasmVizijRuntime {
  readonly vizijId: string;
  readonly rigPrefix: string;
  step(dt_ms: number): void;
  run(period_ms?: number): Promise<void>;
  stop(): void;
  readonly running: boolean;
  readonly behaviorError: string | undefined;
  behaviorErrorChanged(): Promise<string | undefined>;
  call(call_json: string): Promise<string>;
  invoke(method: string, args_json: string, module_id?: string): Promise<unknown>;
  spawn(call_json: string): Promise<RunHandle>;
  halt(run: string): Promise<void>;
  listKeys(prefix?: string): Promise<KeyInfo[]>;
  describeMethods(prefix?: string): Promise<MethodDescription[]>;
  loadGraph(graph_json: string): Promise<string>;
  applyGraphEdits(edits_json: string): Promise<string>;
  setValue(path: string, value_json: string): void;
  writeValues(values_json: string): void;
  readValues(paths: string[]): Record<string, ValueJSON | null>;
  snapshot(): Record<string, ValueJSON>;
  drainChanges(): Record<string, ValueJSON | null>;
  moduleAnimation(animation_json: string): ModuleAnimation;
  hold(paths: string[]): void;
  release(paths?: string[]): void;
  free(): void;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  VizijRuntime: {
    fromGraph(graph_json?: string, modules?: AroraModule[]): WasmVizijRuntime;
  };
  mount(canvas: string, options_json?: string): void;
  loadVizij(
    vizij_id: string,
    glb: Uint8Array,
    options_json?: string,
    play?: Play,
    modules?: AroraModule[],
  ): WasmVizijRuntime;
  unloadVizij(vizij_id: string): void;
  placeVizij(vizij_id: string, x: number, y: number, width: number, height: number): void;
  fillCanvas(vizij_id: string): void;
  setView(vizij_id: string, view_json?: string): void;
  safeArea(vizij_id: string): Rect | null;
  ready(vizij_id: string): boolean;
  drainPicks(): Pick[];
  setSelection(vizij_id: string, element_ids: string[]): void;
  setStaticFeature(vizij_id: string, element_id: string, feature: string, value_json: string): void;
  describe(glb: Uint8Array): VizijDescription;
  memoryBytes(): number;
  mappings(): Mapping[];
  mapping(id: string, rig_prefix: string): object | null;
  profiles(): ProfileSummary[];
  profile(id: string, rig_prefix: string): Profile | null;
  skills(): Skill[];
  skillSource(id: string): object | null;
  composeVizij(gltf_json: string, options_json?: string): object;
  behaviorValue(graph_json: string): object;
  runEdits(run: string, from_json: string, to_json: string): object;
}

const bindingCache: { current: WasmBindings | null } = { current: null };
let wasmModulePromise: Promise<unknown> | null = null;
let wasmUrlCache: string | null = null;

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

function pkgWasmJsUrl(): URL {
  return new URL("../../pkg/vizij.js", import.meta.url);
}

function importStaticWasmModule(): Promise<unknown> {
  return import("../../pkg/vizij.js");
}

function importDynamicWasmModule(): Promise<unknown> {
  return import(/* @vite-ignore */ pkgWasmJsUrl().toString());
}

async function importWasmModule(): Promise<unknown> {
  if (!wasmModulePromise) {
    wasmModulePromise = importStaticWasmModule().catch((err) => {
      if (typeof console !== "undefined" && typeof console.warn === "function") {
        console.warn(
          "@vizij/runtime: static wasm import failed, falling back to runtime URL import.",
          err,
        );
      }
      return importDynamicWasmModule();
    });
  }
  return wasmModulePromise;
}

function defaultWasmUrl(): string {
  if (!wasmUrlCache) {
    wasmUrlCache = new URL("../../pkg/vizij_bg.wasm", import.meta.url).toString();
  }
  return wasmUrlCache;
}

const loadBindingsImpl =
  typeof window === "undefined"
    ? loadWasmBindings
    : (loadWasmBindingsBrowser as typeof loadWasmBindings);

async function loadBindings(input?: LoaderInitInput): Promise<WasmBindings> {
  await loadBindingsImpl<WasmBindings>(
    {
      cache: bindingCache,
      importModule: () => importWasmModule(),
      defaultWasmUrl,
      init: async (module, initArg) => {
        await (module as WasmBindings).default(toWasmBindgenInitOptions(initArg));
      },
      getBindings: (module) => module as WasmBindings,
    },
    input,
  );
  return bindingCache.current!;
}

export type InitInput = LoaderInitInput;

let _initPromise: Promise<void> | null = null;

/**
 * Initialize the wasm module once. The returned promise is memoized; await it
 * during startup before calling `startRuntime`.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
  })();
  return _initPromise;
}

/**
 * A Vizij's device — or, from {@link startRuntime}, a device with no Vizij.
 * Its surface is the device's own, as any client of an Arora device has it
 * (a bridge client, Semio Studio): {@link call}, {@link invoke},
 * {@link spawn} and {@link halt}, {@link listKeys} and
 * {@link describeMethods}, and the store's reads and writes. What the face
 * does — its skills, its programs, its animations, its rest — it does
 * through the methods its modules and its interpreter declare, by name:
 *
 * ```ts
 * await quori.invoke("say", { text: "Hello" });                // a run's RunHandle
 * await quori.invoke("run_behavior", { name: "live", behavior: await behaviorValue(graph) });
 * await quori.invoke("reset");                                  // every key back to rest
 * await quori.invoke("reset_keys", { keys: { strs: outputKeysOfMyGraph } });
 * ```
 *
 * The graph it runs reads and writes the same keys. A Vizij's paths live
 * under its {@link rigPrefix}; {@link path} builds one.
 */
export class Runtime {
  private inner: WasmVizijRuntime;

  constructor(inner: WasmVizijRuntime) {
    this.inner = inner;
  }

  /** The slot the Vizij is shown under; empty for a device with no Vizij. */
  get vizijId(): string {
    return this.inner.vizijId;
  }

  /** The prefix the Vizij's own paths live under (`rig/<faceId>/`), empty
   * when the GLB's bundle names no `faceId`. */
  get rigPrefix(): string {
    return this.inner.rigPrefix;
  }

  /** A path of the Vizij's own: `rigPrefix + relative`. */
  path(relative: string): string {
    return this.inner.rigPrefix + relative;
  }

  /**
   * Advance the device one tick. `dtMs` is the wall time since the previous
   * step in milliseconds (the difference of two `requestAnimationFrame`
   * timestamps). Unavailable while `run()` owns the device.
   */
  step(dtMs: number): void {
    this.inner.step(dtMs);
  }

  /**
   * Hand the device its own loop at `periodMs` (default ~100 Hz) until
   * {@link stop}. The rest of this surface keeps working meanwhile. A
   * failing behavior tick does **not** end the loop — it stands as
   * `behaviorError` until a tick recovers; the returned promise rejects only
   * if the runtime itself fails, and resolves on `stop()`.
   */
  run(periodMs?: number): Promise<void> {
    return this.inner.run(periodMs);
  }

  /** Reclaim the device from `run()`: the loop ends at its next step
   * boundary and `step()` works again. */
  stop(): void {
    this.inner.stop();
  }

  /** Whether `run()` currently owns the device. */
  get running(): boolean {
    return this.inner.running;
  }

  /**
   * The behavior's standing error — the message of its latest failed tick,
   * `undefined` while the behavior is healthy or none is installed. A
   * failing tick does not stop the device or its `run()` loop; the reading
   * stays available throughout.
   */
  get behaviorError(): string | undefined {
    return this.inner.behaviorError;
  }

  /**
   * Resolves on the next change of the standing behavior error, with the new
   * reading: a message when a distinct failure appears, `undefined` when a
   * tick recovers. Sequential awaits share one cursor, so no change is
   * missed between them; one await may be pending at a time. Rejects when
   * the device is gone.
   */
  behaviorErrorChanged(): Promise<string | undefined> {
    return this.inner.behaviorErrorChanged();
  }

  /**
   * Call a module function by id: an Arora `Call` naming its `module_id`
   * and function `id`, its `args` as `{ id, value }` pairs. Every operation
   * of this surface that reaches the device is enqueued before it returns
   * and applied inside the device's **next** step, so its promise resolves
   * only after that step runs: under `run()` just `await` it; a direct
   * driver calls {@link step} after issuing it.
   */
  call(call: RuntimeCall): Promise<RuntimeCallResult> {
    const json = typeof call === "string" ? call : JSON.stringify(call);
    return this.inner.call(json).then((result) => JSON.parse(result) as RuntimeCallResult);
  }

  /**
   * Call a method the device describes ({@link describeMethods}) by name,
   * its arguments by parameter name in any `ValueInput` form — Arora's
   * by-name invoke, as a bridge client sends it. A method that returns a
   * task status starts a run, and the promise resolves to its
   * {@link RunHandle}; any other resolves to its return value (an Arora
   * `Value`, `undefined` for none). An argument the method does not declare
   * fails the call. A name two modules export needs `moduleId` to choose
   * one.
   */
  invoke(
    method: string,
    args: Record<string, ValueInput> = {},
    moduleId?: string,
  ): Promise<unknown> {
    const normalized: Record<string, ValueJSON> = {};
    for (const [parameter, value] of Object.entries(args)) {
      normalized[parameter] = toValueJSON(value);
    }
    try {
      return this.inner.invoke(method, JSON.stringify(normalized), moduleId);
    } catch (e) {
      return Promise.reject(e);
    }
  }

  /**
   * Start a task run of `call` — an Arora `Call` naming a task method the
   * device describes — through the interpreter module's SPAWN. Resolves to
   * the run's {@link RunHandle}. {@link invoke} does the same by name.
   */
  spawn(call: RuntimeCall): Promise<RunHandle> {
    return this.inner.spawn(typeof call === "string" ? call : JSON.stringify(call));
  }

  /** Halt run `run` — its {@link RunHandle}'s `run` — through the
   * interpreter module's HALT. Resolves once applied: the run's status key
   * then reads `failure`. A run's outputs hold their last values; returning
   * them to rest is the client's step (`invoke("reset_keys", { keys: { strs: keys } })`). */
  halt(run: string): Promise<void> {
    return this.inner.halt(run);
  }

  /** The keys the device holds or describes, under `prefix` when given,
   * each with what its store says of it — Arora's ListKeys. A run's keys
   * are listed under {@link BEHAVIOR_RUNS} and its kin. */
  listKeys(prefix?: string): Promise<KeyInfo[]> {
    return this.inner.listKeys(prefix);
  }

  /** The methods the device describes, under `prefix` when given, with
   * their parameters and return type — Arora's DescribeMethods: what
   * {@link invoke} calls by name. */
  describeMethods(prefix?: string): Promise<MethodDescription[]> {
    return this.inner.describeMethods(prefix);
  }

  /**
   * Replace the device's running graph **in place**: the spec reaches the
   * interpreter as the interpreter module's LOAD, so the store, the modules
   * and the device survive the swap, and the runs keep running. Applied at
   * the device's next step, which the promise waits for.
   */
  loadGraph(graph: GraphSpecInput): Promise<void> {
    const json = typeof graph === "string" ? graph : JSON.stringify(graph);
    return this.inner.loadGraph(json).then(() => undefined);
  }

  /**
   * Edit the running graph **in place** — the interpreter module's EDIT —
   * with a spec-level graph diff (`upsert_nodes` / `remove_nodes` /
   * `upsert_edges` / `remove_edges`): the nodes it leaves alone keep their
   * runtime state. {@link runEdits} builds the edits that change a run's
   * behavior. Applied at the device's next step, which the promise waits
   * for.
   */
  applyGraphEdits(edits: GraphEditsInput): Promise<void> {
    const json = typeof edits === "string" ? edits : JSON.stringify(edits);
    return this.inner.applyGraphEdits(json).then(() => undefined);
  }

  /** Write one key, in any `ValueInput` form. */
  setValue(path: string, value: ValueInput): void {
    this.inner.setValue(path, JSON.stringify(toValueJSON(value)));
  }

  /** Write several keys as one store change. */
  writeValues(values: Record<string, ValueInput>): void {
    const normalized: Record<string, ValueJSON> = {};
    for (const [path, value] of Object.entries(values)) {
      normalized[path] = toValueJSON(value);
    }
    this.inner.writeValues(JSON.stringify(normalized));
  }

  /** Read keys: each path maps to its value, or `null` when unset. */
  readValues(paths: string[]): Record<string, ValueJSON | null> {
    return this.inner.readValues(paths);
  }

  /** Every key the store holds, with its value. */
  snapshot(): Record<string, ValueJSON> {
    return this.inner.snapshot();
  }

  /** The keys that changed since the previous drain (`null` = cleared); the
   * first drain returns the whole current state. */
  drainChanges(): Record<string, ValueJSON | null> {
    return this.inner.drainChanges();
  }

  /**
   * `animation` — one of {@link describe}'s `animations`, or a page's own in
   * that shape — as the animation module's `load_animation` takes it: its
   * id, name and duration (seconds), and `moduleAnimation`, the module's
   * `AnimationClip` with each track keyed by the store key its channel
   * names through this Vizij's rig, as the Vizij's own animations are
   * loaded. Pass it on: `invoke("load_animation", { clip:
   * moduleAnimation })`.
   */
  moduleAnimation(animation: Animation): ModuleAnimation {
    return this.inner.moduleAnimation(JSON.stringify(animation));
  }

  /**
   * Hold rig outputs of the Vizij against this device — the inspector's
   * lock. Each path is an animatable id (bare, or as its rig path
   * `rig/<faceId>/<id>`), held whole, or `<id>:<component>` — `x`, `y`, `z`
   * of a vector or euler, `r`, `g`, `b` of a colour — held alone while the
   * other components follow the device. A held output keeps the value the
   * Vizij shows while the device writes on; the device and its store are
   * untouched, so {@link release} shows the device's current value at once.
   * Holds are kept per Vizij across its reloads, and dropped by
   * {@link unloadVizij}.
   */
  hold(paths: string[]): void {
    this.inner.hold(paths);
  }

  /** Let held outputs follow the device again: `paths` as {@link hold}'s,
   * each released as it was held (releasing an animatable leaves its held
   * components held); omitted, every hold of the Vizij is released. */
  release(paths?: string[]): void {
    this.inner.release(paths);
  }

  /** Release the wasm-side device. The instance is unusable afterwards; a
   * Vizij's device is disposed after {@link unloadVizij}. */
  dispose(): void {
    this.inner.free();
  }
}


function bindings(): WasmBindings {
  const current = bindingCache.current;
  if (!current) {
    throw new Error("@vizij/runtime: call init() first");
  }
  return current;
}

/**
 * Create the page's one App over `canvas` — a CSS selector, or the canvas
 * element itself (given an id if it has none). The canvas fits its parent
 * and stays transparent wherever no Vizij draws. Calls {@link init} if it
 * has not run yet. A page mounts once.
 */
export async function mount(
  canvas: string | HTMLCanvasElement,
  options?: MountOptions,
  input?: InitInput,
): Promise<void> {
  await init(input);
  let selector: string;
  if (typeof canvas === "string") {
    selector = canvas;
  } else {
    if (!canvas.id) {
      canvas.id = `vizij-${Math.random().toString(36).slice(2)}`;
    }
    selector = `#${canvas.id}`;
  }
  bindings().mount(selector, options ? JSON.stringify(options) : undefined);
}

/**
 * Show a Vizij under `vizijId` and start its device: the GLB's bindings and
 * bundle are read, its graphs composed (`options` as {@link composeVizij}'s,
 * plus `stageNeutral`), the device built with the animation, rest, gaze and
 * viseme modules, and the scene queued for the App — {@link whenReady} resolves
 * once it shows. Requires {@link mount}.
 *
 * A Vizij already shown under `vizijId` is replaced — how an authoring page
 * reloads a structurally edited Vizij from its exported GLB bytes: the
 * previous scene keeps drawing until the new one is ready (so
 * {@link ready} reads false meanwhile), its placement, view, selection and
 * holds carry over, and the page disposes the previous {@link Runtime}.
 */
export async function loadVizij(
  vizijId: string,
  glb: Uint8Array | ArrayBuffer,
  options?: VizijOptions,
  input?: InitInput,
): Promise<Runtime> {
  await init(input);
  const bytes = glb instanceof Uint8Array ? glb : new Uint8Array(glb);
  const { audio, modules, ...rest } = options ?? {};
  const play = audio === false ? undefined : (audio ?? defaultPlay);
  return new Runtime(
    bindings().loadVizij(vizijId, bytes, options ? JSON.stringify(rest) : undefined, play, modules),
  );
}

/** Take the Vizij down: its scene, its camera, its GLB, its selection and
 * holds. Dispose its {@link Runtime} afterwards. */
export function unloadVizij(vizijId: string): void {
  bindings().unloadVizij(vizijId);
}

/** Confine the Vizij's camera to a rectangle of the canvas (CSS pixels from
 * its top-left corner) — how several Vizijs share one canvas. Kept across the
 * Vizij's reloads. */
export function placeVizij(vizijId: string, rect: Rect): void {
  bindings().placeVizij(vizijId, rect.x, rect.y, rect.width, rect.height);
}

/** Place the Vizij over `element`, as it lies over `canvas` on the page —
 * call it again when the layout changes. */
export function placeVizijIn(vizijId: string, element: Element, canvas: Element): void {
  const c = canvas.getBoundingClientRect();
  const e = element.getBoundingClientRect();
  placeVizij(vizijId, { x: e.x - c.x, y: e.y - c.y, width: e.width, height: e.height });
}

/** Give the Vizij's camera the whole canvas again. */
export function fillCanvas(vizijId: string): void {
  bindings().fillCanvas(vizijId);
}

/**
 * Frame and tone-map one Vizij: `view`'s framing fields override the
 * page's {@link MountOptions} for this Vizij alone, its `bounds` the GLB's
 * authored `rootBounds`. Each call replaces the Vizij's previous view — a
 * field left out takes the page's (tone mapping: `none`), and `{}` hands the
 * Vizij back to the page. Kept across the Vizij's reloads; may be set before
 * it loads. Shows from the next frame.
 */
export function setView(vizijId: string, view: VizijView): void {
  bindings().setView(vizijId, JSON.stringify(view));
}

/**
 * Where the Vizij's safe area — the bounds its camera frames — lies on the
 * canvas, in CSS pixels from the canvas's top-left corner (the frame
 * {@link placeVizij} takes), as of the last frame drawn: what a DOM overlay
 * outlining it is positioned by. A zoom that crops the bounds reaches past
 * the Vizij's rectangle. `null` until the Vizij's camera has drawn.
 */
export function safeArea(vizijId: string): Rect | null {
  return bindings().safeArea(vizijId);
}

/** Whether the Vizij's scene has spawned and its bindings are joined — from
 * then on its device's pose shows. False while a reload's new scene loads. */
export function ready(vizijId: string): boolean {
  return bindings().ready(vizijId);
}

/** Resolves once {@link ready} is true for the Vizij; rejects after
 * `timeoutMs` (default 60 s). */
export function whenReady(vizijId: string, timeoutMs = 60_000): Promise<void> {
  return new Promise((resolve, reject) => {
    const started = Date.now();
    const poll = () => {
      if (ready(vizijId)) {
        resolve();
      } else if (Date.now() - started > timeoutMs) {
        reject(new Error(`@vizij/runtime: Vizij ${vizijId} not ready after ${timeoutMs} ms`));
      } else {
        setTimeout(poll, 50);
      }
    };
    poll();
  });
}

/**
 * The pointer presses on Vizijs since the last drain, oldest first: each
 * names the Vizij whose rectangle it was in and the element under the
 * pointer, or `null` for a press that hit no element — what clears an
 * authoring selection. The element is found over its mesh as drawn, morphs
 * applied, the nearest along the ray. A press outside every Vizij's
 * rectangle is not reported.
 */
export function drainPicks(): Pick[] {
  return bindings().drainPicks();
}

/**
 * Glow the outlines of the Vizij's elements `elementIds` (the ids its
 * RobotData declares, as {@link describe} lists them), replacing the
 * previous selection; an empty list clears it. The outline is each mesh's
 * feature edges, drawn over the face in the selection red; a selected group
 * glows every element mesh under it. Kept across the Vizij's reloads.
 */
export function setSelection(vizijId: string, elementIds: string[]): void {
  bindings().setSelection(vizijId, elementIds);
}

/**
 * Set a static feature of one of the Vizij's elements in place — no export,
 * no reload — as an authoring page does while it scrubs one. `feature` is
 * the RobotData feature name: `translation`, `rotation` (an euler,
 * radians), `scale` (`{ x, y, z }`, or a number scaling evenly), `color`,
 * `emissive` (linear `{ r, g, b }`), `opacity`, `metalness`,
 * `emissiveIntensity` (numbers), one component of a triple by Semio
 * Studio's per-axis name (`translation.x`, `rotation.r`, `color.g`: a
 * number), or one of the element's morph target names (a weight) — the
 * value as RobotData stores a static feature, or any
 * `ValueInput`. Applied once the Vizij is ready; a reload shows what its GLB
 * carries. A feature a device output drives is the device's again at its
 * next write.
 */
export function setStaticFeature(
  vizijId: string,
  elementId: string,
  feature: string,
  value: ValueInput,
): void {
  bindings().setStaticFeature(vizijId, elementId, feature, JSON.stringify(toValueJSON(value)));
}

/** The module's linear memory in bytes; it never shrinks, so a flat reading
 * across Vizij loads and unloads is what "nothing leaks" looks like. */
export function memoryBytes(): number {
  return bindings().memoryBytes();
}

/** What a GLB declares — its elements, animatables, bounds, graphs and
 * programs, and its bundle's poses, rig inputs, animations and metadata —
 * without loading it. Calls {@link init} if it has not run yet. */
export async function describe(
  glb: Uint8Array | ArrayBuffer,
  input?: InitInput,
): Promise<VizijDescription> {
  await init(input);
  const bytes = glb instanceof Uint8Array ? glb : new Uint8Array(glb);
  return bindings().describe(bytes);
}

/**
 * A device with no Vizij: `graph` (a graph spec, in any form the spec
 * normalizer accepts) as its behavior over a fresh store and rig, its free
 * inputs resting at their authored defaults (the rest module's `reset`), the
 * animation and rest modules host-linked and `modules` loaded as guests.
 * Nothing is drawn — a bench, or a graph run in Node. Omit `graph` for the built-in
 * passthrough proof graph (`sensor/x` → `actuator/y`). Calls {@link init}
 * if it has not run yet.
 */
export async function startRuntime(
  graph?: GraphSpecInput,
  input?: InitInput,
  modules?: AroraModule[],
): Promise<Runtime> {
  await init(input);
  const graphJson =
    graph === undefined ? undefined : typeof graph === "string" ? graph : JSON.stringify(graph);
  return new Runtime(bindings().VizijRuntime.fromGraph(graphJson, modules));
}

/**
 * The standard mappings Vizij ships (ROS4HRI, …) — the introspectable list an
 * authoring app offers for opt-in. Calls {@link init} if it has not run yet.
 */
export async function mappings(input?: InitInput): Promise<Mapping[]> {
  await init(input);
  return bindings().mappings();
}

/**
 * A standard mapping's graph as a spec object, ready to compose into a
 * running device's graph or to embed into a face GLB. `rigPrefix` (e.g.
 * `"rig/quori_latest/"`) is prepended to the control paths the mapping
 * writes; omit it for the unprefixed graph. `null` for an unknown id (see
 * {@link mappings}).
 */
export async function mapping(
  id: string,
  rigPrefix = "",
  input?: InitInput,
): Promise<object | null> {
  await init(input);
  return bindings().mapping(id, rigPrefix);
}

/** @deprecated Renamed {@link mappings}: the graphs it lists are mappings, not profiles. */
export const standardProfiles = mappings;

/** @deprecated Renamed {@link mapping}: the graph it returns is a mapping, not a profile. */
export const standardProfile = mapping;

/**
 * The profiles Vizij ships — a *profile* being an interface, the set of store
 * paths and their types a face's graphs are authored against. The list an
 * authoring app's import picker offers. Calls {@link init} if it has not run
 * yet.
 */
export async function profiles(input?: InitInput): Promise<ProfileSummary[]> {
  await init(input);
  return bindings().profiles();
}

/**
 * One profile in full — every path it declares, with type, range, default and
 * standard metadata. `rigPrefix` (e.g. `"rig/quori_latest/"`) addresses a
 * face-scoped profile to one face's store; a device-scoped profile comes back
 * as is. Omit it for the portable form. `null` for an unknown id (see
 * {@link profiles}).
 */
export async function profile(
  id: string,
  rigPrefix = "",
  input?: InitInput,
): Promise<Profile | null> {
  await init(input);
  return bindings().profile(id, rigPrefix);
}

/**
 * The skills Vizij ships (the look_at gaze skill, …) — the introspectable
 * list an authoring app's Skills menu and a device's actions view offer.
 * Calls {@link init} if it has not run yet.
 */
export async function skills(input?: InitInput): Promise<Skill[]> {
  await init(input);
  return bindings().skills();
}

/**
 * A skill's canonical fragment graph as a spec object, ready to embed into a
 * face GLB as its `skill::<id>` override of the shipped behavior. Skill
 * fragments are face-independent by construction (their placeholder `task/*`
 * paths are rewritten per run), so there is no rig prefix to apply or strip.
 * `null` for an unknown id (see {@link skills}).
 */
export async function skillSource(id: string, input?: InitInput): Promise<object | null> {
  await init(input);
  return bindings().skillSource(id);
}

/** Options for {@link composeVizij}; every field falls back to the native
 * `vizij` app's deploy default. */
export interface ComposeVizijOptions {
  /** Base bundle graph kinds to compose. Default: `rig`, `pose-driver`,
   * `pose`, `standard-adaptation`. */
  graphs?: string[];
  /** `"auto"` (the bundle's active program — default), `"none"`, or a
   * program id. */
  program?: string;
  /** Compose the built-in ROS4HRI mapping (an embedded copy still wins).
   * Default `true`. */
  ros4hri?: boolean;
  /** Compose the animation source. Default `false`; every device this
   * package builds links the animation module, so the source dispatches. */
  animations?: boolean;
}

/**
 * The composed behavior graph of a face bundle — the composition the native
 * `vizij` app deploys: the bundle's base graphs, its embedded standard
 * mappings (each suppressing the built-in of the same id — an embedded copy
 * is the author's pinned override), the built-in ROS4HRI mapping unless opted
 * out, then the selected program. `gltf` is the GLB's glTF JSON document. The
 * returned spec feeds {@link startRuntime} or {@link Runtime.loadGraph}, so an
 * exported GLB can be deployed and verified without the native app.
 */
export async function composeVizij(
  gltf: object,
  options?: ComposeVizijOptions,
  input?: InitInput,
): Promise<object> {
  await init(input);
  return bindings().composeVizij(
    JSON.stringify(gltf),
    options ? JSON.stringify(options) : undefined,
  );
}

/**
 * `graph` (a graph spec, in any form the spec normalizer accepts) as
 * \`run_behavior\`'s \`behavior\` argument: the graph as the interpreter
 * module's LOAD carries one, as an Arora \`Value\`.
 * \`runtime.invoke("run_behavior", { name, behavior })\` runs it beside the
 * device's graph until halted. Calls {@link init} if it has not run yet.
 */
export async function behaviorValue(graph: GraphSpecInput, input?: InitInput): Promise<object> {
  await init(input);
  return bindings().behaviorValue(typeof graph === "string" ? graph : JSON.stringify(graph));
}

/**
 * The graph edits that change run \`run\`'s behavior in place, from \`from\`
 * — the graph it runs: what it was spawned with, or the \`to\` of the edit
 * before — to \`to\`: what {@link Runtime.applyGraphEdits} takes. A run's
 * nodes live in the device's graph under ids its interpreter derives from
 * the run; these edits name them, so the nodes \`to\` keeps keep their
 * runtime state. Calls {@link init} if it has not run yet.
 */
export async function runEdits(
  run: string,
  from: GraphSpecInput,
  to: GraphSpecInput,
  input?: InitInput,
): Promise<object> {
  await init(input);
  const json = (graph: GraphSpecInput) => (typeof graph === "string" ? graph : JSON.stringify(graph));
  return bindings().runEdits(run, json(from), json(to));
}

export { ANIMATION_PLAYERS_PATH, decodePlayerStates } from "./animations.js";
export type { PlayerState, InstanceState } from "./animations.js";
export { unlockAudio, play as playAudio, IDLE_STOP_MS } from "./audio.js";
export type { Play, SpeechMark } from "./audio.js";
export { toValueJSON } from "@vizij/value-json";
export type { ValueJSON, ValueInput } from "@vizij/value-json";
