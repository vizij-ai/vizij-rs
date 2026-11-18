import { execSync } from "node:child_process";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";

const crate = resolve(process.cwd(), "crates/tools/vizij-import-wasm");
const outDir = resolve(process.cwd(), "npm/@vizij/import/pkg");

execSync(`wasm-pack build "${crate}" --target web --out-dir "${outDir}" --release`, {
  stdio: "inherit",
});

// wasm-pack writes a .gitignore that would keep pkg/ out of the published
// package; an empty .npmignore takes precedence.
writeFileSync(resolve(outDir, ".npmignore"), "");
