// Arora wasm modules load into a device as guests: `startRuntime(graph,
// init, modules)` and `loadFace`'s `options.modules` take `{ headerJson,
// wasmBytes }` pairs, instantiated by the device's engine at build. The only
// artifact around is the animation module, whose id the host-linked engine
// serves — a host module registered under an id a guest holds replaces it —
// so what this proves is the loading: a guest that is not a wasm module
// fails the build, a well-formed one loads and the device runs.
import assert from "node:assert/strict";
import { loadAnimationModule } from "@vizij/animation-module";
import { startRuntime } from "../dist/runtime/src/index.js";

const module = await loadAnimationModule();
assert.ok(module.wasmBytes.length > 0 && module.headerJson.length > 0);

// A guest the engine cannot instantiate fails the build, so a guest is
// never silently dropped.
await assert.rejects(
  startRuntime(undefined, undefined, [{ headerJson: module.headerJson, wasmBytes: new Uint8Array([0, 1, 2, 3]) }]),
  /failed to load module|arora build failed/,
  "a malformed guest fails the build",
);

// A well-formed guest loads; the device steps and its functions answer.
const FN_CREATE_PLAYER = "76697a69-6a00-0000-0f00-000000000002";
const P_NAME = "76697a69-6a00-0000-0f02-000000000001";
const runtime = await startRuntime(undefined, undefined, [module]);
const pending = runtime.call({ id: FN_CREATE_PLAYER, args: [{ id: P_NAME, value: { str: "p" } }] });
runtime.step(0);
assert.ok("u32" in (await pending).ret, "create_player answers on a device with a guest loaded");
runtime.dispose();
console.log("@vizij/runtime guest-module: ok");
