/**
 * What the animation transport of {@link Runtime} speaks to the animation
 * module: the module's declared function, parameter and field ids (its Rust
 * declaration, `animation` in `vizij-animation-module`;
 * `tests/animations.mjs` checks them against the header
 * `@vizij/animation-module` ships), and the decoding of the `[PlayerState]`
 * the composed animation source writes each step.
 *
 * @module
 */

/** The declared ids the transport calls, each function with its parameters. */
export const ANIMATION_IDS = {
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
} as const;

/** The store key the composed animation source writes `[PlayerState]` to,
 * each step. */
export const ANIMATION_PLAYERS_PATH = "vizij/animations/players";

/** `PlayerState`'s field ids. */
export const PLAYER_STATE_FIELDS = {
  player: "76697a69-6a00-0000-0111-000000000001",
  state: "76697a69-6a00-0000-0111-000000000002",
  time_ns: "76697a69-6a00-0000-0111-000000000003",
  duration_ns: "76697a69-6a00-0000-0111-000000000004",
  speed: "76697a69-6a00-0000-0111-000000000005",
} as const;

/** One player's state, as the animation module reports it. */
export interface PlayerState {
  player: number;
  /** `playing`, `paused` or `stopped`. */
  state: string;
  /** Seconds: the playhead, after the loop mode maps it. */
  time: number;
  /** Seconds: the player's length. */
  duration: number;
  /** Zero while paused or stopped. */
  speed: number;
}

/** The players of a `[PlayerState]` store value (`{ structs: { elements } }`);
 * empty for anything else. */
export function decodePlayerStates(value: unknown): PlayerState[] {
  const elements = (value as { structs?: { elements?: unknown } } | null)?.structs?.elements;
  if (!Array.isArray(elements)) {
    return [];
  }
  return elements.map((element) => {
    const fields = new Map<string, Record<string, unknown>>(
      ((element as { fields?: { id: string; value: Record<string, unknown> }[] }).fields ?? []).map(
        (field) => [field.id, field.value],
      ),
    );
    const scalar = (id: string): unknown => {
      const value = fields.get(id);
      return value ? Object.values(value)[0] : undefined;
    };
    return {
      player: Number(scalar(PLAYER_STATE_FIELDS.player)),
      state: String(scalar(PLAYER_STATE_FIELDS.state) ?? ""),
      time: Number(scalar(PLAYER_STATE_FIELDS.time_ns) ?? 0) / 1e9,
      duration: Number(scalar(PLAYER_STATE_FIELDS.duration_ns) ?? 0) / 1e9,
      speed: Number(scalar(PLAYER_STATE_FIELDS.speed) ?? 0),
    };
  });
}
