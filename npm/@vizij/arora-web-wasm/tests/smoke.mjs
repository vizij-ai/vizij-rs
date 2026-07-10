// Values flow JS → device store → arora tick → store → JS, through the
// published wrapper surface (dist/), exactly as a browser consumer uses it.
import assert from "node:assert/strict";
import { startDevice } from "../dist/arora-web-wasm/src/index.js";

const device = await startDevice(); // built-in passthrough graph: sensor/x -> actuator/y

device.setValue("sensor/x", { f32: 0.5 });
assert.equal(device.step(16), true, "device stays live");
assert.deepEqual(device.readValues(["actuator/y"]), { "actuator/y": { f32: 0.5 } });

device.writeValues({ "sensor/x": { f32: 0.25 } });
assert.equal(device.step(16), true);
assert.deepEqual(device.readValues(["actuator/y"]), { "actuator/y": { f32: 0.25 } });

const changes = device.drainChanges();
assert.deepEqual(changes["actuator/y"], { f32: 0.25 }, "change feed saw the graph write");

const snapshot = device.snapshot();
assert.deepEqual(snapshot["sensor/x"], { f32: 0.25 });

device.dispose();
console.log("arora-web-wasm smoke: ok");
