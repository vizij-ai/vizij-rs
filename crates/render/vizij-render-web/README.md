# vizij-render-web

`vizij-render` mounted on a page: the same renderer the native app runs,
in a browser canvas, over WebGPU.

Demonstrative. Nothing in this repository depends on it, and the web apps keep
`@vizij/render` — `docs/proposal-vizij-native-app.md` puts a wasm build of the
Bevy view out of scope for v1. This crate exists so that decision can be taken
against measurements rather than estimates.

## Building and running

```bash
node scripts/build-render-web.mjs      # cargo build + wasm-bindgen, stages the harness
node scripts/serve-render-harness.mjs  # http://localhost:8391/
```

The build refuses to run unless the `wasm-bindgen` CLI matches the version the
lockfile resolves — the bindgen schema is unstable between releases, and a
mismatch produces a confusing runtime failure rather than a build error. It
also stages the two face GLBs from `fixtures/`, which the `snapshot-regression`
CI job fetches; without them the harness 404s.

The harness offers an element picker, a per-feature slider bound to the
selected animatable, a DOM label anchored to the picked element, and a panel
that drives the view offset while it slides.

## The JS surface

```js
start(canvas, assetRoot, glbPath, glbBytes)  // mount and run; once per page
set_float(id, value)                         // write one animatable
set_vec3(id, x, y, z)
animatables()                                // [{id, node, feature, morph_target}]
set_pick_callback(fn)                        // fn(elementName)
set_anchor_callback(fn)                      // fn([{element, x, y, min_x, min_y, max_x, max_y}])
set_view_offset(x, y, w, h, fullW, fullH)    // three's setViewOffset
clear_view_offset()
```

The GLB arrives twice by design: Bevy's asset server loads it by path for
meshes, materials and morph targets, and the same bytes come in directly so the
`RobotData` extension can be read, which Bevy's loader does not surface. The
browser serves the second read from cache.

## Constraints worth knowing before embedding it

- **One view per page.** The feed, the animatable list and the view offset are
  process-wide statics, because the Bevy app owns the browser's animation loop
  and a second view would have nothing to run on. `start` returns an error if
  called twice.
- **Bevy's loop cannot be restarted.** In a single-page app, mount once and
  keep it; a component that starts the renderer on mount will start a second
  app on the next mount unless it guards at module scope.
- **Assets are fetched with `AssetMetaCheck::Never`.** Bevy asks for a `.meta`
  sidecar per asset and falls back on a 404, but a single-page app answers any
  unknown path with its `index.html` at 200, so the fallback never fires and
  the asset fails on HTML that will not parse as RON.
- **`LogPlugin` is disabled**, because a page has already installed the `log`
  global logger. Bevy's own events reach it through the `tracing` dependency's
  `log` feature.
- **WebGPU only.** Bevy resolves the backend at build time, so there is no
  WebGL2 fallback in this build.
- **`animatables()` is typed `any`** in the generated `.d.ts`; it is serialized
  with `serde-wasm-bindgen` rather than declared.

## Gaps

There is no automated test. The sibling `vizij-arora-web` ships a
`wasm_bindgen_test` proof running in a headless browser, and this crate should
match that before it is treated as anything more than a demonstration.
