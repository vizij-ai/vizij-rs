import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { init, optimizeMorphTriangles, stillFolding } from "../src/index.js";

const wasm = () => readFile(new URL("../../../pkg/vizij_import_wasm_bg.wasm", import.meta.url));

/** A GLB holding `document` and `bin` (4-byte aligned) as its chunks. */
function glb(document: object, bin = new Uint8Array(0)): Uint8Array {
  const json = new TextEncoder().encode(JSON.stringify(document));
  const jsonLength = Math.ceil(json.length / 4) * 4;
  const total = 12 + 8 + jsonLength + 8 + bin.length;
  const out = new Uint8Array(total);
  const view = new DataView(out.buffer);
  out.set(new TextEncoder().encode("glTF"), 0);
  view.setUint32(4, 2, true);
  view.setUint32(8, total, true);
  view.setUint32(12, jsonLength, true);
  out.set(new TextEncoder().encode("JSON"), 16);
  out.fill(0x20, 20, 20 + jsonLength);
  out.set(json, 20);
  view.setUint32(20 + jsonLength, bin.length, true);
  out.set([0x42, 0x49, 0x4e, 0x00], 24 + jsonLength);
  out.set(bin, 28 + jsonLength);
  return out;
}

/**
 * A unit square split as a fan from vertex 0, with one morph target that
 * folds that fan only halfway through: the pass rewrites its triangles.
 */
function foldingSquare(): Uint8Array {
  const rest = [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0];
  const turned = [1.4, 0.3, 0, -1.0, 0.8, 0, 0.5, -1.8, 0, 1.0, -1.1, 0];
  const positions = new Float32Array(rest);
  const deltas = new Float32Array(turned.map((v, i) => v - rest[i]));
  const indices = new Uint16Array([0, 1, 2, 0, 2, 3]);
  const bin = new Uint8Array(48 + 12 + 48);
  bin.set(new Uint8Array(positions.buffer), 0);
  bin.set(new Uint8Array(indices.buffer), 48);
  bin.set(new Uint8Array(deltas.buffer), 60);
  const vec3 = { componentType: 5126, count: 4, type: "VEC3" };
  return glb(
    {
      asset: { version: "2.0" },
      buffers: [{ byteLength: bin.length }],
      bufferViews: [
        { buffer: 0, byteOffset: 0, byteLength: 48 },
        { buffer: 0, byteOffset: 48, byteLength: 12 },
        { buffer: 0, byteOffset: 60, byteLength: 48 },
      ],
      accessors: [
        { bufferView: 0, ...vec3 },
        { bufferView: 1, componentType: 5123, count: 6, type: "SCALAR" },
        { bufferView: 2, ...vec3 },
      ],
      meshes: [
        {
          name: "quad",
          primitives: [
            { attributes: { POSITION: 0 }, indices: 1, targets: [{ POSITION: 2 }] },
          ],
          extras: { targetNames: ["turn"] },
        },
      ],
    },
    bin,
  );
}

test("a file without morphed meshes is left as it is", async () => {
  await init(await wasm());
  const result = optimizeMorphTriangles(glb({ asset: { version: "2.0" } }), "safe");
  assert.equal(result.bytes, null);
  assert.deepEqual(result.report, { parts: [] });
});

test("a file whose triangles fold comes back rewritten, with a report", async () => {
  await init(await wasm());
  const input = foldingSquare();
  const result = optimizeMorphTriangles(input, "safe");
  assert.ok(result.bytes, "the pass rewrites the folding fan");
  assert.equal(result.bytes.length, input.length);
  // A fresh copy owns its whole buffer; a view into wasm memory would not.
  assert.ok(result.bytes.buffer instanceof ArrayBuffer);
  assert.equal(result.bytes.byteOffset, 0);
  assert.equal(result.bytes.buffer.byteLength, result.bytes.length);
  assert.notDeepEqual(result.bytes, input);
  assert.equal(result.report.parts[0].meshName, "quad");
  assert.equal(result.report.parts[0].outcome, "retriangulated");
  assert.ok(
    result.report.parts[0].folds.some((fold) => fold.shape === "turn (halfway)"),
  );
});

test("a file that is not a GLB is rejected", async () => {
  await init(await wasm());
  assert.throws(() => optimizeMorphTriangles(new Uint8Array([1, 2, 3]), "safe"));
  assert.throws(
    () => optimizeMorphTriangles(foldingSquare(), "bold" as "safe"),
    /unknown mode/,
  );
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
      { shape: "smile_shape", kind: "target" as const, before: 0.4, after: 0.12, visiblyWorse: false, visiblyBetter: false },
      { shape: "a_shape", kind: "target" as const, before: 0.007, after: 0, visiblyWorse: false, visiblyBetter: false },
      { shape: "rest", kind: "rest" as const, before: null, after: null, visiblyWorse: false, visiblyBetter: false },
    ],
  };
  assert.deepEqual(stillFolding({ parts: [part] }, 0.01), [
    { part, folds: [part.folds[0]] },
  ]);
});
