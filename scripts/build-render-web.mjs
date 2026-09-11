/**
 * Builds the browser renderer and stages the harness that exercises it.
 *
 * `cargo build` plus `wasm-bindgen` rather than `wasm-pack`: the other wasm
 * crates here ship as npm packages and wasm-pack's packaging is what they want,
 * while this one is loaded by a page directly. The bindgen CLI's version has to
 * match the `wasm-bindgen` crate in the lockfile exactly — the schema is
 * unstable between releases — so this checks before building rather than
 * failing afterwards on a mismatched artifact.
 */
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, copyFileSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const crate = resolve(root, "crates/render/vizij-render-web");
const harness = resolve(crate, "harness");
const wasm = resolve(root, "target/wasm32-unknown-unknown/release/vizij_render_web.wasm");

const run = (cmd, args) => execFileSync(cmd, args, { stdio: "inherit", cwd: root });

/** The `wasm-bindgen` version the lockfile resolves, which the CLI must match. */
function lockedBindgenVersion() {
  const lock = readFileSync(resolve(root, "Cargo.lock"), "utf8");
  const match = lock.match(/\[\[package\]\]\nname = "wasm-bindgen"\nversion = "([^"]+)"/);
  if (!match) throw new Error("no wasm-bindgen entry in Cargo.lock");
  return match[1];
}

const wanted = lockedBindgenVersion();
let installed;
try {
  installed = execFileSync("wasm-bindgen", ["--version"], { encoding: "utf8" })
    .trim()
    .split(/\s+/)[1];
} catch {
  throw new Error(
    `wasm-bindgen CLI not found. Install it with:\n  cargo install wasm-bindgen-cli --version ${wanted}`,
  );
}
if (installed !== wanted) {
  throw new Error(
    `wasm-bindgen CLI is ${installed} but the lockfile wants ${wanted}; the bindgen schema must match exactly. Fix with:\n` +
      `  cargo install -f wasm-bindgen-cli --version ${wanted}`,
  );
}

run("cargo", ["build", "--release", "-p", "vizij-render-web", "--target", "wasm32-unknown-unknown"]);
run("wasm-bindgen", [
  "--target",
  "web",
  "--out-dir",
  resolve(harness, "pkg"),
  "--out-name",
  "vizij_render_web",
  wasm,
]);

// The harness renders the same faces the snapshot regression does. The
// `snapshot-regression` CI job fetches them into `fixtures/`; do the same
// locally to serve the harness.
const assets = resolve(harness, "assets");
mkdirSync(assets, { recursive: true });
for (const glb of ["Quori_Current_Extended.glb", "Toasty_Current.glb"]) {
  const from = resolve(root, "fixtures", glb);
  if (existsSync(from)) copyFileSync(from, resolve(assets, glb));
  else console.warn(`fixtures/${glb} missing — the harness will 404 on it`);
}

console.log(`\nbuilt. serve the harness with:\n  node scripts/serve-render-harness.mjs`);
