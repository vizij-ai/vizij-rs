---
"@vizij/runtime": patch
---

`invoke`, `setValue`, `writeValues` and `moduleAnimation` cost linearly in the size of the values they carry. A large record, such as a long animation converted by `moduleAnimation` and passed to `load_clip` or `reload_clip`, used to cost superlinearly the first time: it went through an intermediate JSON tree holding about sixty times its text in the wasm heap. It is now read straight into an Arora value, and written straight from one.
