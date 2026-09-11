# vizij-render-core

What draws a Vizij face, separated from what runs it.

This crate reads a GLB's Vizij metadata, joins it to the spawned Bevy scene,
fits the camera, and applies values onto transforms, materials and morph
weights. It knows nothing about devices, runtimes, bridges or operators —
`crates/vizij` is one host, `vizij-render-web` is another.

## The seam

Everything crossing the boundary is a closure the host installs, so the crate
has no dependency on Arora, on a browser, or on a UI toolkit.

| Resource | Direction | What it carries |
| -- | -- | -- |
| `PoseFeed` | in | `Vec<(TypedPath, Value)>`, read once per frame |
| `ViewOffsetFeed` | in | an optional `SubCameraView` — three's `setViewOffset` |
| `PickSink` | out | the element a pointer clicked, by glTF node name |
| `AnchorSink` | out | every element's screen-space box, once per frame |

A host over an Arora device installs `PoseFeed::new(move || rig.pose())`; a
replay host closes over its own samples; a test returns a fixed vector. Values
are `vizij_api_core::Value`, which *is* `arora_types::value::Value`, so a host
holding an Arora store converts nothing.

Anchors are published from a `Last` system, after transform propagation, so
they are the positions that frame drew rather than the previous frame's.

## What a host still owns

- **Which values exist, and when.** The crate samples the feed; it never asks
  for one.
- **Everything about the window.** Bevy sizes its surface from the canvas's
  parent when it starts, so a parent that measures zero yields a running app
  that draws nothing, with no error.
- **Assets.** `Face::glb_path` is resolved by Bevy's asset server against
  whatever root the host configured.

## Features

`cli` derives `clap::ValueEnum` on `Fit`, for a host exposing it as a flag.

## Tests

`cargo test -p vizij-render-core` covers the camera fit (contain, cover,
stretch, per-axis zoom, and agreement with Bevy's own scaling modes) and the
rule that a pointer hit resolves to the nearest ancestor carrying an element.

The render itself is covered one level up, by `crates/vizij`'s
`snapshot_regression` test, which renders both reference faces and compares
them to committed PNGs. Run it after any change here:

```bash
VIZIJ_FIXTURES=$PWD/fixtures cargo test -p vizij --test snapshot_regression -- --ignored
```
