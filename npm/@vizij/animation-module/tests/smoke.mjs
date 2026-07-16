// The packaged artifact is loadable and well-formed: the header parses as an
// Arora module header exporting the animation functions, and the executable
// is a wasm binary.
import assert from "node:assert/strict";
import { loadAnimationModule } from "../dist/index.js";

const { headerJson, wasmBytes } = await loadAnimationModule();

const header = JSON.parse(headerJson);
assert.equal(header.name, "vizij-animation");
const exported = header.exports.map((e) => e.name);
for (const fn of ["load_animation", "create_player", "add_instance", "step"]) {
  assert.ok(exported.includes(fn), `header exports ${fn}`);
}

assert.ok(wasmBytes.length > 0, "wasm bytes present");
const magic = Array.from(wasmBytes.slice(0, 4));
assert.deepEqual(magic, [0x00, 0x61, 0x73, 0x6d], "wasm magic number");

console.log("animation-module smoke: ok");
