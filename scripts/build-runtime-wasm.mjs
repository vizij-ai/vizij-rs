import { execSync } from "node:child_process";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";

// The browser module is the `vizij` crate itself (its `web` module), built
// without the desktop half; one WebGL2 bundle.
const crate = resolve(process.cwd(), "crates/vizij");
const outDir = resolve(process.cwd(), "npm/@vizij/runtime/pkg");

execSync(
  `wasm-pack build "${crate}" --target web --out-dir "${outDir}" --out-name vizij --release -- --no-default-features`,
  { stdio: "inherit" },
);

// ensure root .npmignore exists and is empty
writeFileSync(resolve(outDir, ".npmignore"), "");
