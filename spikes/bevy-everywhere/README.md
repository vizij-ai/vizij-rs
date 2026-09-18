# Spikes behind the plan "One Bevy view for every Vizij face"

Stand-alone experiments written during the study recorded in vizij-docs
`active_projects/runtime/bevy_everywhere/` (branch `docs/bevy-everywhere-study`,
PR vizij-ai/vizij-docs#16). Each directory is a self-contained cargo project or
script set with its own logs and screenshots; every study page under
`studies/` there cites the directory it ran in. Build outputs (target dirs,
`pkg/`, `.wasm`, `.apk`, `.so`), `node_modules`, toolchains and the face GLBs
(copies of vizij-web's fixtures) are not committed; the recipes rebuild them.
Emulator runs keep their screenshots and `*-summary.txt`, not the raw logcat
dumps.

| Directory | What it establishes |
| --- | --- |
| `bevy-wasm` | Bevy 0.19.1 renders a face GLB with morph targets in a browser canvas over WebGL2; the seven-feature list; sizes, wasm-opt, LTO; two module instances; the overlay-viewport pattern; `cargo check` of `vizij` for wasm32 |
| `challenge-wasm` | device + view in one wasm module, JS-paced; two faces on one canvas with per-face `RenderLayers`; 25 load/unload cycles; loading from bytes; mesh picking; App restart after `AppExit` (page scripts `q1.js`..`q5.js`, `serve.sh`) |
| `bevy-android` | a Bevy 0.19.1 APK on the cargo-ndk + Gradle + NativeActivity recipe; `cargo ndk check` of `vizij` |
| `challenge-android` | rendering per emulator image (API 32/34/35), GPU mode and backend; `run-image.sh`, `run-case.sh`, `vkprobe` |
| `arora-wasm` | which arora and vizij interop crates compile for wasm32 |
| `tts-wasm` | the wasm32 branch of `vizij-arora-tts` under one skeleton, with native and wasm-bindgen tests and `www/play.js` |
| `ws-operator` | the in-app local WS bridge with a populated registry, verified over the wire; the Studio `Operator` seam |
| `headless-export` | GLB export from the JS world model without a renderer (Node) |
| `standalone-anatomy`, `native-app-anatomy`, `lto-crate-type-probe` | dependency trees and a cargo LTO probe |

Warm target directories are worth keeping: a cold Bevy wasm build is about
27 CPU-minutes and a cold Android build about 20 minutes; source changes
rebuild in 14–37 s. The `wasm-bindgen` CLI must match the locked crate
version (0.2.128); the native `wasm-opt` binary is needed (the npm `binaryen`
package's JavaScript build is unusably slow): take it from binaryen's
`version_123` release into `bevy-wasm/tools/`.
