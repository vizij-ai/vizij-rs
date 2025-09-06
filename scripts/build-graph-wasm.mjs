import { execSync } from "node:child_process";
import { resolve } from "node:path";

const crate  = resolve(process.cwd(), "crates/node-graph/vizij-graph-wasm");
const outDir = resolve(process.cwd(), "npm/@vizij/node-graph-wasm/pkg");

execSync(`wasm-pack build "${crate}" --target web --out-dir "${outDir}" --release`, {
  stdio: "inherit",
});