/**
 * What {@link Runtime}'s animation methods speak to the animation module: the
 * module's declared function, parameter and field ids (its Rust declaration,
 * `animation` in `vizij-animation-module`; `tests/animations.mjs` checks them
 * against the header `@vizij/animation-module` ships), and the decoding of
 * the `[PlayerState]` the composed animation source writes each step.
 *
 * @module
 */

/** The declared ids the runtime calls, each function with its parameters. */
export const ANIMATION_IDS = {
  load_animation: {
    function: "76697a69-6a00-0000-0f00-000000000001",
    clip: "76697a69-6a00-0000-0f01-000000000001",
  },
  create_player: {
    function: "76697a69-6a00-0000-0f00-000000000002",
    name: "76697a69-6a00-0000-0f02-000000000001",
  },
  add_instance_with_weight: {
    function: "76697a69-6a00-0000-0f00-000000000012",
    player: "76697a69-6a00-0000-0f12-000000000001",
    anim: "76697a69-6a00-0000-0f12-000000000002",
    weight: "76697a69-6a00-0000-0f12-000000000003",
  },
  play: {
    function: "76697a69-6a00-0000-0f00-000000000005",
    player: "76697a69-6a00-0000-0f05-000000000001",
  },
  pause: {
    function: "76697a69-6a00-0000-0f00-000000000006",
    player: "76697a69-6a00-0000-0f06-000000000001",
  },
  stop: {
    function: "76697a69-6a00-0000-0f00-000000000007",
    player: "76697a69-6a00-0000-0f07-000000000001",
  },
  seek: {
    function: "76697a69-6a00-0000-0f00-000000000008",
    player: "76697a69-6a00-0000-0f08-000000000001",
    time_ns: "76697a69-6a00-0000-0f08-000000000002",
  },
  set_speed: {
    function: "76697a69-6a00-0000-0f00-000000000009",
    player: "76697a69-6a00-0000-0f09-000000000001",
    speed: "76697a69-6a00-0000-0f09-000000000002",
  },
  set_loop: {
    function: "76697a69-6a00-0000-0f00-00000000000a",
    player: "76697a69-6a00-0000-0f0a-000000000001",
    mode: "76697a69-6a00-0000-0f0a-000000000002",
  },
  set_weight: {
    function: "76697a69-6a00-0000-0f00-00000000000b",
    player: "76697a69-6a00-0000-0f0b-000000000001",
    instance: "76697a69-6a00-0000-0f0b-000000000002",
    weight: "76697a69-6a00-0000-0f0b-000000000003",
  },
  unload_animation: {
    function: "76697a69-6a00-0000-0f00-000000000010",
    anim: "76697a69-6a00-0000-0f10-000000000001",
  },
  remove_player: {
    function: "76697a69-6a00-0000-0f00-000000000011",
    player: "76697a69-6a00-0000-0f11-000000000001",
  },
} as const;

/** The store key the composed animation source writes `[PlayerState]` to,
 * each step. */
export const ANIMATION_PLAYERS_PATH = "vizij/animations/players";

/** `PlayerState`'s field ids (record 1.1.0). */
export const PLAYER_STATE_FIELDS = {
  player: "76697a69-6a00-0000-0111-000000000001",
  state: "76697a69-6a00-0000-0111-000000000002",
  time_ns: "76697a69-6a00-0000-0111-000000000003",
  duration_ns: "76697a69-6a00-0000-0111-000000000004",
  speed: "76697a69-6a00-0000-0111-000000000005",
  name: "76697a69-6a00-0000-0111-000000000006",
  instances: "76697a69-6a00-0000-0111-000000000007",
  loop_mode: "76697a69-6a00-0000-0111-000000000008",
} as const;

/** `InstanceState`'s field ids. */
export const INSTANCE_STATE_FIELDS = {
  instance: "76697a69-6a00-0000-0112-000000000001",
  anim: "76697a69-6a00-0000-0112-000000000002",
  weight: "76697a69-6a00-0000-0112-000000000003",
} as const;

/** One instance on a player, as the animation module reports it. */
export interface InstanceState {
  instance: number;
  /** The animation it plays: what `unload_animation` takes. */
  anim: number;
  weight: number;
}

/** One player's state, as the animation module reports it. */
export interface PlayerState {
  player: number;
  /** The name `create_player` gave it — an animation's player is named after
   * the animation's id — or empty. */
  name: string;
  /** `playing`, `paused` or `stopped`. */
  state: string;
  /** Seconds: the playhead, after the loop mode maps it. */
  time: number;
  /** Seconds: the player's length. */
  duration: number;
  /** Zero while paused or stopped. */
  speed: number;
  /** `once`, `loop` or `ping_pong`, as `set_loop` takes it. */
  loopMode: string;
  instances: InstanceState[];
}

type Fields = Map<string, Record<string, unknown>>;

/** The elements of an array-of-structures value (`{ structs: { elements } }`),
 * each as its fields by id; empty for anything else. */
function structures(value: unknown): Fields[] {
  const elements = (value as { structs?: { elements?: unknown } } | null)?.structs?.elements;
  if (!Array.isArray(elements)) {
    return [];
  }
  type Element = { fields?: { id: string; value: Record<string, unknown> }[] };
  return elements.map(
    (element) =>
      new Map(((element as Element).fields ?? []).map((field) => [field.id, field.value])),
  );
}

/** A scalar field's value (`{ u32: 3 }` → 3). */
function scalar(fields: Fields, id: string): unknown {
  const value = fields.get(id);
  return value ? Object.values(value)[0] : undefined;
}

/** The players of a `[PlayerState]` store value; empty for anything else. */
export function decodePlayerStates(value: unknown): PlayerState[] {
  return structures(value).map((fields) => ({
    player: Number(scalar(fields, PLAYER_STATE_FIELDS.player)),
    name: String(scalar(fields, PLAYER_STATE_FIELDS.name) ?? ""),
    state: String(scalar(fields, PLAYER_STATE_FIELDS.state) ?? ""),
    time: Number(scalar(fields, PLAYER_STATE_FIELDS.time_ns) ?? 0) / 1e9,
    duration: Number(scalar(fields, PLAYER_STATE_FIELDS.duration_ns) ?? 0) / 1e9,
    speed: Number(scalar(fields, PLAYER_STATE_FIELDS.speed) ?? 0),
    loopMode: String(scalar(fields, PLAYER_STATE_FIELDS.loop_mode) ?? "loop"),
    instances: structures(fields.get(PLAYER_STATE_FIELDS.instances)).map((instance) => ({
      instance: Number(scalar(instance, INSTANCE_STATE_FIELDS.instance)),
      anim: Number(scalar(instance, INSTANCE_STATE_FIELDS.anim)),
      weight: Number(scalar(instance, INSTANCE_STATE_FIELDS.weight) ?? 0),
    })),
  }));
}
