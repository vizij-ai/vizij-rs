---
"@vizij/runtime": minor
---

`onInitProgress(listener)` follows `init()`: the wasm module's download in bytes (against its `Content-Length` when the response is not compressed), its compile, its start and `ready`. The listener is called with the current stage at once, then with each change, the download at most every 50 ms. The bytes are counted on a clone of the fetched response, and the response itself is what compiles as it streams in, so Chrome caches the compiled code for the next load. The README says how a host serves the module for that: a content-hashed URL, an immutable `Cache-Control`, `application/wasm`, and brotli.
