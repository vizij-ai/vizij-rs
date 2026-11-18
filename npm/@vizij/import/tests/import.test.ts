import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { init, optimizeMorphTriangles, stillFolding } from "../src/index.js";

const wasm = () => readFile(new URL("../../../pkg/vizij_import_wasm_bg.wasm", import.meta.url));

/** A GLB holding only `document`, with an empty binary chunk. */
function glb(document: object): Uint8Array {
  const json = new TextEncoder().encode(JSON.stringify(document));
  const jsonLength = Math.ceil(json.length / 4) * 4;
  const total = 12 + 8 + jsonLength + 8;
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  out.set(new TextEncoder().encode("glTF"), 0);
  view.setUint32(4, 2, true);
  view.setUint32(8, total, true);
  view.setUint32(12, jsonLength, true);
  out.set(new TextEncoder().encode("JSON"), 16);
  out.fill(0x20, 20, 20 + jsonLength);
  out.set(json, 20);
  view.setUint32(20 + jsonLength, 0, true);
  out.set([0x42, 0x49, 0x4e, 0x00], 24 + jsonLength);
  return out;
}

test("a file without morphed meshes is left as it is", async () => {
  await init(await wasm());
  const result = optimizeMorphTriangles(glb({ asset: { version: "2.0" } }));
  assert.equal(result.bytes, null);
  assert.deepEqual(result.report, { parts: [] });
});

test("a file that is not a GLB is rejected", async () => {
  await init(await wasm());
  assert.throws(() => optimizeMorphTriangles(new Uint8Array([1, 2, 3])));
});

test("stillFolding keeps only shapes above the threshold", () => {
  const part = {
    mesh: 0,
    meshName: "mouth",
    primitive: 0,
    vertices: 65,
    outcome: "retriangulated" as const,
    skipReason: null,
    folds: [
      { shape: "smile_shape", before: 0.4, after: 0.12 },
      { shape: "a_shape", before: 0.007, after: 0 },
      { shape: "rest", before: null, after: null },
    ],
  };
  assert.deepEqual(stillFolding({ parts: [part] }, 0.01), [
    { part, folds: [part.folds[0]] },
  ]);
});
