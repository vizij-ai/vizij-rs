import { execSync } from "node:child_process";
import { resolve } from "node:path";

const crate  = resolve(process.cwd(), "crates/animation/vizij-animation-wasm");
const outDir = resolve(process.cwd(), "npm/@vizij/animation-wasm/pkg");

execSync(`wasm-pack build "${crate}" --target web --out-dir "${outDir}" --release`, {
  stdio: "inherit",
});