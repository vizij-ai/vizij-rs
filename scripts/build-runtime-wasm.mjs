import { execSync } from "node:child_process";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";

// The browser module is the `vizij` crate's `web` module, built without the
// desktop half and linked by `vizij-wasm` (a cdylib, so the release profile's
// fat LTO applies); one WebGL2 bundle.
const crate = resolve(process.cwd(), "crates/vizij-wasm");
const outDir = resolve(process.cwd(), "npm/@vizij/runtime/pkg");

execSync(
  `wasm-pack build "${crate}" --target web --out-dir "${outDir}" --out-name vizij --release`,
  { stdio: "inherit" },
);

// ensure root .npmignore exists and is empty
writeFileSync(resolve(outDir, ".npmignore"), "");
