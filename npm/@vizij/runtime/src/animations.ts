/**
 * The animation module's state as a client reads it: the \`[PlayerState]\`
 * the composed animation source writes to {@link ANIMATION_PLAYERS_PATH}
 * each step — one per player, by the name it was created under — decoded
 * from the Arora records the store holds (keyed by field id) into plain
 * objects. A client drives the module through its declared functions by
 * name (\`runtime.invoke("play", { player })\`); this is how it learns which
 * player plays what, and where each stands.
 *
 * @module
 */

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
  /** The speed multiplier, as `set_speed` set it: kept while paused or
   * stopped. */
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
