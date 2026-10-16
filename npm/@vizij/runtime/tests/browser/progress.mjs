// init() reports its progress: a listener subscribed before it sees the
// download counted up to the module's size against its Content-Length, then
// compile, start and ready, the download reported at most every 50 ms while
// a slow network delivers it. The fetched response itself reaches
// instantiateStreaming, so the module compiles as it streams in and Chrome
// can cache the compiled code — it caches none for a Response built in
// script, whose `url` is empty. Served compressed, the download has no
// total; served under the wrong MIME type, the module still loads. A
// listener subscribing after init is called with `ready` at once, and an
// animation frame runs between compile, start and ready, so each is drawn.
import assert from "node:assert/strict";
import { statSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { FIXTURES, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser progress test");
  process.exit(0);
}

const WASM = "/dist/pkg/vizij_bg.wasm";
const size = statSync(fileURLToPath(new URL(`../..${WASM}`, import.meta.url))).size;

// Records the URL of every response instantiateStreaming is given, and
// counts the animation frames.
const recordStreaming = () => {
  window.frameCount = 0;
  const frame = () => {
    window.frameCount += 1;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  window.streamed = [];
  const original = WebAssembly.instantiateStreaming;
  WebAssembly.instantiateStreaming = async function (source, imports) {
    const response = await source;
    window.streamed.push(response.url);
    return original.call(this, response, imports);
  };
};

async function load(wasm) {
  const query = wasm ? `?wasm=${encodeURIComponent(wasm)}` : "";
  const { page, logs, close } = await open(FIXTURES, query, { initScript: recordStreaming });
  try {
    const [progress, now, streamed] = await page.evaluate(() => [
      window.vizijHarness.progress(),
      window.vizijHarness.progressNow(),
      window.streamed,
    ]);
    return { progress, now, streamed, logs };
  } finally {
    await close();
  }
}

/** The stages in order, a download run collapsed into one entry. */
const stages = (progress) =>
  progress.map((p) => p.stage).filter((stage, i, all) => stage !== all[i - 1]);

function checkDownload(progress, total) {
  const { at, frame } = progress[0];
  assert.deepEqual(progress[0], { stage: "download", loaded: 0, total: null, at, frame });
  assert.deepEqual(stages(progress), ["download", "compile", "start", "ready"]);
  const downloads = progress.filter((p) => p.stage === "download").slice(1);
  assert.ok(downloads.length >= 2, "the response's headers and its last byte are reported");
  assert.equal(downloads[0].loaded, 0);
  assert.equal(downloads.at(-1).loaded, size, "the final report counts the whole module");
  for (const d of downloads) assert.equal(d.total, total);
  for (let i = 1; i < downloads.length; i++) {
    assert.ok(downloads[i].loaded >= downloads[i - 1].loaded, "the count only grows");
  }
  // Every report but the final count waits 50 ms after the previous one.
  for (let i = 2; i < downloads.length - 1; i++) {
    const gap = downloads[i].at - downloads[i - 1].at;
    assert.ok(gap >= 49, `download reported ${gap.toFixed(1)} ms after the previous one`);
  }
}

/** Whether an animation frame ran between the reports of `from` and `to`:
 * a listener's drawing of `from` reached the screen. */
function framed(progress, from, to) {
  const a = progress.find((p) => p.stage === from);
  const b = progress.find((p) => p.stage === to);
  return b.frame > a.frame;
}

const plain = await load();
checkDownload(plain.progress, size);
assert.ok(framed(plain.progress, "compile", "start"), "a frame shows compile");
assert.ok(framed(plain.progress, "start", "ready"), "a frame shows start");
assert.deepEqual(plain.now, [{ stage: "ready" }]);
assert.equal(plain.streamed.length, 1);
assert.ok(plain.streamed[0].endsWith(WASM), `streamed ${plain.streamed[0]}`);
console.log(`progress: ${stages(plain.progress).join(", ")}`);

// About a second of delivery: one report per 50 ms of it, give or take.
const paced = await load(`${WASM}?paced`);
checkDownload(paced.progress, size);
const reports = paced.progress.filter((p) => p.stage === "download").length;
assert.ok(reports >= 5, `${reports} download reports over a paced delivery`);
console.log(`progress: ${reports} download reports over a paced delivery`);

const gzip = await load(`${WASM}?encoding=gzip`);
checkDownload(gzip.progress, null);
console.log("progress: a compressed module reports no total");

const mime = await load(`${WASM}?type=application/octet-stream`);
checkDownload(mime.progress, size);
assert.ok(
  mime.logs.some((l) => l.includes("application/wasm")),
  "wasm-bindgen falls back on the wrong MIME type",
);
console.log("progress: a module served under the wrong MIME type still loads");
