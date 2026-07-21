// The animation module runs inside the browser device (VIZ-61 Stage A):
// the device is constructed WITH the module, JS sets up a one-track ramp
// through the call surface (load_animation / create_player / add_instance),
// and a graph ExternalFunction node calls the module's step each tick,
// landing the sampled outputs in the device store. Mirrors the Rust
// host-side boundary proof (crates/interop/vizij-animation-module/tests/
// host_ramp.rs) through the public JS surface.
import assert from "node:assert/strict";
import { loadAnimationModule } from "@vizij/animation-module";
import { startDevice } from "../dist/arora-web-wasm/src/index.js";

// --- declared ids (module.yaml + the type records) ---------------------------
const FN_LOAD = "76697a69-6a00-0000-0f00-000000000001";
const FN_CREATE_PLAYER = "76697a69-6a00-0000-0f00-000000000002";
const FN_ADD_INSTANCE = "76697a69-6a00-0000-0f00-000000000003";
const FN_STEP = "76697a69-6a00-0000-0f00-000000000004";

const P_CLIP = "76697a69-6a00-0000-0f01-000000000001";
const P_NAME = "76697a69-6a00-0000-0f02-000000000001";
const P_PLAYER = "76697a69-6a00-0000-0f03-000000000001";
const P_ANIM = "76697a69-6a00-0000-0f03-000000000002";
const P_DT_NS = "76697a69-6a00-0000-0f04-000000000001";

const CLIP_TYPE = "76697a69-6a00-0000-0000-000000000100";
const CLIP_NAME = "76697a69-6a00-0000-0100-000000000001";
const CLIP_DURATION = "76697a69-6a00-0000-0100-000000000002";
const CLIP_TRACKS = "76697a69-6a00-0000-0100-000000000003";

const TRACK_TYPE = "76697a69-6a00-0000-0000-000000000101";
const TR_ID = "76697a69-6a00-0000-0101-000000000001";
const TR_NAME = "76697a69-6a00-0000-0101-000000000002";
const TR_ANIMATABLE = "76697a69-6a00-0000-0101-000000000003";
const TR_POINTS = "76697a69-6a00-0000-0101-000000000004";

const KP_TYPE = "76697a69-6a00-0000-0000-000000000102";
const KP_ID = "76697a69-6a00-0000-0102-000000000001";
const KP_STAMP = "76697a69-6a00-0000-0102-000000000002";
const KP_VALUE = "76697a69-6a00-0000-0102-000000000003";

const TO_TRACK_ID = "76697a69-6a00-0000-0110-000000000001";
const TO_DEFAULT_KEY = "76697a69-6a00-0000-0110-000000000002";
const TO_VALUE = "76697a69-6a00-0000-0110-000000000003";

// --- the ramp clip, in the Arora Value JSON vocabulary -----------------------
const field = (id, value) => ({ id, value });
const keypoint = (id, stamp, v) => ({
  fields: [
    field(KP_ID, { str: id }),
    field(KP_STAMP, { f32: stamp }),
    field(KP_VALUE, { f32: v }),
  ],
});
const clip = {
  struct: {
    id: CLIP_TYPE,
    fields: [
      field(CLIP_NAME, { str: "ramp" }),
      field(CLIP_DURATION, { u32: 1000 }),
      field(CLIP_TRACKS, {
        structs: {
          id: TRACK_TYPE,
          elements: [
            {
              fields: [
                field(TR_ID, { str: "t0" }),
                field(TR_NAME, { str: "ramp" }),
                field(TR_ANIMATABLE, { str: "node/x" }),
                field(TR_POINTS, {
                  structs: {
                    id: KP_TYPE,
                    elements: [keypoint("k0", 0.0, 0.0), keypoint("k1", 1.0, 1.0)],
                  },
                }),
              ],
            },
          ],
        },
      }),
    ],
  },
};

// --- the behavior: a graph whose ExternalFunction node steps the module ------
// Each tick: read the runtime's golden dt (arora/dt, nanoseconds) from the
// store, call the module's step(dt) through the engine, and write the returned
// [TrackOutput] to anim/out — the Stage-B shape (the graph drives the
// animation module; no JS clip pipeline).
const graph = {
  nodes: [
    { id: "dt", type: "input", params: { path: "arora/dt" } },
    {
      id: "step",
      type: "externalfunction",
      params: { function: FN_STEP, param_ids: [P_DT_NS] },
    },
    { id: "out", type: "output", params: { path: "anim/out" } },
  ],
  edges: [
    { from: { node_id: "dt" }, to: { node_id: "step", input: "args_0" } },
    { from: { node_id: "step" }, to: { node_id: "out", input: "in" } },
  ],
};

// --- run ---------------------------------------------------------------------
const device = await startDevice(graph, undefined, [await loadAnimationModule()]);

// Setup through the call surface. Each call dispatches inside the next step.
const pending = [
  device.call({ id: FN_LOAD, args: [field(P_CLIP, clip)] }),
  device.call({ id: FN_CREATE_PLAYER, args: [field(P_NAME, { str: "p" })] }),
];
device.step(0);
const [anim, player] = await Promise.all(pending);
assert.ok("u32" in anim.ret, "load_animation returns an animation id");
assert.ok("u32" in player.ret, "create_player returns a player id");

const pInstance = device.call({
  id: FN_ADD_INSTANCE,
  args: [field(P_PLAYER, player.ret), field(P_ANIM, anim.ret)],
});
device.step(0);
assert.ok("u32" in (await pInstance).ret, "add_instance returns an instance id");

// Two 0.25 s steps: each tick the graph feeds the golden dt into the module's
// step and lands the sampled [TrackOutput] in the store.
const firstTrack = () => {
  const out = device.readValues(["anim/out"])["anim/out"];
  assert.ok(out && out.structs, "anim/out carries the step's [TrackOutput]");
  const track = Object.fromEntries(out.structs.elements[0].fields.map((f) => [f.id, f.value]));
  return {
    trackId: track[TO_TRACK_ID].str,
    key: track[TO_DEFAULT_KEY].str,
    value: track[TO_VALUE].f32,
  };
};

device.step(250);
const first = firstTrack();
assert.equal(first.trackId, "t0");
assert.equal(first.key, "node/x", "the track carries its authored key");

device.step(250);
const second = firstTrack();
assert.ok(
  first.value > 0 && second.value > first.value,
  `ramp advances strictly upward, got ${first.value} then ${second.value}`,
);
assert.ok(
  Math.abs(second.value - 0.5) < 1e-3,
  `expected ~0.5 at the 0.5 s midpoint, got ${second.value}`,
);

device.dispose();
console.log("arora-web-wasm animation-module: ok");
