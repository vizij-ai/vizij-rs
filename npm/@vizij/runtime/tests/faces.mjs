// The handle's instances, kept from the module's pushed events, through the
// published wrapper surface (dist/), with the module's events fed by hand:
// each load's promise is settled by its own answer, an instance's answers in
// the order its loads were asked for — an unload's rejected loads kept as
// placeholders that take their answers — a key already showing or loading
// starts nothing, and `ready`, `pick` and `safeArea` reach the page's handler
// as the module sent them.
import assert from "node:assert/strict";
import { trackFaces } from "../dist/runtime/src/faces.js";

const stopped = [];
const disposed = [];
// What the module answers a load with when a later load into its instance,
// or its unload, comes first.
const REPLACED = "replaced by a later load before it finished loading";
const UNLOADED = "its instance was unloaded before it finished loading";
const tracked = trackFaces({
  stop: (instance) => stopped.push(instance),
  dispose: (device) => disposed.push(device),
  unloaded: UNLOADED,
});
const heard = [];
const seenShowing = [];
const send = tracked.dispatch((event) => {
  heard.push(event);
  // What the handle says while the handler runs.
  seenShowing.push([
    event.type,
    tracked.faces.get(event.instance)?.key,
    tracked.loading.has(event.instance),
  ]);
});

let started = 0;
const begin = (name) => () => {
  started += 1;
  return name;
};
// Each promise's outcome, as it lands.
const outcome = (promise) => {
  const seen = { state: "pending" };
  promise.then(
    (value) => Object.assign(seen, { state: "resolved", value }),
    (error) => Object.assign(seen, { state: "rejected", error: error.message }),
  );
  return seen;
};
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));
const elements = [{ id: "e1", name: "eye" }];

// A load replaced before its answer: the answers arrive in order, the first
// settles the first.
const first = outcome(tracked.load("a", "k1", begin("dev-a1")));
const second = outcome(tracked.load("a", "k2", begin("dev-a2")));
assert.deepEqual([...tracked.loading], ["a"]);
send({ type: "loadFailed", instance: "a", key: "k1", error: REPLACED });
await tick();
assert.equal(first.state, "rejected");
assert.match(first.error, /replaced/);
assert.equal(second.state, "pending", "the later load waits for its own answer");
assert.deepEqual(disposed, ["dev-a1"], "a device whose face never showed is freed");
assert.ok(tracked.loading.has("a"), "still loading the second");

send({ type: "loaded", instance: "a", key: "k2", elements });
await tick();
assert.deepEqual(second, { state: "resolved", value: "dev-a2" });
assert.deepEqual(tracked.faces.get("a"), { instance: "a", key: "k2", elements });
assert.equal(tracked.loading.has("a"), false);
assert.deepEqual(
  seenShowing.at(-1),
  ["loaded", "k2", false],
  "the handle is current before the handler runs",
);

// A key the instance shows, with none loading, starts nothing and resolves
// to the device showing it.
const before = started;
assert.equal(await tracked.load("a", "k2", begin("never")), "dev-a2");
assert.equal(started, before, "nothing started");

// A key the instance is loading joins that load.
const loading = tracked.load("a", "k3", begin("dev-a3"));
const joined = tracked.load("a", "k3", begin("never"));
assert.equal(joined, loading, "the repeat is the same load");
assert.equal(started, before + 1);
// Asking for the key showing while another loads replaces that load.
const back = outcome(tracked.load("a", "k2", begin("dev-a4")));
assert.equal(started, before + 2, "the key showing, asked for while another loads, loads again");
const loadingOutcome = outcome(loading);
send({ type: "loadFailed", instance: "a", key: "k3", error: REPLACED });
send({ type: "loaded", instance: "a", key: "k2", elements });
await tick();
assert.equal(loadingOutcome.state, "rejected");
assert.deepEqual(back, { state: "resolved", value: "dev-a4" });

// An unload rejects what is loading at once; the module's later answer for
// it is taken by its placeholder, not by a load asked for after the unload.
const unloaded = outcome(tracked.load("b", undefined, begin("dev-b1")));
tracked.unload("b");
await tick();
assert.deepEqual(stopped, ["b"]);
assert.equal(unloaded.state, "rejected");
assert.equal(unloaded.error, UNLOADED, "rejected with the module's own answer");
assert.ok(disposed.includes("dev-b1"));
assert.equal(tracked.loading.has("b"), false);
const after = outcome(tracked.load("b", undefined, begin("dev-b2")));
send({ type: "loadFailed", instance: "b", error: UNLOADED });
await tick();
assert.equal(after.state, "pending", "the placeholder took the unload's answer");
assert.ok(tracked.loading.has("b"));
send({ type: "loaded", instance: "b", elements: [] });
await tick();
assert.deepEqual(after, { state: "resolved", value: "dev-b2" });
assert.equal(disposed.includes("dev-b2"), false, "a device handed to the page is the page's");

// A device is freed once: by the unload, not again by a second unload or by
// the module's later answer for its load.
const twice = outcome(tracked.load("d", undefined, begin("dev-d1")));
tracked.unload("d");
tracked.unload("d");
send({ type: "loadFailed", instance: "d", error: UNLOADED });
await tick();
assert.equal(twice.state, "rejected");
assert.equal(disposed.filter((device) => device === "dev-d1").length, 1, "freed once");

// A load with no key names no content, so it is never taken for a repeat:
// each one starts, and each settles on its own answer.
const startedBefore = started;
const plain1 = outcome(tracked.load("e", undefined, begin("dev-e1")));
const plain2 = outcome(tracked.load("e", undefined, begin("dev-e2")));
assert.equal(started, startedBefore + 2, "both unkeyed loads start");
send({ type: "loadFailed", instance: "e", error: REPLACED });
send({ type: "loaded", instance: "e", elements: [] });
await tick();
assert.equal(plain1.state, "rejected");
assert.deepEqual(plain2, { state: "resolved", value: "dev-e2" });

// An unload drops the face showing, so its key loads again.
tracked.unload("a");
assert.equal(tracked.faces.has("a"), false);
const reloaded = outcome(tracked.load("a", "k2", begin("dev-a5")));
send({ type: "loaded", instance: "a", key: "k2", elements });
await tick();
assert.deepEqual(reloaded, { state: "resolved", value: "dev-a5" });

// A load the module refuses outright rejects with its error, queueing
// nothing: the next answer settles the next load.
const refused = outcome(
  tracked.load("c", undefined, () => {
    throw new Error("the GLB does not read");
  }),
);
await tick();
assert.equal(refused.state, "rejected");
assert.equal(tracked.loading.has("c"), false);
const next = outcome(tracked.load("c", undefined, begin("dev-c")));
send({ type: "loaded", instance: "c", elements: [] });
await tick();
assert.deepEqual(next, { state: "resolved", value: "dev-c" });

// `ready`, `pick` and `safeArea` reach the handler as sent; the safe areas
// are kept.
const ready = { type: "ready", instance: "a", drawn: true };
const gaveUp = { type: "ready", instance: "c", drawn: false };
const pick = { type: "pick", instance: "a", element: "e1" };
const miss = { type: "pick", instance: "a", element: null };
const area = { type: "safeArea", instance: "a", area: { x: 1, y: 2, width: 3, height: 4 } };
const gone = { type: "safeArea", instance: "a", area: null };
for (const event of [ready, gaveUp, pick, miss, area]) send(event);
assert.deepEqual(heard.slice(-5), [ready, gaveUp, pick, miss, area]);
assert.deepEqual(tracked.safeAreas.get("a"), area.area);
send(gone);
assert.equal(tracked.safeAreas.has("a"), false);

// Readiness is the handle's, kept from `ready` and set before the handler
// hears it. It is taken out when a load into the instance starts — not when
// the new face has loaded, which is still not drawn — and when the instance
// is unloaded; a failed load puts back the face it was to replace, as it
// was drawn, and one that is not the last load asked for puts back nothing.
{
  const seen = [];
  const r = trackFaces({ stop() {}, dispose() {}, unloaded: UNLOADED });
  const sendR = r.dispatch((event) => seen.push([event.type, r.ready.get(event.instance)]));
  const failed = (key) =>
    sendR({ type: "loadFailed", instance: "x", key, error: "its scene failed to load" });
  r.load("x", "k1", () => "dev-x1").catch(() => {});
  sendR({ type: "loaded", instance: "x", key: "k1", elements: [] });
  assert.equal(r.ready.has("x"), false, "loaded is not drawn");
  sendR({ type: "ready", instance: "x", drawn: true });
  assert.equal(r.ready.get("x"), true);
  assert.deepEqual(seen.at(-1), ["ready", true], "ready before the handler hears it");

  // A reload: not ready from its start, through its `loaded`, until drawn.
  r.load("x", "k2", () => "dev-x2").catch(() => {});
  assert.equal(r.ready.has("x"), false, "a reload is not ready from its start");
  sendR({ type: "loaded", instance: "x", key: "k2", elements: [] });
  assert.equal(r.ready.has("x"), false, "nor once the new face has loaded");
  sendR({ type: "ready", instance: "x", drawn: false });
  assert.equal(r.ready.get("x"), false, "drawn as its `ready` said");

  // A failed reload puts the face kept back as it was.
  r.load("x", "k3", () => "dev-x3").catch(() => {});
  assert.equal(r.ready.has("x"), false);
  failed("k3");
  assert.equal(r.ready.get("x"), false, "the face kept is ready again");

  // Two reloads: the first failing leaves the second loading, not ready.
  r.load("x", "k4", () => "dev-x4").catch(() => {});
  r.load("x", "k5", () => "dev-x5").catch(() => {});
  failed("k4");
  assert.equal(r.ready.has("x"), false, "a load still under way");
  sendR({ type: "loaded", instance: "x", key: "k5", elements: [] });
  assert.equal(r.ready.has("x"), false);
  sendR({ type: "ready", instance: "x", drawn: true });
  assert.equal(r.ready.get("x"), true);

  // A load that starts nothing, or that the module refuses, changes nothing.
  r.load("x", "k5", () => "never").catch(() => {});
  r.load("x", undefined, () => {
    throw new Error("the GLB does not read");
  }).catch(() => {});
  assert.equal(r.ready.get("x"), true, "nothing started, nothing reset");

  // A face that failed to replace one never drawn leaves it not ready, until
  // its own `ready`.
  r.load("y", undefined, () => "dev-y1").catch(() => {});
  sendR({ type: "loaded", instance: "y", elements: [] });
  r.load("y", undefined, () => "dev-y2").catch(() => {});
  sendR({ type: "loadFailed", instance: "y", error: "its scene failed to load" });
  assert.equal(r.ready.has("y"), false);
  sendR({ type: "ready", instance: "y", drawn: true });
  assert.equal(r.ready.get("y"), true);

  r.unload("x");
  assert.equal(r.ready.has("x"), false, "an unload takes it out");
  await tick();
}

// A handler set later replaces the first; loads settle whether or not one is set.
const quiet = trackFaces({ stop() {}, dispose() {}, unloaded: UNLOADED });
const silent = outcome(quiet.load("q", undefined, () => "dev-q"));
quiet.dispatch()({ type: "loaded", instance: "q", elements: [] });
await tick();
assert.deepEqual(silent, { state: "resolved", value: "dev-q" });

console.log("@vizij/runtime faces: ok");
