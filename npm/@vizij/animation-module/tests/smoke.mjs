// The packaged artifact is loadable and well-formed: the header parses as an
// Arora module header exporting the animation functions, the executable is a
// wasm binary, and the records describe the module and the structures its
// functions return, in the `vizij` scope, as `artifact/records/` ships them.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { loadAnimationModule } from "../dist/index.js";

const { headerJson, wasmBytes, records } = await loadAnimationModule();

const header = JSON.parse(headerJson);
assert.equal(header.name, "animation");
const exported = header.exports.map((e) => e.name);
for (const fn of ["load_animation", "create_player", "add_instance", "step"]) {
  assert.ok(exported.includes(fn), `header exports ${fn}`);
}

assert.ok(wasmBytes.length > 0, "wasm bytes present");
const magic = Array.from(wasmBytes.slice(0, 4));
assert.deepEqual(magic, [0x00, 0x61, 0x73, 0x6d], "wasm magic number");

// The `vizij` scope's folder, at the registry root.
const VIZIJ_SCOPE = "76697a69-6a00-0000-f01d-000000000000";
const STEP_VALUES = "76697a69-6a00-0000-0000-000000000113";
const lines = (yaml) => yaml.split("\n");
const folder = records[`folder/${VIZIJ_SCOPE}.yaml`];
assert.ok(folder, "the vizij scope's folder record is shipped");
assert.ok(lines(folder).includes("name: vizij"), "the scope's folder is named by its handle");

const { major, minor, patch } = header.version;
const module = records[`module/${header.id}@${major}.${minor}.${patch}.yaml`];
assert.ok(module, "the module record is shipped at the header's id and version");
assert.ok(lines(module).includes(`parent: ${VIZIJ_SCOPE}`), "the module record is in the vizij scope");
assert.ok(lines(module).includes(`name: ${header.name}`), "the module record is named as the header");
for (const fn of ["step_values", "player_states", "output_keys"]) {
  assert.ok(lines(module).includes(`    name: ${fn}`), `the module record describes ${fn}`);
}

const stepValues = records[`structure/${STEP_VALUES}@1.0.0.yaml`];
assert.ok(stepValues, "StepValues' structure record is shipped");
assert.ok(lines(stepValues).includes("name: StepValues"));
assert.ok(lines(stepValues).includes(`parent: ${VIZIJ_SCOPE}`), "StepValues is in the vizij scope");
for (const field of ["revision", "values"]) {
  assert.ok(lines(stepValues).includes(`    name: ${field}`), `StepValues has its ${field} field`);
}

for (const [path, yaml] of Object.entries(records)) {
  const file = fileURLToPath(new URL(`../artifact/records/${path}`, import.meta.url));
  assert.equal(readFileSync(file, "utf8"), yaml, `artifact/records/${path} is the inlined record`);
}

console.log("animation-module smoke: ok");
