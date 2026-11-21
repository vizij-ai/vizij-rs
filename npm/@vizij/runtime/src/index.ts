/**
 * Stable ESM entrypoint for `@vizij/runtime`.
 *
 * Vizijs in the browser: one wasm module holding the Bevy view and the
 * Arora device (the `vizij` crate's `web` module). A page {@link mount}s its
 * canvas once — one App for the page's lifetime — and loads a Vizij into
 * each instance it shows, an id of its own, through the {@link VizijHandle}
 * that resolves: each face's device is a {@link Runtime} of its own,
 * JS-paced, and each face draws into the rectangle of the canvas the page
 * places it in. The App reports through one handler ({@link VizijEvent}):
 * a load's answer, a face drawn, a click, a safe area moving. Values cross
 * the boundary in the normalized `ValueJSON` vocabulary shared with the
 * other Vizij packages.
 *
 * Typical use:
 * ```ts
 * import { mount } from "@vizij/runtime";
 *
 * const vizijs = await mount("#vizijs");
 * vizijs.onEvent((event) => {
 *   if (event.type === "ready") console.log(`${event.instance} drawn`);
 * });
 * const quori = await vizijs.load("quori", glbBytes, { key: assetId });
 * vizijs.placeIn("quori", document.getElementById("slot")!);
 * quori.run(); // the device paces itself from the face's first drawn frame
 * quori.setValue(quori.path("standard/vizij/expression/happy"), 1);
 * const run = await quori.spawn({ id: SAY_ID, args: [...] });
 * ```
 *
 * A host with its own clock skips `run()` and calls `quori.step(dtMs)` per
 * animation frame instead. {@link startRuntime} gives a device with no Vizij
 * (a graph on a store, nothing drawn) — a bench, or a graph run in Node.
 *
 * An authoring page edits what it shows in place: `pick` events report
 * clicks on elements and misses, {@link VizijHandle.setSelection} outlines
 * the selected elements, {@link Runtime.hold} keeps rig outputs against the
 * device, {@link Runtime.holdKeys} keeps the store keys it edits against
 * the Vizij's behavior, {@link VizijHandle.setValue} writes a static
 * feature without a reload, and a structural edit reloads the Vizij from
 * its exported GLB bytes by loading them into the same instance, the face
 * showing drawing until the new one is ready.
 */
import { toValueJSON, type ValueJSON, type ValueInput } from "@vizij/value-json";
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";
import { play as defaultPlay, type Play } from "./audio.js";
import { trackFaces, type Rect, type ShownFace, type VizijEvent } from "./faces.js";
import { countDownload, nextPaint, reportProgress } from "./progress.js";

export { onInitProgress, type InitProgress } from "./progress.js";
export type { FaceElement, Rect, ShownFace, VizijEvent } from "./faces.js";
export type * from "./controls.js";
import type { ControlCommand } from "./controls.js";

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
   * \`run_behavior\`'s \`behavior\` argument, and {@link outputKeys} lists
   * the keys its run leaves holding when halted. */
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

/** How a Vizij's framed bounds fit its rectangle: whole and letterboxed
 * (`contain`), filling it and cropped (`cover`), or filling it and
 * distorted (`stretch`). */
export type Fit = "contain" | "cover" | "stretch";

/** Options for {@link mount}: how every Vizij is rendered. `fit` and `zoom`
 * are every Vizij's defaults; {@link VizijHandle.setView} overrides them for one. */
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

/** One Vizij's view, for {@link VizijHandle.setView}: its framing over the page's
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
 * executable)`, carried together: a module directory's `header.json` and
 * guest. Loaded as a guest, its functions are
 * reachable by id from {@link Runtime.call} and from the graph's
 * `ExternalFunction` nodes, like the host-linked modules'.
 */
export interface AroraModule {
  headerJson: string;
  wasmBytes: Uint8Array;
}

/** Options for {@link VizijHandle.load}: the composition, as {@link composeVizij}'s,
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
  /** The slope a `cubic` segment ending here arrives with, in value units
   * per second (glTF's `CUBICSPLINE` in-tangent); without one, the
   * segment's chord slope. */
  inTangent?: number | null;
  /** The slope a `cubic` segment starting here leaves with, in value units
   * per second; without one, the segment's chord slope. */
  outTangent?: number | null;
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
  /** The face's hardware version, its bundle's `metadata.hardwareVersion`:
   * the version of the face itself — its shapes, what can move in them and
   * the layout giving those movements their meaning — such as `quori-1.3`.
   * `null` when the bundle states none (or a blank one); a device showing
   * the face then reports its `faceId` as its hardware version. */
  hardwareVersion: string | null;
  /** The face's software version, its bundle's `metadata.softwareVersion`:
   * the version of what the face runs — its profiles, exposed inputs,
   * programs, extra rigs and animations — such as `1.2`. `null` when the
   * bundle states none (or a blank one). */
  softwareVersion: string | null;
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
  /** animatable id → the nodes it drives and the feature (`translation`,
   * `rotation`, `scale`, `color`, `opacity`, …, a component of one by
   * Semio Studio's per-axis name — `translation.x`, `rotation.r`,
   * `color.g` — or a morph target's name). `nodes` lists every node, in the
   * GLB's order: shapes that share a material share its colour and opacity
   * animatables. `node` is the first of them. */
  animatables: Record<string, { node: string; nodes: string[]; feature: string }>;
  /** The bundle's graph entries — its rig, pose driver, programs, … — each
   * with the `id` it carries (`null` for one without) and its `kind`. */
  graphs: { id: string | null; kind: string }[];
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

/** An animation as the animation module's `load_clip` takes it, from
 * {@link Runtime.moduleAnimation}. */
export interface ModuleAnimation {
  id: string;
  name: string | null;
  /** Seconds. */
  duration: number;
  /** The module's `Clip` (format 2.0), as an Arora `Value`: `load_clip`'s
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
  readonly instance: string;
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
  holdKeys(keys: string[]): void;
  releaseKeys(keys?: string[]): void;
  free(): void;
}

interface WasmBindings {
  default: (input?: unknown) => Promise<unknown>;
  VizijRuntime: {
    fromGraph(graph_json?: string, modules?: AroraModule[]): WasmVizijRuntime;
  };
  mount(canvas: string, options_json?: string): void;
  setEventHandler(handler: (event: VizijEvent) => void): void;
  load(
    instance: string,
    glb: Uint8Array,
    options_json?: string,
    play?: Play,
    modules?: AroraModule[],
  ): WasmVizijRuntime;
  unload(instance: string): void;
  place(instance: string, rect: Float64Array): void;
  setView(instance: string, view_json?: string): void;
  setSelection(instance: string, elements: string[]): void;
  setValue(instance: string, target: string, feature: string, values: Float32Array): void;
  control(command: string): void;
  describe(glb: Uint8Array): VizijDescription;
  memoryBytes(): number;
  mappings(): Mapping[];
  mapping(id: string, rig_prefix: string): object | null;
  profiles(): ProfileSummary[];
  profile(id: string, rig_prefix: string): Profile | null;
  skills(): Skill[];
  skillSource(id: string): object | null;
  composeVizij(gltf_json: string, options_json?: string): object;
  channelKeys(gltf_json: string, channels: string[]): string[];
  behaviorValue(graph_json: string): object;
  outputKeys(graph_json: string): string[];
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

/**
 * What wasm-bindgen's init is handed for `moduleOrPath`, with its download
 * reported: a URL or a `Request` is fetched here and its `Response` counted
 * as it streams into the compile; bytes or a compiled `WebAssembly.Module`
 * need no download and go straight to the compile. `counted` resolves once
 * `compile` is reported.
 */
async function withDownloadProgress(
  moduleOrPath: unknown,
): Promise<{ source: unknown; counted: Promise<void> }> {
  const source = await moduleOrPath;
  const fetchable =
    typeof source === "string" ||
    (typeof URL === "function" && source instanceof URL) ||
    (typeof Request === "function" && source instanceof Request);
  if (fetchable) {
    const { response, counted } = countDownload(await fetch(source as RequestInfo | URL));
    return { source: response, counted };
  }
  if (typeof Response === "function" && source instanceof Response) {
    const { response, counted } = countDownload(source);
    return { source: response, counted };
  }
  reportProgress({ stage: "compile" });
  return { source, counted: Promise.resolve() };
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
        const { module_or_path } = toWasmBindgenInitOptions(initArg);
        const { source, counted } = await withDownloadProgress(module_or_path);
        await (module as WasmBindings).default({ module_or_path: source });
        // Each stage gets a painted frame before the next replaces it.
        await counted;
        await nextPaint();
        reportProgress({ stage: "start" });
        await nextPaint();
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
 * during startup before calling `startRuntime`. {@link onInitProgress}
 * follows it: the download, the compile, the module's start, and `ready`
 * once it resolves.
 */
export function init(input?: InitInput): Promise<void> {
  if (_initPromise) return _initPromise;
  _initPromise = (async () => {
    await loadBindings(input);
    reportProgress({ stage: "ready" });
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
 * await quori.invoke("reset_keys", { keys: { strs: await outputKeys(graph) } });
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

  /** The instance the Vizij was loaded into; empty for a device with no
   * Vizij. */
  get instance(): string {
    return this.inner.instance;
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
   * timestamps). A step of 0 advances no time: it carries changes through
   * the Vizij's graphs and runs, and moves nothing that time drives —
   * clocks, oscillators, noise, springs, dampers, slews and animations
   * hold — so a paused Vizij shows each edit the face computes without
   * time; one that reaches a smoother is its new target, shown once time
   * passes. A `say` run follows its audio, which plays in real time.
   * Unavailable while `run()` owns the device.
   *
   * Until its Vizij is first drawn (its `ready` event), every step is a step of
   * 0 whatever `dtMs` says, and the first step after advances the time
   * since that frame, at most `dtMs`: the device's time starts where the
   * page first shows the face, not where the page created it.
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
   *
   * Called before its Vizij is first drawn (its `ready` event), the loop starts
   * at that frame, its first step advancing the time since. Until then the
   * device is the page's, as before a `run()`, its operations settling at a
   * zero dt; `stop()` meanwhile gives the run up and the promise resolves.
   */
  run(periodMs?: number): Promise<void> {
    return this.inner.run(periodMs);
  }

  /** Reclaim the device from `run()`: the loop ends at its next step
   * boundary and `step()` works again. A `run()` still waiting for its
   * Vizij to be drawn is given up. */
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
   * of this surface that changes the device is queued before it returns, and
   * its promise resolves once a step has applied it. When no `run()` is
   * going, a step with no time in it (`step(0)`) applies it in the next
   * microtask. The operations and store writes of one turn share that step,
   * applied in the order issued, unless the page calls {@link step} in the
   * same turn. Under `run()`, the run's next step applies it.
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
   * them to rest is the client's step: {@link outputKeys} of its graph, then
   * `invoke("reset_keys", { keys: { strs: keys } })`. */
  halt(run: string): Promise<void> {
    return this.inner.halt(run);
  }

  /** The keys the device holds or describes, under `prefix` when given,
   * each with what its store says of it — Arora's ListKeys. A run's keys
   * are listed under {@link BEHAVIOR_RUNS} and its kin. Answered at once,
   * with no step. */
  listKeys(prefix?: string): Promise<KeyInfo[]> {
    return this.inner.listKeys(prefix);
  }

  /** The methods the device describes, under `prefix` when given, with
   * their parameters and return type — Arora's DescribeMethods: what
   * {@link invoke} calls by name. Answered at once, with no step. */
  describeMethods(prefix?: string): Promise<MethodDescription[]> {
    return this.inner.describeMethods(prefix);
  }

  /**
   * Replace the device's running graph **in place**: the spec reaches the
   * interpreter as the interpreter module's LOAD, so the store, the modules
   * and the device survive the swap, and the runs keep running. The promise
   * resolves once the step that applies it, as {@link call}'s, has run the
   * new graph. A graph that would fail every evaluation is refused and the
   * promise rejects naming the node at fault: a node leaving a required
   * input (`required` in `@vizij/node-graph`'s registry) with neither an edge
   * nor an inline default, an edge to a node the graph lacks, two edges into
   * one input, two nodes sharing an id, or a cycle. The running graph and its
   * runs then go on as they were.
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
   * behavior. The promise resolves once the step that applies it, as
   * {@link call}'s, has run the edited graph. An edit leaving a graph
   * {@link loadGraph} would refuse is refused whole, and the running graph
   * and its runs go on as they were.
   */
  applyGraphEdits(edits: GraphEditsInput): Promise<void> {
    const json = typeof edits === "string" ? edits : JSON.stringify(edits);
    return this.inner.applyGraphEdits(json).then(() => undefined);
  }

  /**
   * Write one key, in any `ValueInput` form. The store holds it at once; the
   * step that carries it to the face is {@link call}'s.
   */
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

  /** The keys that changed since the previous drain, each at its latest
   * value (`null` = cleared); the first drain returns the whole current
   * state. What has not been drained is held as one value per changed key,
   * however many steps go by without a drain. */
  drainChanges(): Record<string, ValueJSON | null> {
    return this.inner.drainChanges();
  }

  /**
   * `animation` — one of {@link describe}'s `animations`, or a page's own in
   * that shape — as the animation module's `load_clip` takes it: its
   * id, name and duration (seconds), and `moduleAnimation`, the module's
   * `Clip` (format 2.0) with each track keyed by the store key its channel
   * names through this Vizij's rig, as the Vizij's own animations are
   * loaded ({@link channelKeys} names the same keys from the Vizij's glTF
   * document); on a device from {@link startRuntime}, which has no Vizij,
   * a channel is its own key. Pass it on: `invoke("load_clip", { clip:
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
   * Holds are kept per instance across its reloads, and dropped by
   * {@link VizijHandle.unload}.
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

  /**
   * Hold store keys at the values clients write: from the next step until
   * {@link releaseKeys}, the Vizij's own behavior — its graphs, its program
   * and skill runs, its animations — does not write them, while a client's
   * write ({@link setValue}, {@link writeValues}, the rest module's
   * `reset`) still does. What the graphs compute from a held key follows
   * the client's value: a held pose weight drives the pose it weighs. A
   * held key no client has written keeps the value it had, and holding a
   * key the behavior does not write changes nothing. Keys are full store
   * paths (`rt.path("poses/<id>.weight")`). The holds are the device's, one
   * set whoever holds: a reloaded Vizij starts with none. Where {@link hold} keeps what the view
   * shows of an output, this keeps a key's value in the store, for every
   * reader of the key.
   */
  holdKeys(keys: string[]): void {
    this.inner.holdKeys(keys);
  }

  /** Return held keys to the Vizij's behavior; omitted, every held key,
   * whoever held it. A key the behavior writes every step — a graph's
   * output, a run's, an animation's — takes its writer's value at the next
   * step, which the release asks for as {@link call} does: on a device
   * nothing steps, a `step(0)` in the next microtask, shared with the
   * turn's operations and writes; under {@link run}, the run's next step. */
  releaseKeys(keys?: string[]): void {
    this.inner.releaseKeys(keys);
  }

  /** Release the wasm-side device. This object is unusable afterwards; a
   * Vizij's device is disposed once its face is unloaded
   * ({@link VizijHandle.unload}) or replaced by a later load. */
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
 * A value for one feature of an element: one number (an opacity, a morph
 * weight, one component of a triple, an even scale) or three (a
 * translation, an euler rotation in radians, a scale, a linear RGB colour).
 */
export type FeatureValue = number | [number, number, number];

/**
 * The page's App, mounted on its canvas. **Every face in it is an instance
 * the page names**: its own string, never rewritten, which every call about
 * that face and every event from it carries.
 */
export interface VizijHandle {
  /** The canvas the App draws into. */
  readonly canvas: HTMLCanvasElement;
  /**
   * Loads the Vizij in `glb` into the instance `instance` and starts its
   * device. Under a new id it adds a face; under one that shows a face it
   * replaces it — how an authoring page reloads a structurally edited Vizij
   * from its exported bytes: the face showing keeps drawing until the new
   * one is ready, the instance's placement, view, selection and view holds
   * ({@link Runtime.hold}) carry over, and the new device holds no keys
   * ({@link Runtime.holdKeys}).
   *
   * Resolves to the face's device, its {@link Runtime}, once the face has
   * loaded whole (its `loaded` event): step it, or `run` it — its time
   * starts when the face is first drawn, its `ready` event. The page
   * disposes a Runtime it was given once its face is unloaded or replaced.
   * Rejects, keeping what the instance showed, if `glb` is not a Vizij the
   * module can read or build, its scene fails to load, or a later load into
   * the same instance, or its unload, comes first; the device of a load that
   * rejects is disposed here.
   *
   * With `options.key`, a load of the key the instance shows, with none
   * loading, resolves to the device showing it, and a load of the key it is
   * loading joins that load; neither starts anything.
   */
  load(instance: string, glb: Uint8Array | ArrayBuffer, options?: LoadOptions): Promise<Runtime>;
  /**
   * Takes the face in `instance` down: its scene, its camera, its GLB, its
   * selection and view holds, and anything still loading into it, whose
   * promise rejects. The device the page holds keeps stepping, its key
   * holds with it, until the page disposes it.
   */
  unload(instance: string): void;
  /**
   * Where the face in `instance` draws on the canvas: `rect`, in CSS pixels
   * from the canvas's top-left corner — how several faces share one canvas
   * — or `"fill"`, the whole canvas, where a face never placed draws. Any
   * rectangle: the face is framed on all of it and the part on the canvas
   * draws, so a slot scrolled past the canvas's edge shows its face cut off
   * there, at the position and scale the whole slot gives it; an empty
   * rectangle (a width or height of 0 or less) or one wholly off the canvas
   * draws nothing. Kept across the face's reloads, and may be set before it
   * loads.
   */
  place(instance: string, rect: Rect | "fill"): void;
  /** Places the face in `instance` over `element`, as it lies over the
   * canvas on the page — call it again when the layout changes or the page
   * scrolls. */
  placeIn(instance: string, element: Element): void;
  /**
   * Frames and tone-maps the face in `instance`: `view`'s framing fields
   * override the page's {@link MountOptions} for this face alone, its
   * `bounds` the GLB's authored `rootBounds`. Each call replaces the face's
   * previous view — a field left out takes the page's (tone mapping:
   * `none`), and `{}` hands the face back to the page. Kept across the
   * face's reloads; may be set before it loads. Shows from the next frame.
   */
  setView(instance: string, view: VizijView): void;
  /**
   * Outlines `elements` of the face in `instance`, by the ids its RobotData
   * declares (as its `loaded` event and {@link describe} list them), in
   * place of whatever it outlined; none clears it. One line in the
   * selection red runs around the whole of what is selected, over whatever
   * stands in front of it; a selected group outlines every element mesh
   * under it. Kept across the face's reloads.
   */
  setSelection(instance: string, elements: string[]): void;
  /**
   * Writes a value to `feature` of the element `target` of the face in
   * `instance`, in place — no export, no reload — as an authoring page does
   * while it scrubs a static feature, which no device output drives.
   * `feature` is its RobotData name: `translation`, `rotation` (an euler,
   * radians), `scale` (three, or one scaling evenly), `color`, `emissive`
   * (linear RGB), `opacity`, `metalness`, `emissiveIntensity` (one), one
   * component of a triple by Semio Studio's per-axis name (`translation.x`,
   * `rotation.r`, `color.g`: one), or one of the element's morph target
   * names (a weight). Drawn once the face is indexed; a reload shows what
   * its GLB carries, and a feature a device output drives is the device's
   * again at its next write. A face's animatables are its device's outputs:
   * write the device's keys through its {@link Runtime}.
   */
  setValue(instance: string, target: string, value: FeatureValue, feature: string): void;
  /**
   * Adds, removes, shows, moves or changes a control, by the driver's key
   * for it, in the shape `@semio/scene` takes for robots. A control on a
   * face names it by its instance, and one on a face still loading waits
   * for it; a goal names the face it is used through as its `view`. A drag
   * moves nothing: it is heard as a `control` event, and the driver sends
   * `setPose` if it wants the change.
   */
  control(command: ControlCommand): void;
  /**
   * The faces showing, by instance, as their `loaded` events described
   * them. An event can come before a page has set its handler, so read
   * this rather than rely on hearing it.
   */
  readonly faces: ReadonlyMap<string, ShownFace>;
  /** The instances with a load under way, showing a face yet or not. */
  readonly loading: ReadonlySet<string>;
  /** Where each face's safe area lies on the canvas, by instance, as its
   * last `safeArea` event said: what a DOM overlay outlining it is
   * positioned by. A zoom that crops the bounds reaches past the face's
   * rectangle, and a rectangle past the canvas's edge takes its safe area
   * with it. */
  readonly safeAreas: ReadonlyMap<string, Rect>;
  /** Sets where the App's events go, replacing any earlier handler. */
  onEvent(handler: (event: VizijEvent) => void): void;
}

/** How a Vizij is loaded into an instance: the composition and its device
 * ({@link VizijOptions}), and its key. */
export interface LoadOptions extends VizijOptions {
  /**
   * Names the content: Studio's asset id, a content hash, any string the
   * page keeps stable for the same Vizij. Loading a key an instance already
   * shows, with none loading, or the key it is loading, starts nothing.
   */
  key?: string;
}

/**
 * Create the page's one App over `canvas` — a CSS selector, or the canvas
 * element itself (given an id if it has none) — and resolve to its handle.
 * The canvas fits its parent and stays transparent wherever no face draws.
 * Calls {@link init} if it has not run yet. A page mounts once.
 */
export async function mount(
  canvas: string | HTMLCanvasElement,
  options?: MountOptions,
  input?: InitInput,
): Promise<VizijHandle> {
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
  const wasm = bindings();
  wasm.mount(selector, options ? JSON.stringify(options) : undefined);
  const element =
    typeof canvas === "string" ? (document.querySelector(selector) as HTMLCanvasElement) : canvas;
  const tracked = trackFaces<WasmVizijRuntime>({
    stop: (instance) => wasm.unload(instance),
    dispose: (device) => device.free(),
  });
  wasm.setEventHandler(tracked.dispatch());
  const place = (instance: string, rect: Rect | "fill") =>
    wasm.place(
      instance,
      Float64Array.from(rect === "fill" ? [] : [rect.x, rect.y, rect.width, rect.height]),
    );
  return {
    canvas: element,
    faces: tracked.faces,
    safeAreas: tracked.safeAreas,
    get loading() {
      return tracked.loading;
    },
    async load(instance, glb, options) {
      const bytes = glb instanceof Uint8Array ? glb : new Uint8Array(glb);
      const { audio, modules, ...rest } = options ?? {};
      const play = audio === false ? undefined : (audio ?? defaultPlay);
      const device = await tracked.load(instance, rest.key, () =>
        wasm.load(instance, bytes, JSON.stringify(rest), play, modules),
      );
      return runtimeOf(device);
    },
    unload(instance) {
      tracked.unload(instance);
    },
    place,
    placeIn(instance, target) {
      const c = element.getBoundingClientRect();
      const e = target.getBoundingClientRect();
      place(instance, { x: e.x - c.x, y: e.y - c.y, width: e.width, height: e.height });
    },
    setView(instance, view) {
      wasm.setView(instance, JSON.stringify(view));
    },
    setSelection(instance, elements) {
      wasm.setSelection(instance, elements);
    },
    setValue(instance, target, value, feature) {
      const values = typeof value === "number" ? [value] : value;
      wasm.setValue(instance, target, feature, Float32Array.from(values));
    },
    control(command) {
      wasm.control(JSON.stringify(command));
    },
    onEvent(handler) {
      wasm.setEventHandler(tracked.dispatch(handler));
    },
  };
}

/** One {@link Runtime} per device, so a key that joins a load, or finds its
 * face showing, resolves to the same object the load did. */
const runtimes = new WeakMap<WasmVizijRuntime, Runtime>();

function runtimeOf(device: WasmVizijRuntime): Runtime {
  let runtime = runtimes.get(device);
  if (!runtime) {
    runtime = new Runtime(device);
    runtimes.set(device, runtime);
  }
  return runtime;
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
 * `"rig/quori_latest/"`) is prepended to the face-standard paths the mapping
 * reads and writes (`standard/vizij/*`); omit it for the unprefixed graph.
 * `null` for an unknown id (see {@link mappings}).
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

/** Options for {@link composeVizij}. A face speaks the vizij standard
 * (`standard/vizij/*`) by default; ROS4HRI is the profile a ROS robot asks
 * for with `ros4hri`. */
export interface ComposeVizijOptions {
  /** Base bundle graph kinds to compose. Default: `rig`, `pose-driver`,
   * `pose`, `standard-adaptation`. */
  graphs?: string[];
  /** `"auto"` (the bundle's active program — default), `"none"`, or a
   * program id. */
  program?: string;
  /** Compose the built-in ROS4HRI mapping, so the `standard/ros4hri/*`
   * keys drive the face too. Default `false`; the native `vizij` app
   * composes it unless `--no-ros4hri`. A face that embeds its own copy runs
   * that copy either way, and the built-in never composes over it. */
  ros4hri?: boolean;
  /** Compose the animation source. Default `false`; every device this
   * package builds links the animation module, so the source dispatches. */
  animations?: boolean;
}

/**
 * The composed behavior graph of a face bundle — the composition the native
 * `vizij` app deploys: the bundle's base graphs, its embedded standard
 * mappings (each suppressing the built-in of the same id — an embedded copy
 * is the author's pinned override), the built-in ROS4HRI mapping when
 * `ros4hri` asks for it, then the selected program. `gltf` is the GLB's glTF
 * JSON document. The returned spec feeds {@link startRuntime} or
 * {@link Runtime.loadGraph}, so an exported GLB can be deployed and verified
 * without the native app.
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
 * The store key each of `channels` — animation track channels
 * ({@link AnimationTrack.channel}) — drives on the Vizij whose glTF JSON
 * document is `gltf` (its `VIZIJ_bundle` read as {@link composeVizij} reads
 * it), without loading it: one key per channel, in order, the key
 * {@link Runtime.moduleAnimation} gives the track once the Vizij is loaded.
 * A channel resolves through the bundle's rig: to itself when it is a rig
 * input's path; to the rig input at `rig/<faceId>/<channel>`; to the input
 * the rig names it by (its input node's id without `input_`, or that id's
 * `direct_` or `pose_control_` alias); else to `rig/<faceId>/<channel>`
 * regardless (the bare channel when the bundle names no `faceId`). A
 * channel under `rig/` is never prefixed again. What a page drives on a
 * device from {@link startRuntime} over {@link composeVizij}'s graph, where
 * the Vizij is not loaded, to move it as its animations do. Rejects a
 * document that carries no `VIZIJ_bundle`. Calls {@link init} if it has not
 * run yet.
 */
export async function channelKeys(
  gltf: object,
  channels: string[],
  input?: InitInput,
): Promise<string[]> {
  await init(input);
  return bindings().channelKeys(JSON.stringify(gltf), channels);
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
 * The store keys a run of `graph` (a graph spec, in any form the spec
 * normalizer accepts) writes and leaves holding when it is halted: its output
 * nodes' paths, sorted — without the outputs on `task/…` paths, which land on
 * the run's own keys. What returns a halted program's outputs to rest:
 * `runtime.invoke("reset_keys", { keys: { strs: await outputKeys(graph) } })`.
 * Calls {@link init} if it has not run yet.
 */
export async function outputKeys(graph: GraphSpecInput, input?: InitInput): Promise<string[]> {
  await init(input);
  return bindings().outputKeys(typeof graph === "string" ? graph : JSON.stringify(graph));
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
