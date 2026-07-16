#!/usr/bin/env node
// Produce the package's `artifact/` from the Rust module crate:
//
//   artifact/vizij_animation_module.wasm  — the module executable, built with
//     `cargo build -p vizij-animation-module --target wasm32-wasip1 --release`
//   artifact/header.json — the module's Arora header, converted from the
//     build's generated `src/arora_generated/module.yaml` (the cargo build
//     runs the crate's build script, which emits it)
//
// The header ships as JSON because that is the form the browser loaders take
// (`arora-web`'s `loadModule`, `@vizij/arora-web-wasm`'s `modules` option).

import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync, copyFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import YAML from "yaml";

const pkgDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const repoRoot = resolve(pkgDir, "../../..");
const crateDir = resolve(repoRoot, "crates/interop/vizij-animation-module");
const artifactDir = resolve(pkgDir, "artifact");

execFileSync(
  "cargo",
  ["build", "-p", "vizij-animation-module", "--target", "wasm32-wasip1", "--release"],
  { cwd: repoRoot, stdio: "inherit" },
);

mkdirSync(artifactDir, { recursive: true });

const header = YAML.parse(
  readFileSync(resolve(crateDir, "src/arora_generated/module.yaml"), "utf8"),
);
writeFileSync(resolve(artifactDir, "header.json"), JSON.stringify(header, null, 2) + "\n");

copyFileSync(
  resolve(repoRoot, "target/wasm32-wasip1/release/vizij_animation_module.wasm"),
  resolve(artifactDir, "vizij_animation_module.wasm"),
);

console.log("[animation-module] artifact/ updated");
