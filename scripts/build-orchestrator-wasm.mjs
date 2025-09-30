import { execSync } from "node:child_process";
import { resolve } from "node:path";
import { writeFileSync } from "node:fs";

const crate  = resolve(process.cwd(), "crates/orchestrator/vizij-orchestrator-wasm");
const outDir = resolve(process.cwd(), "npm/@vizij/orchestrator-wasm/pkg");

execSync(`wasm-pack build "${crate}" --target web --out-dir "${outDir}" --release --features urdf_ik`, {
  stdio: "inherit",
});

// ensure root .npmignore exists and is empty
writeFileSync(resolve(outDir, ".npmignore"), "");
