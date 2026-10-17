// A face whose RobotData ids are plain strings, not UUIDs: Quori with every
// element and animatable id renamed (`element_<n>`, `animatable_<n>`, the
// same name wherever the GLB names the id) loads, draws as Quori does, lists
// its elements under their new ids, and a press on it picks one of them.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import { FIXTURES, meanDiff32, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser string-ids test");
  process.exit(0);
}

const CANVAS = { x: 0, y: 0, width: 763, height: 486 };
const QUORI = "Quori_Current.glb";
const UUID = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi;

/** The GLB's JSON chunk and binary chunk. */
function chunks(glb) {
  const json = JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString("utf8"));
  const binStart = 20 + glb.readUInt32LE(12);
  return { json, bin: glb.subarray(binStart) };
}

/** A GLB of `json` and the binary chunk `bin` (its 8-byte header kept). */
function pack(json, bin) {
  let text = Buffer.from(JSON.stringify(json), "utf8");
  const pad = (4 - (text.length % 4)) % 4;
  text = Buffer.concat([text, Buffer.alloc(pad, 0x20)]);
  const header = Buffer.alloc(20);
  header.writeUInt32LE(0x46546c67, 0); // glTF
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(20 + text.length + bin.length, 8);
  header.writeUInt32LE(text.length, 12);
  header.writeUInt32LE(0x4e4f534a, 16); // JSON
  return Buffer.concat([header, text, bin]);
}

/** `glb` with every RobotData element and animatable id renamed to a plain
 * string, wherever its JSON names it; the renaming, UUID → name. */
function renamed(glb) {
  const { json, bin } = chunks(glb);
  const names = new Map();
  const name = (uuid, kind) => {
    const key = uuid.toLowerCase();
    if (!names.has(key)) names.set(key, `${kind}_${names.size}`);
  };
  for (const node of json.nodes ?? []) {
    const rd = node.extensions?.RobotData;
    if (!rd) continue;
    for (const uuid of String(rd.id).match(UUID) ?? []) name(uuid, "element");
    for (const feature of Object.values(rd.features ?? {})) {
      if (!feature?.animated) continue;
      for (const uuid of String(feature.value?.id ?? "").match(UUID) ?? []) name(uuid, "animatable");
    }
  }
  const text = JSON.stringify(json).replace(UUID, (uuid) => names.get(uuid.toLowerCase()) ?? uuid);
  return { glb: pack(JSON.parse(text), bin), names };
}

const original = await fs.readFile(`${FIXTURES}/${QUORI}`);
const { glb, names } = renamed(original);
assert.ok(names.size > 10, `Quori's ids renamed: ${names.size}`);

const { page, logs, close } = await open(FIXTURES);
const harness = (name, ...args) =>
  page.evaluate(([name, args]) => window.vizijHarness[name](...args), [name, args]);
const frames = (n) =>
  page.evaluate(
    (n) =>
      new Promise((resolve) => {
        const tick = (left) => (left ? requestAnimationFrame(() => tick(left - 1)) : resolve());
        tick(n);
      }),
    n,
  );
/** Load `bytes` as the Vizij `id`, settled and drawn; the canvas then. */
const shown = async (id, bytes) => {
  await page.evaluate(
    async ([id, base64]) => {
      const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
      await window.vizijHarness.loadBytes(id, bytes, { program: "none" });
    },
    [id, bytes.toString("base64")],
  );
  await page.waitForFunction((id) => window.vizijHarness.ready(id), id, { timeout: 240_000 });
  await page.evaluate(() => {
    const h = window.vizijHarness;
    h.manual();
    h.advance(16, 10);
  });
  await frames(5);
  return page.screenshot({ clip: CANVAS });
};
// A press held over frames; what it picked (one press, however many times
// the browser delivers it).
const press = async (x, y) => {
  await harness("picks");
  await page.mouse.move(x, y);
  await frames(3);
  await page.mouse.down();
  await frames(3);
  await page.mouse.up();
  await frames(3);
  const picks = await harness("picks");
  for (const pick of picks) assert.deepEqual(pick, picks[0], `one press, ${JSON.stringify(picks)}`);
  return picks.slice(0, 1);
};

try {
  const reference = await shown("face", original);
  const plain = await shown("face", glb);
  const diff = meanDiff32(plain, reference);
  console.log(`plain ids: ${names.size} renamed, mean 32×32 diff from Quori ${diff.toFixed(2)}`);
  assert.ok(diff < 0.5, `the face draws as Quori does (diff ${diff.toFixed(2)})`);

  const described = await page.evaluate(
    (base64) =>
      window.vizijHarness.describeBytes(Uint8Array.from(atob(base64), (c) => c.charCodeAt(0))),
    glb.toString("base64"),
  );
  const ids = described.elements.map((element) => element.id);
  assert.ok(ids.length > 0 && ids.every((id) => /^element_\d+$/.test(id)), JSON.stringify(ids));

  let picks = [];
  for (let attempt = 0; attempt < 5 && picks.length === 0; attempt++) {
    picks = await press(CANVAS.width / 2, CANVAS.height / 2);
  }
  assert.equal(picks.length, 1, JSON.stringify(picks));
  assert.ok(ids.includes(picks[0].elementId), `the centre picked ${JSON.stringify(picks)}`);
  console.log(`a press at the centre picked ${picks[0].elementId}`);

  const state = await harness("state");
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser string ids: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
