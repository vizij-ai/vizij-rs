// Arora wasm modules load into a device as guests: `startRuntime(graph,
// init, modules)` and `loadVizij`'s `options.modules` take `{ headerJson,
// wasmBytes }` pairs, instantiated by the device's engine at build. The guest
// is Arora's animation module, built from arora-animation-module on
// crates.io (`animation-guest.mjs`): a component of Arora's `arora:module`
// world, its header naming the `wasm-component` executor. A device loads one
// module per id and host-links this module under the header's id, so the
// guest loads under an id of its own, where its functions answer from the
// guest.
import assert from "node:assert/strict";
import { startRuntime } from "../dist/runtime/src/index.js";
import { animationModule } from "./animation-guest.mjs";

const module = animationModule();
assert.ok(module.wasmBytes.length > 0 && module.headerJson.length > 0);
const header = JSON.parse(module.headerJson);
assert.equal(header.executor.name, "wasm-component", "the guest is a component");

// A guest under the id of a module the device host-links fails the build,
// naming both.
await assert.rejects(
  startRuntime(undefined, undefined, [module]),
  /guest module 'animation' and the host module describing .* both have id/,
  "a guest under the host-linked module's id fails the build",
);

const OWN_ID = crypto.randomUUID();
const own = { headerJson: JSON.stringify({ ...header, id: OWN_ID }), wasmBytes: module.wasmBytes };

// A guest the engine cannot instantiate fails the build, so a guest is
// never silently dropped.
await assert.rejects(
  startRuntime(undefined, undefined, [{ ...own, wasmBytes: new Uint8Array([0, 1, 2, 3]) }]),
  /failed to load module 'animation'/,
  "a malformed guest fails the build",
);

// A well-formed guest loads; the device steps and the guest's functions
// answer.
const FN_CREATE_PLAYER = "76697a69-6a00-0000-0f00-000000000002";
const P_NAME = "76697a69-6a00-0000-0f02-000000000001";
const runtime = await startRuntime(undefined, undefined, [own]);
const pending = runtime.call({ module_id: OWN_ID, id: FN_CREATE_PLAYER, args: [{ id: P_NAME, value: { str: "p" } }] });
runtime.step(0);
assert.ok("u32" in (await pending).ret, "the guest's create_player answers");
runtime.dispose();
console.log("@vizij/runtime guest-module: ok");
