// Arora's animation module as these tests load it: its module directory,
// written from arora-animation-module (crates.io) by the workspace's
// `vizij-animation-artifact`, under the cargo target directory. A Vizij
// device host-links the same module, so the header names the functions and
// parameters the device describes, and the guest is a well-formed Arora
// guest for the `modules` route.
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, rmSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
// cargo builds under CARGO_TARGET_DIR when the environment sets it.
const targetDir = resolve(repoRoot, process.env.CARGO_TARGET_DIR ?? "target");
const moduleDir = resolve(targetDir, "animation-module");

const cargo = (args) =>
  execFileSync("cargo", args, { cwd: repoRoot, stdio: ["ignore", "inherit", "inherit"] });

/**
 * The module directory, written afresh: `{ headerJson, wasmBytes }`, the
 * `AroraModule` a device loads as a guest. With `{ guest: false }`, the
 * header alone, without building the guest.
 */
export function animationModule({ guest = true } = {}) {
  rmSync(moduleDir, { recursive: true, force: true });
  const write = ["run", "-q", "-p", "vizij-animation-artifact", "--", moduleDir];
  if (guest) {
    cargo(["build", "-q", "-p", "arora-animation-module", "--target", "wasm32-wasip2"]);
    write.push(resolve(targetDir, "wasm32-wasip2/debug/arora_animation_module.wasm"));
  }
  cargo(write);
  const headerJson = readFileSync(resolve(moduleDir, "header.json"), "utf8");
  if (!guest) return { headerJson };
  const guests = readdirSync(moduleDir).filter((file) => file.endsWith(".wasm"));
  if (guests.length !== 1) {
    throw new Error(`the module directory holds ${guests.length} guests: ${guests.join(", ")}`);
  }
  return {
    headerJson,
    wasmBytes: new Uint8Array(readFileSync(resolve(moduleDir, guests[0]))),
  };
}
