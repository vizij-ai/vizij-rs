# Vizij — Local Dev (Rust→WASM→Web) with `npm link`

This repo pair uses a **split build**:

* **`vizij-rs`** (producer): Rust crates + `wasm-bindgen` FFI; builds and ships the npm package **`@vizij/animation-wasm`**.
* **`vizij-web`** (consumer): web workspace (React wrapper + demo/website) that depends on `@vizij/animation-wasm`.

During local development we wire the two repos together with **`npm link`** so you can edit Rust and immediately test in the demo app.

## Why this approach?

* **Decoupled CI:** web CI doesn’t need the Rust toolchain or `wasm-pack`. It consumes published npm packages (or the local link).
* **Fast inner loop:** edit Rust ➜ rebuild WASM ➜ Vite dev server refreshes ➜ see changes.
* **Clear boundaries:** the WASM package (`@vizij/animation-wasm`) is the distribution artifact for web consumers.

---

## Layout

```
vizij-rs/                              # Rust workspace + npm wrapper (producer)
  crates/animation/
    vizij-animation-core/              # engine-agnostic logic
    bevy_vizij_animation/              # Bevy plugin
    vizij-animation-wasm/              # wasm-bindgen cdylib
  npm/@vizij/animation-wasm/           # npm wrapper that re-exports pkg/
    pkg/                               # wasm-pack output (generated)
    src/index.ts                       # stable ESM entry (exports default init + named APIs)

vizij-web/                             # Web workspace (consumer)
  packages/@vizij/animation-react/     # React provider + hooks
  apps/demo-animation/                 # demo app (Vite)
```

---

## One-time Vite dev configuration

Because we use `npm link`, the package lives under `node_modules` as a symlink. We keep the symlink **inside** `node_modules` and tell Vite to **watch** it.

**`vizij-web/apps/demo-animation/vite.config.ts`**

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";

export default defineConfig({
  plugins: [react()],
  // Keep symlinks resolved as symlinks under node_modules
  resolve: { preserveSymlinks: true },
  server: {
    // Watch our linked dep under node_modules (Vite ignores node_modules by default)
    watch: {
      ignored: [
        "**/node_modules/**",
        "!**/node_modules/@vizij/animation-wasm/**",
      ],
    },
    headers: {
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    },
  },
  optimizeDeps: {
    // Let Vite load the ESM glue directly instead of pre-bundling it
    exclude: ["@vizij/animation-wasm"],
  },
});
```

---

## Scripts (recommended)

### In `vizij-rs` (producer)

Add a tiny helper so the `pkg/` always lands in the wrapper:

**`vizij-rs/scripts/build-wasm.mjs`**

```js
import { execSync } from "node:child_process";
import { resolve } from "node:path";

const crate  = resolve(process.cwd(), "crates/animation/vizij-animation-wasm");
const outDir = resolve(process.cwd(), "npm/@vizij/animation-wasm/pkg");

execSync(`wasm-pack build "${crate}" --target web --out-dir "${outDir}" --release`, {
  stdio: "inherit",
});
```

Optional: auto-rebuild on Rust changes (requires `cargo install cargo-watch`):

**`vizij-rs/package.json`** (create if missing)

```json
{
  "name": "vizij-rs-scripts",
  "private": true,
  "scripts": {
    "build:wasm": "node scripts/build-wasm.mjs",
    "watch:wasm": "cargo watch -w crates/animation/vizij-animation-core -w crates/animation/vizij-animation-wasm -s \"node scripts/build-wasm.mjs\"",
    "link:wasm": "npm --workspace npm/@vizij/animation-wasm run build && (cd npm/@vizij/animation-wasm && npm link)",
    "clean:wasm": "rm -rf npm/@vizij/animation-wasm/pkg npm/@vizij/animation-wasm/dist"
  },
  "workspaces": [
    "npm/@vizij/*"
  ]
}
```

**Notes**

* Ensure the wrapper’s `src/index.ts` exports both **default** and **named** exports:

  ```ts
  import init, { Animation, abi_version } from "../pkg/vizij_animation_wasm.js";
  export default init;
  export { init, Animation, abi_version };
  export type { AnimationConfig, AnimationOutputs } from "./types";
  ```
* In the wrapper’s `package.json`, include the `pkg` in the publish tarball:

  ```json
  "files": ["dist", "pkg", "README.md"]
  ```

### In `vizij-web` (consumer, at the repo root)

**`vizij-web/package.json`**

```json
{
  "name": "vizij-web",
  "private": true,
  "workspaces": ["packages/*/*", "apps/*"],
  "scripts": {
    "link:wasm": "npm link @vizij/animation-wasm",
    "dev": "npm run --workspace demo-animation dev",
    "build": "npm run --workspace @vizij/animation-react build && npm run --workspace demo-animation build",
    "reset": "node -e \"const {execSync}=require('node:child_process');execSync('rm -rf node_modules package-lock.json');execSync('find apps -maxdepth 2 -name node_modules -type d -prune -exec rm -rf {} +');execSync('find packages -maxdepth 3 -name node_modules -type d -prune -exec rm -rf {} +');execSync('find . -type d -name .vite -prune -exec rm -rf {} +');\" && npm ci"
  }
}
```

---

## Day-to-day local development

### 0) Prereqs

* Rust & Cargo
* `wasm-pack` (`cargo install wasm-pack`)
* Node LTS
* (optional) `cargo-watch`: `cargo install cargo-watch`

### 1) Build & link the WASM package (in `vizij-rs`)

```bash
cd vizij-rs
npm run build:wasm:animation && npm run build:wasm:graph     
# or: node scripts/build-wasm.mjs

npm run link:wasm:animation  && npm run link:wasm:graph       
# compiles wrapper TS and runs `npm link`
# (or run `npm run watch:wasm` in another terminal to rebuild on Rust edits)
```

### 2) Link it into the web workspace (in `vizij-web`)

```bash
cd ../vizij-web
npm run link:wasm       
npm i                   # root install (workspaces)
npm run build:animation && npm run build:graph
npm run dev:animation 
npm run dev:graph 
# runs demo app dev server
# open http://localhost:5173
```

### 3) Edit flow

* **Rust changes**: `vizij-rs` terminal runs `watch:wasm` (recommended). Vite reloads when `pkg/*.wasm` or glue JS changes.
* **React/TS changes**: rebuild the React wrapper (or just rely on Vite HMR if the app imports it source-mapped); the dev server will refresh.

### 4) When things get weird (clean reset)

```bash
# stop Vite
cd vizij-web
npm run reset
# relink if needed
npm run link:wasm
npm run dev
```

### 5) Unlink when you’re done

```bash
cd vizij-web && npm unlink @vizij/animation-wasm && npm i
cd ../vizij-rs/npm/@vizij/animation-wasm && npm unlink
```

---

## Production & CI

* **Publish** `@vizij/animation-wasm` from `vizij-rs` CI (includes `pkg/`).
* **Use semver** in `vizij-web` (`"@vizij/animation-wasm": "^x.y.z"`). No `npm link` in CI; just `npm ci && npm run build`.
* The Vite Option B dev config is harmless in prod builds; you can leave it.

---

## Troubleshooting tips

* **“No matching export 'default'”** → ensure wrapper exports `default` **and** named symbols.
* **Vite ‘outside fs allow list’** → you picked Option B; keep `preserveSymlinks: true` and un-ignore the linked package in `server.watch.ignored`.
* **TypeScript can’t find `../pkg/...`** → regenerate `pkg/` (`npm run build:wasm`), then rebuild wrapper TS.
* **Demo uses the wrong dependency** → run `npm ls @vizij/animation-wasm` in `vizij-web`. If it’s not the link, re-run `npm run link:wasm`.

---

If you want, I can also generate these scripts + the Vite config as a small patch you can apply directly to your repos.
