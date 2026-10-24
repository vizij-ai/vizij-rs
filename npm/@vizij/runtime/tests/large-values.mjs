// A large value costs its size: `setValue` and `invoke` read a value's text
// straight into an Arora value, so the wasm heap grows by a small multiple of
// that text at most. V8 grows a wasm memory a page at a time, each grow costing
// in the memory's size, so a call whose working set is a large multiple of its
// text costs superlinearly the first time — a long animation's `load_clip` on
// every authoring edit. The module's memory never shrinks, so what one call
// grows it by is the most that call held at once beyond what the heap had.
import assert from "node:assert/strict";
import {
  composeVizij,
  memoryBytes,
  startRuntime,
} from "../dist/runtime/src/index.js";

const graph = await composeVizij(
  { extensions: { VIZIJ_bundle: { graphs: [] } } },
  { animations: true }
);
const runtime = await startRuntime(graph);
const settle = async (pending) => {
  let done = false;
  pending.then(
    () => (done = true),
    () => (done = true)
  );
  for (let i = 0; !done; i++) {
    assert.ok(i < 10, "the promise resolves within a few steps");
    runtime.step(10);
    await new Promise((resolve) => setImmediate(resolve));
  }
  return pending;
};
await settle(runtime.describeMethods());

// One track of 2000 keypoints: about 2 MB of text as `load_clip` takes it.
const keypoints = 2000;
const { moduleAnimation } = runtime.moduleAnimation({
  id: "long",
  name: "Long",
  duration: keypoints,
  tracks: [
    {
      channel: "face/x",
      interpolation: "linear",
      keyframes: Array.from({ length: keypoints }, (_, i) => ({
        time: i,
        value: Math.sin(i),
        interpolation: null,
      })),
    },
  ],
});
const text = JSON.stringify(moduleAnimation).length;

// Through a JSON tree, either call grows the heap by about 24 times the text;
// read straight, by about 3.
const MOST_PER_TEXT_BYTE = 6;
const grows = async (what, call) => {
  const before = memoryBytes();
  await call();
  const grown = memoryBytes() - before;
  assert.ok(
    grown <= MOST_PER_TEXT_BYTE * text,
    `${what} of ${text} bytes of text grew the heap by ${grown} bytes ` +
      `(${(grown / text).toFixed(1)}x, more than ${MOST_PER_TEXT_BYTE}x)`
  );
};

await grows("setValue", () => runtime.setValue("probe/long", moduleAnimation));
await grows("invoke", async () => {
  const anim = await settle(
    runtime.invoke("load_clip", { clip: moduleAnimation })
  );
  assert.equal(
    typeof anim?.u32,
    "number",
    "load_clip answers the animation's id"
  );
});
