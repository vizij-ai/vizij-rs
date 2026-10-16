---
"@vizij/runtime": patch
---

The wasm module is linked with fat LTO and leaves out Bevy's log plugin and multi-threaded executor, which a page never runs: 24.3 MB (6.9 MB gzip, 4.3 MB brotli), from 26.6 MB (7.5 MB, 4.6 MB).
