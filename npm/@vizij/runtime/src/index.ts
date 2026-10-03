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
 */
import { toValueJSON, type ValueJSON, type ValueInput } from "@vizij/value-json";
import {
  loadBindings as loadWasmBindings,
  type InitInput as LoaderInitInput,
} from "@vizij/wasm-loader";
import { loadBindings as loadWasmBindingsBrowser } from "@vizij/wasm-loader/browser";
import { play as defaultPlay, type Play } from "./audio.js";
import {
  ANIMATION_IDS,
  ANIMATION_PLAYERS_PATH,
  decodePlayerStates,
  type PlayerState,
} from "./animations.js";

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
 * An Arora `Call`, as an object (or already-serialized JSON): the function
 * `id`, optionally the `module_id` it lives in (inferred from the loaded
 * modules when omitted), and `args` as `{ id, value }` pairs in the Arora
 * `Value` vocabulary.
 */
export type RuntimeCall = object | string;

/** What a runtime call resolves to: the returned Arora `Value`. */
export interface RuntimeCallResult {
  ret: unknown;
  mutated?: unknown[];
}

/**
 * A task run's lifecycle contract, as {@link Runtime.spawn} resolves it: the
 * `status` key to watch (the run's `Status` value, terminal once it ends),
 * the `feedback` keys it updates while it runs, the `result` keys it writes
 * when it ends, the `update` keys a live goal update is written to, and the
 * `stop` call {@link Runtime.halt} issues. Keys are store paths.
 */
export interface TaskHandle {
  id: string;
  stop: object;
  status: string;
  feedback: string[];
  result?: string[];
  update?: string[];
}

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

/** A pointer press on a Vizij: the slot it is shown under and the element
 * id its GLB's RobotData declares. */
export interface Pick {
  vizijId: string;
  elementId: string;
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
 * whether the bundle's neutral pose is staged at load (default `true`;
 * {@link Runtime.reset} returns to it at any time), the speech —
 * `audio` is the playback hook the `say` provider hands its audio to: the
 * page's Web Audio player by default, `false` for a device that plays no
 * speech — and `speechApiUrl` the TTS deployment, and the wasm modules to
 * load into the device as guests. */
export interface VizijOptions extends Omit<ComposeVizijOptions, "animations"> {
  /** Load the face's own animations (its bundle's `animations`) into its
   * device, silent until played. Default `true`; `false` leaves the device's
   * animation module empty for {@link Runtime.loadAnimation}. Unlike
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
  rootBounds: { center: { x: number; y: number }; size: { x: number; y: number } } | null;
  elements: {
    id: string;
    name: string;
    kind: string;
    material: string | null;
    morphTargets: string[];
  }[];
  /** animatable UUID → the node it drives and the feature (`translation`,
   * `rotation`, `scale`, `color`, `opacity`, or a morph target's name). */
  animatables: Record<string, { node: string; feature: string }>;
  graphs: { kind: string }[];
  /** The ids of the motion-graph programs the Vizij can play. */
  programs: string[];
  /** Program id → its label, for the programs that carry one. */
  programLabels: Record<string, string>;
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

/** An animation a Vizij's device holds, as {@link Runtime.animations} lists
 * it: the id its player is named after, and the player's length. */
export interface LoadedAnimation {
  id: string;
  /** Seconds. */
  duration: number;
}

/** What the wasm makes of an animation for the module's `load_animation`. */
interface WasmModuleAnimation {
  id: string;
  name: string | null;
  duration: number;
  /** The module's `AnimationClip`, as an Arora `Value`. */
  moduleAnimation: object;
}

/** An animation's playback, as {@link Runtime.animationState} reads it. */
export interface AnimationState {
  /** Seconds: the playhead. */
  time: number;
  /** Seconds. */
  duration: number;
  /** Advancing: played, neither paused, stopped nor completed. */
  playing: boolean;
  /** Wraps at its end (the default) rather than stopping there. */
  loop: boolean;
  /** The speed multiplier it plays at — kept while paused. */
  speed: number;
  /** Played to its end without looping: it holds its last pose there. */
  completed: boolean;
}

/** Options for {@link Runtime.playAnimation}. */
export interface PlayAnimationOptions {
  /** Start from the beginning rather than from the playhead. Default `false`. */
  reset?: boolean;
  /** The speed multiplier to play at, kept from then on (as
   * {@link Runtime.setAnimationSpeed}). Default: the animation's current one,
   * 1 at load. */
  speed?: number;
}

/** Options for {@link Runtime.stopAnimation}. */
export interface StopAnimationOptions {
  /** Return the animation's keys to its first frame before it falls silent.
   * `false` leaves them where the animation last wrote them. Default
   * `true`. */
  clearOutputs?: boolean;
}

/** The module's transport functions that take a player. */
type TransportFunction = "play" | "pause" | "stop" | "seek" | "set_speed" | "set_loop";

/** A transport method's body: the animation's player, what the runtime
 * remembers of it, and whether its earlier transport calls are in flight. */
type TransportCommand = (
  player: PlayerState,
  transport: AnimationTransport,
  settling: boolean,
) => Promise<void>;

/** What a player's state says a runtime last asked of it. */
function askedOf(player: PlayerState): AnimationTransport["asked"] {
  return player.state === "playing" || player.state === "paused" ? player.state : "stopped";
}

/** What a {@link Runtime} remembers of an animation it drives, for the player
 * it drives: the speed a pause or stop zeroes (the module reports 0 then),
 * and what it last asked of the animation. */
interface AnimationTransport {
  player: number;
  speed: number;
  asked: "playing" | "paused" | "stopped";
  /** Bumped by every play, pause and stop, so a stop's deferred silence
   * yields to a later command. */
  epoch: number;
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
  spawn(call_json: string): Promise<string>;
  spawnSkill(name: string, args_json: string): Promise<string>;
  halt(handle_json: string): Promise<string>;
  reset(): Promise<string>;
  loadGraph(graph_json: string): Promise<string>;
  applyGraphEdits(edits_json: string): Promise<string>;
  setValue(path: string, value_json: string): void;
  writeValues(values_json: string): void;
  readValues(paths: string[]): Record<string, ValueJSON | null>;
  snapshot(): Record<string, ValueJSON>;
  drainChanges(): Record<string, ValueJSON | null>;
  moduleAnimation(animation_json: string): WasmModuleAnimation;
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
  describe(glb: Uint8Array): VizijDescription;
  memoryBytes(): number;
  mappings(): Mapping[];
  mapping(id: string, rig_prefix: string): object | null;
  profiles(): ProfileSummary[];
  profile(id: string, rig_prefix: string): Profile | null;
  skills(): Skill[];
  skillSource(id: string): object | null;
  composeVizij(gltf_json: string, options_json?: string): object;
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

/** The id a module function returned (`{ u32: 3 }`). */
function returnedId(ret: unknown): number {
  const id = (ret as { u32?: unknown } | null)?.u32;
  if (typeof id !== "number") {
    throw new Error(
      `@vizij/runtime: expected an id from the animation module, got ${JSON.stringify(ret)}`,
    );
  }
  return id;
}

/**
 * A Vizij's device — or, from {@link startRuntime}, a device with no Vizij.
 * All methods talk to the device's own store; the graph it runs reads and
 * writes the same keys. A Vizij's paths live under its {@link rigPrefix};
 * {@link path} builds one.
 */
export class Runtime {
  private inner: WasmVizijRuntime;
  private transports = new Map<string, AnimationTransport>();
  /** Per animation id, the load or unload in flight: the next one, and the
   * transport, wait for it. */
  private loads = new Map<string, Promise<void>>();
  /** Per animation id, the transport calls in flight: a load or unload waits
   * for them, so it reads the playback they leave. */
  private inflight = new Map<string, Set<Promise<void>>>();

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
   * Call a module function through the device (the animation module's need
   * no `module_id`). The call is enqueued before this returns and dispatches
   * inside the device's **next** step, so the returned promise resolves only
   * after that step runs. Under `run()` just `await` it; a direct driver
   * calls `step` after issuing it.
   */
  call(call: RuntimeCall): Promise<RuntimeCallResult> {
    const json = typeof call === "string" ? call : JSON.stringify(call);
    return this.inner.call(json).then((result) => JSON.parse(result) as RuntimeCallResult);
  }

  /**
   * Spawn a described function — `say`, `look_at`, `play_viseme` — as a task
   * run on the device's interpreter, the way a bridge does for a remote.
   * Resolves (after the next step) to the run's {@link TaskHandle}: watch
   * its `status` key, read its `feedback` keys, {@link halt} it with it.
   */
  spawn(call: RuntimeCall): Promise<TaskHandle> {
    const json = typeof call === "string" ? call : JSON.stringify(call);
    return this.inner.spawn(json).then((handle) => JSON.parse(handle) as TaskHandle);
  }

  /**
   * Spawn one of the device's skills by name — `say` (`text`, `voice`),
   * `look_at` (`policy`, `target`, `frame`), `play_viseme` (`shape`,
   * `weight`) — with its arguments by parameter name, in any `ValueInput`
   * shorthand. `say` needs a face loaded with a playback hook. Resolves
   * like {@link spawn}.
   */
  spawnSkill(name: string, args: Record<string, ValueInput>): Promise<TaskHandle> {
    const normalized: Record<string, ValueJSON> = {};
    for (const [parameter, value] of Object.entries(args)) {
      normalized[parameter] = toValueJSON(value);
    }
    return this.inner
      .spawnSkill(name, JSON.stringify(normalized))
      .then((handle) => JSON.parse(handle) as TaskHandle);
  }

  /** Halt a run: resolves once the halt is applied (its status key then
   * reads terminal). */
  halt(handle: TaskHandle): Promise<void> {
    return this.inner.halt(JSON.stringify(handle)).then(() => undefined);
  }

  /**
   * Return the face to rest: every key the device describes goes back to
   * the value it rests at — each free input of its graph to its authored
   * default, each input the bundle's neutral pose names to its neutral — and
   * the graph carries the face there on its next tick. Keys with no
   * declared rest keep their values. The writes go through the device's
   * store, so every bridge sees them. Resolves once applied; on a device
   * not under `run()` a zero-dt step lands it.
   */
  reset(): Promise<void> {
    const done = this.inner.reset();
    if (!this.inner.running) {
      this.inner.step(0);
    }
    return done.then(() => undefined);
  }

  /**
   * Replace the device's running graph **in place**: the spec reaches the
   * interpreter as the engine's LOAD call, so the store, the modules and the
   * device itself all survive the swap. Resolves once the new graph is
   * installed; on a device not under `run()` a zero-dt step is taken so the
   * swap lands without an external driver.
   */
  loadGraph(graph: GraphSpecInput): Promise<void> {
    const json = typeof graph === "string" ? graph : JSON.stringify(graph);
    const loaded = this.inner.loadGraph(json);
    if (!this.inner.running) {
      this.inner.step(0);
    }
    return loaded.then(() => undefined);
  }

  /**
   * Edit the running graph **in place**: `edits` is a spec-level graph diff
   * (`upsert_nodes` / `remove_nodes` / `upsert_edges` / `remove_edges`) that
   * reaches the interpreter as the engine's EDIT call. Unchanged nodes keep
   * their runtime state. Resolves once applied; on a device not under
   * `run()` a zero-dt step lands it.
   */
  applyGraphEdits(edits: GraphEditsInput): Promise<void> {
    const json = typeof edits === "string" ? edits : JSON.stringify(edits);
    const applied = this.inner.applyGraphEdits(json);
    if (!this.inner.running) {
      this.inner.step(0);
    }
    return applied.then(() => undefined);
  }

  /** Write one store key. Accepts any `ValueInput` shorthand. */
  setValue(path: string, value: ValueInput): void {
    this.inner.setValue(path, JSON.stringify(toValueJSON(value)));
  }

  /** Write several store keys as one change. */
  writeValues(values: Record<string, ValueInput>): void {
    const normalized: Record<string, ValueJSON> = {};
    for (const [path, value] of Object.entries(values)) {
      normalized[path] = toValueJSON(value);
    }
    this.inner.writeValues(JSON.stringify(normalized));
  }

  /** Read store keys; absent keys map to `null`. */
  readValues(paths: string[]): Record<string, ValueJSON | null> {
    return this.inner.readValues(paths);
  }

  /** Every key currently in the store. */
  snapshot(): Record<string, ValueJSON> {
    return this.inner.snapshot();
  }

  /**
   * The keys that changed since the last call (`null` = cleared). The first
   * drain returns the store's whole current state: the subscription opens on
   * it, so no separate init snapshot is needed.
   */
  drainChanges(): Record<string, ValueJSON | null> {
    return this.inner.drainChanges();
  }

  /**
   * The animations the device's animation module holds, whoever loaded them
   * — the Vizij's own, loaded with it, those {@link loadAnimation} loaded,
   * and those another client loaded through the module's functions — read
   * from the module: one per named player holding an instance, in the player
   * states the animation source writes to `vizij/animations/players` each
   * step, in load order. An animation writes the keys its tracks' channels
   * name — while it plays, is paused or has completed.
   */
  animations(): LoadedAnimation[] {
    const players = this.players();
    const ids = [...new Set(players.map((player) => player.name))].filter((id) => id !== "");
    return ids.flatMap((id) => {
      const player = this.playerOf(id, players);
      return player ? [{ id, duration: player.duration }] : [];
    });
  }

  /**
   * Load an animation into the device's animation module through the
   * module's declared functions — what any client sends: `load_animation`,
   * `create_player` named after the animation's id,
   * `add_instance_with_weight` at 0 and `stop`. It loads silent, stopped at
   * its start, looping at speed 1. `animation` is in {@link describe}'s
   * `animations` shape (or a bundle `animations` entry), its channels
   * resolved through the Vizij's rig like its own animations'.
   *
   * Loading an id already loaded replaces it — the old player and animation
   * out and the new ones in, in one step — keeping its playback as the
   * module reports it once the transport calls in flight have landed: the
   * playhead, loop mode, speed, weight, and whether it plays, is paused or
   * is stopped. That is the authoring timeline's live edit. Resolves once
   * the device holds it (two steps, after those calls).
   */
  loadAnimation(animation: Animation): Promise<LoadedAnimation & { name: string | null }> {
    const { id, name, duration, moduleAnimation } = this.inner.moduleAnimation(
      JSON.stringify(animation),
    );
    return this.inOrder(id, () => this.load(id, moduleAnimation)).then(() => ({ id, name, duration }));
  }

  /**
   * Unload an animation through the module's declared functions, once the
   * transport calls in flight on it have landed: `remove_player` on its
   * player and `unload_animation` on what it plays. Its keys keep the values
   * it last wrote — a `stopAnimation` issued before returns them to its first
   * frame. Resolves (after the step that applies it) to whether it was
   * loaded.
   */
  unloadAnimation(id: string): Promise<boolean> {
    return this.inOrder(id, async () => {
      const transport = this.transports.get(id);
      if (transport) {
        transport.epoch += 1;
        this.transports.delete(id);
      }
      const player = this.playerOf(id);
      if (!player) {
        return false;
      }
      await Promise.all([
        this.animationCall("remove_player", { player: { u32: player.player } }),
        ...player.instances.map(({ anim }) =>
          this.animationCall("unload_animation", { anim: { u32: anim } }),
        ),
      ]);
      return true;
    });
  }

  /**
   * Play an animation from its playhead, or from its start with `reset`.
   * Applied at the device's next step, which the promise waits for. Its state
   * reports `completed` once a non-looping animation reaches its end; it holds
   * its last pose there until stopped or played again with `reset`.
   */
  playAnimation(id: string, options: PlayAnimationOptions = {}): Promise<void> {
    return this.transport(id, (player, transport) => {
      if (options.speed !== undefined) {
        transport.speed = options.speed;
      }
      transport.asked = "playing";
      transport.epoch += 1;
      const calls = [this.weigh(player, 1)];
      if (options.reset) {
        calls.push(this.playerCall("seek", player, { time_ns: { u64: 0 } }));
      }
      calls.push(this.playerCall("play", player));
      // `play` resumes at speed 1: the animation's own speed follows it.
      calls.push(this.playerCall("set_speed", player, { speed: { f32: transport.speed } }));
      return Promise.all(calls).then(() => undefined);
    });
  }

  /** Hold an animation's playhead, and its pose. Applied at the next step. */
  pauseAnimation(id: string): Promise<void> {
    return this.transport(id, (player, transport) => {
      transport.asked = "paused";
      transport.epoch += 1;
      return this.playerCall("pause", player);
    });
  }

  /**
   * Stop an animation: its playhead back to the start, and the animation
   * silent — it no longer writes its keys. With `clearOutputs` (the default)
   * its keys first return to its first frame, one step before it falls
   * silent; the promise resolves once silent. A play before then wins.
   */
  stopAnimation(id: string, options: StopAnimationOptions = {}): Promise<void> {
    return this.transport(id, (player, transport) => {
      transport.asked = "stopped";
      const epoch = (transport.epoch += 1);
      const stopped = this.playerCall("stop", player);
      if (options.clearOutputs === false) {
        return Promise.all([stopped, this.weigh(player, 0)]).then(() => undefined);
      }
      return stopped.then(() => (transport.epoch === epoch ? this.weigh(player, 0) : undefined));
    });
  }

  /** Move an animation's playhead to `seconds`. Applied at the next step; a
   * silent animation moves without writing. */
  seekAnimation(id: string, seconds: number): Promise<void> {
    return this.transport(id, (player) => {
      const ns = Math.max(0, Math.round(seconds * 1e9));
      return this.playerCall("seek", player, { time_ns: { u64: ns } });
    });
  }

  /** Loop an animation at its end (the default), or stop it there: `false`
   * lets it complete. Applied at the next step. */
  setAnimationLoop(id: string, loop: boolean): Promise<void> {
    return this.transport(id, (player) =>
      this.playerCall("set_loop", player, { mode: { str: loop ? "loop" : "once" } }),
    );
  }

  /** Set an animation's speed multiplier: at once while it plays (at the next
   * step), else from its next play. */
  setAnimationSpeed(id: string, speed: number): Promise<void> {
    return this.transport(id, (player, transport, settling) => {
      transport.speed = speed;
      // While this runtime's own calls are in flight the player states lag
      // them: what it last asked stands for the module's state.
      const playing = settling ? transport.asked === "playing" : player.state === "playing";
      return playing
        ? this.playerCall("set_speed", player, { speed: { f32: speed } })
        : Promise.resolve();
    });
  }

  /**
   * An animation's playback as the animation source last reported it — the
   * player states it writes to `vizij/animations/players` each step, which
   * may lag a transport call by a step — or `null` for an animation the
   * device does not hold. The speed of a paused or stopped animation, which
   * the module reports as 0, is the one this runtime last set (1 when it set
   * none). Cheap enough to poll every frame.
   */
  animationState(id: string): AnimationState | null {
    const player = this.playerOf(id);
    if (!player) {
      return null;
    }
    const loop = player.loopMode !== "once";
    // A non-looping player clamps its playhead at the end, still advancing.
    const completed = !loop && player.duration > 0 && player.time >= player.duration - 1e-3;
    return {
      time: player.time,
      duration: player.duration,
      playing: player.state === "playing" && !completed,
      loop,
      speed: player.speed !== 0 ? player.speed : (this.remembered(id, player)?.speed ?? 1),
      completed,
    };
  }

  /** Load `moduleAnimation` as animation `id`, replacing the one loaded under it: the new
   * player and animation first (one step), then — in one step, so no frame
   * goes without either — the old player and animation out, the new instance
   * in at the old one's weight, and the old playback restored as the module
   * reports it after the first step. */
  private async load(id: string, moduleAnimation: object): Promise<void> {
    const before = this.playerOf(id);
    const [anim, player] = await Promise.all([
      this.animationCall("load_animation", { clip: moduleAnimation }).then(returnedId),
      this.animationCall("create_player", { name: { str: id } }).then(returnedId),
    ]);
    // The old player as the module reports it now: after every transport
    // call issued before this load, and the step that loaded the new one.
    const old = before && (this.players().find((p) => p.player === before.player) ?? before);
    const kept = old ? this.remembered(id, old) : undefined;
    const calls: Promise<unknown>[] = [];
    if (old) {
      calls.push(this.animationCall("remove_player", { player: { u32: old.player } }));
      for (const instance of old.instances) {
        calls.push(this.animationCall("unload_animation", { anim: { u32: instance.anim } }));
      }
    }
    calls.push(
      this.animationCall("add_instance_with_weight", {
        player: { u32: player },
        anim: { u32: anim },
        weight: { f32: old?.instances[0]?.weight ?? 0 },
      }),
    );
    const command = (name: TransportFunction, args = {}) =>
      calls.push(this.animationCall(name, { player: { u32: player }, ...args }));
    if (old && old.loopMode !== "loop") {
      command("set_loop", { mode: { str: old.loopMode } });
    }
    if (old && old.state !== "stopped") {
      command("seek", { time_ns: { u64: Math.round(old.time * 1e9) } });
      if (old.state === "playing") {
        command("play");
        command("set_speed", { speed: { f32: old.speed } });
      } else {
        command("pause");
      }
    } else {
      command("stop");
    }
    await Promise.all(calls);
    const speed = old && old.speed !== 0 ? old.speed : (kept?.speed ?? 1);
    const asked = old ? askedOf(old) : "stopped";
    this.transports.set(id, { player, speed, asked, epoch: kept?.epoch ?? 0 });
  }

  /** Run `task` once the load or unload of `id` in flight, and the transport
   * calls in flight on it, have settled; the next load or unload, and the
   * transport, wait for it in turn. */
  private inOrder<T>(id: string, task: () => Promise<T>): Promise<T> {
    const before = [this.loads.get(id), ...(this.inflight.get(id) ?? [])].map((pending) =>
      pending?.catch(() => undefined),
    );
    const run = Promise.all(before).then(task);
    const settled = run.then(
      () => undefined,
      () => undefined,
    );
    this.loads.set(id, settled);
    void settled.then(() => {
      if (this.loads.get(id) === settled) {
        this.loads.delete(id);
      }
    });
    return run;
  }

  /**
   * Run a transport `command` on animation `id`'s player — now, so its calls
   * enqueue before this returns, or after the load or unload of `id` in
   * flight. It rejects for an animation the device does not hold. `settling`
   * says whether this runtime's earlier transport calls on `id` are still in
   * flight.
   */
  private transport(id: string, command: TransportCommand): Promise<void> {
    const issue = (): Promise<void> => {
      try {
        const player = this.playerOf(id);
        if (!player) {
          throw new Error(`@vizij/runtime: no animation ${JSON.stringify(id)} is loaded`);
        }
        let transport = this.remembered(id, player);
        if (!transport) {
          const speed = player.speed || 1;
          transport = { player: player.player, speed, asked: askedOf(player), epoch: 0 };
          this.transports.set(id, transport);
        }
        return command(player, transport, (this.inflight.get(id)?.size ?? 0) > 0);
      } catch (error) {
        return Promise.reject(error);
      }
    };
    const pending = this.loads.get(id);
    const run = pending ? pending.then(issue) : issue();
    const inflight = this.inflight.get(id) ?? new Set<Promise<void>>();
    this.inflight.set(id, inflight);
    inflight.add(run);
    const done = () => {
      inflight.delete(run);
      if (inflight.size === 0 && this.inflight.get(id) === inflight) {
        this.inflight.delete(id);
      }
    };
    run.then(done, done);
    return run;
  }

  /** The module's players, as the animation source last wrote them. */
  private players(): PlayerState[] {
    return decodePlayerStates(
      this.inner.readValues([ANIMATION_PLAYERS_PATH])[ANIMATION_PLAYERS_PATH],
    );
  }

  /** The player animation `id` plays on, among `players`: the newest named
   * `id` that holds an instance. */
  private playerOf(id: string, players = this.players()): PlayerState | undefined {
    return players.filter((player) => player.name === id && player.instances.length > 0).pop();
  }

  /** What this runtime remembers of animation `id`, when it is about `player`
   * — or while a load of `id` is in flight, which moves it to the new player
   * once done. */
  private remembered(id: string, player: PlayerState): AnimationTransport | undefined {
    const transport = this.transports.get(id);
    return transport && (transport.player === player.player || this.loads.has(id))
      ? transport
      : undefined;
  }

  /** Call one of the animation module's declared functions, with its
   * arguments by parameter name; resolves to what it returns. */
  private animationCall<F extends keyof typeof ANIMATION_IDS>(
    name: F,
    args: Record<string, object> = {},
  ): Promise<unknown> {
    const ids = ANIMATION_IDS[name] as Record<string, string>;
    return this.call({
      id: ids.function,
      args: Object.entries(args).map(([parameter, value]) => ({ id: ids[parameter], value })),
    }).then((result) => result.ret);
  }

  /** Call one of the module's transport functions on `player`. */
  private playerCall(
    name: TransportFunction,
    player: PlayerState,
    args: Record<string, object> = {},
  ): Promise<void> {
    return this.animationCall(name, { player: { u32: player.player }, ...args }).then(
      () => undefined,
    );
  }

  /** Give `player`'s instances `weight`: 0 silences the animation, 1 lets it
   * write. */
  private weigh(player: PlayerState, weight: number): Promise<void> {
    return Promise.all(
      player.instances.map(({ instance }) =>
        this.animationCall("set_weight", {
          player: { u32: player.player },
          instance: { u32: instance },
          weight: { f32: weight },
        }),
      ),
    ).then(() => undefined);
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
 * once it shows. A Vizij already shown under `vizijId` is replaced. Requires
 * {@link mount}.
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

/** Take the Vizij down: its scene, its camera, its GLB. Dispose its
 * {@link Runtime} afterwards. */
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
 * then on its device's pose shows. */
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

/** The pointer presses on Vizijs since the last drain, oldest first. */
export function drainPicks(): Pick[] {
  return bindings().drainPicks();
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
 * inputs resting at their authored defaults ({@link Runtime.reset}), the
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

export { unlockAudio, play as playAudio, IDLE_STOP_MS } from "./audio.js";
export type { Play, SpeechMark } from "./audio.js";
export { toValueJSON } from "@vizij/value-json";
export type { ValueJSON, ValueInput } from "@vizij/value-json";
