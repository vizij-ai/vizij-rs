# challenge-wasm spike

One wasm module = N arora devices (store + RigHal + graph behavior, JS-paced) +
one Bevy 0.19.1 App for the page lifetime, one transparent canvas, one viewport
camera and one render layer per face. Built for wasm32-unknown-unknown in the
bevy-wasm spike's target dir. Study:
vizij-docs `active_projects/runtime/bevy_everywhere/studies/challenge-wasm-one-module.md`.

- `Cargo.toml`, `src/lib.rs` — the crate (`picking` feature adds MeshPickingPlugin
  and the click observer).
- `build.sh [picking]` — cargo build (release) + wasm-bindgen into `pkg/` or
  `pkg-picking/`; puts `~/.cargo/bin` on PATH itself.
- `serve.sh` — `python3 -m http.server 8766`, requests logged to `logs/http.log`.
- `index.html` — mounts the App on `#view`, fetches GLBs once as bytes, owns the
  devices and steps them from one requestAnimationFrame loop; `?pkg=pkg-picking`
  selects the picking build; `?nomount` skips the mount.
- `common.js`, `q1.js` (one face, writes), `q2.js` (two faces, N unload/load
  cycles, standard-key probe), `q4.js` (clicks), `q5.js` (AppExit, second mount,
  writes before and after) — Playwright drivers, NODE_PATH=semio_studio/node_modules.
- `diffbox.py`, `pixdiff.py` — screenshot diffs; `glbinfo.py` — RobotData nodes and
  rig inputs of a GLB.
- `logs/` — build logs, wasm-opt log, HTTP log, driver results (`q*.json`).
- `shots/` — screenshots and console logs (`q2*` without render layers, `q2b*`,
  `q4*`, `q5b*` with them).
- `old/` — earlier variants of the sources and drivers.
