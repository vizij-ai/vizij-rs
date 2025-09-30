# @vizij/blackboard-wasm

WebAssembly (wasm-bindgen) bindings for the Vizij blackboard core crate.

## Building

From repository root run:

```
npm run build:wasm:blackboard
npm --workspace npm/@vizij/blackboard-wasm run build
```

This runs `wasm-pack build` targeting `web` to produce JS glue + `.wasm` in `pkg/`, then compiles TypeScript in `src/` to `dist/`.

## Linking locally

```
npm run link:wasm:blackboard
```

Then in a consuming project:

```
npm link @vizij/blackboard-wasm
```

## Publishing

Ensure `pkg/` exists (built) and run (from repo root):

```
npm publish --workspace @vizij/blackboard-wasm
```

## API

Currently this package only exposes a low-level `init()` plus all raw wasm-bindgen exports through `bindings`. A higher-level ergonomic wrapper can be added following patterns in `@vizij/animation-wasm` or `@vizij/node-graph-wasm` once requirements are defined.
