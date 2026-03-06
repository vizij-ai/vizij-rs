# Vizij Animation Stack

> Core engine, Bevy integration, and WebAssembly bindings for Vizij animation playback.

The `crates/animation` directory contains the Rust-side animation stack. The three crates here share the same animation data model and are usually changed together when playback, bindings, or wasm surfaces move.

## Crate Map

| Crate | Purpose | Primary consumers |
|-------|---------|-------------------|
| [`vizij-animation-core`](vizij-animation-core/README.md) | Deterministic animation engine, parsing, playback, blending, baking. | Native hosts, orchestrator runtime, wasm binding, Bevy plugin. |
| [`bevy_vizij_animation`](bevy_vizij_animation/README.md) | Bevy plugin that maps engine outputs onto ECS entities/components. | Bevy apps and tools. |
| [`vizij-animation-wasm`](vizij-animation-wasm/README.md) | `wasm-bindgen` bridge used by the npm wrapper. | [`@vizij/animation-wasm`](../../npm/@vizij/animation-wasm/README.md). |

## Typical Workflows

### Rust host

1. Add `vizij-animation-core`.
2. Parse `StoredAnimation` JSON or construct `AnimationData`.
3. Create an `Engine`, register animations, create players/instances, and call `update_values(dt, inputs)` every frame.
4. Apply `Outputs.changes` to your host and consume `Outputs.events` as needed.

### Bevy app

1. Add `bevy_vizij_animation`.
2. Insert `VizijAnimationPlugin`.
3. Mark a hierarchy root with `VizijTargetRoot` and optional overrides with `VizijBindingHint`.
4. Load animations through the shared `VizijEngine` resource and let the plugin drive fixed updates.

### JavaScript / TypeScript

1. Build the wasm package with `pnpm run build:wasm:animation`.
2. Use the npm wrapper from [`npm/@vizij/animation-wasm`](../../npm/@vizij/animation-wasm/README.md).
3. Call `await init()` once, then work through the wrapper `Engine` class.

## Minimal Smoke Test

```rust
use vizij_animation_core::{Engine, Inputs, InstanceCfg};
use vizij_animation_core::stored_animation::parse_stored_animation_json;
use vizij_test_fixtures::animations;

fn main() -> anyhow::Result<()> {
    let json = animations::json("pose-quat-transform")?;
    let stored = parse_stored_animation_json(&json)?;

    let mut engine = Engine::default();
    let anim = engine.load_animation(stored);
    let player = engine.create_player("demo");
    engine.add_instance(player, anim, InstanceCfg::default());

    let outputs = engine.update_values(1.0 / 60.0, Inputs::default());
    for change in outputs.changes {
        println!("{} => {:?}", change.key, change.value);
    }
    Ok(())
}
```

## Build And Test

Run from the repository root.

```bash
cargo test -p vizij-animation-core
cargo test -p bevy_vizij_animation
pnpm run build:wasm:animation
pnpm --filter @vizij/animation-wasm test
```

`pnpm run build:wasm:animation` writes the wasm-bindgen output directly to `npm/@vizij/animation-wasm/pkg/`, which the npm wrapper copies into its published `dist/` layout during `pnpm --filter @vizij/animation-wasm build`.

## Release Notes

Animation releases follow the workspace Changesets flow rather than manual `cargo publish` / `npm publish` steps from this directory:

1. Add a Changeset with `pnpm changeset` for any publishable package change.
2. Run `pnpm release` to rebuild wasm and shared packages before tagging.
3. Let CI handle `pnpm ci:version` and `pnpm ci:publish`.

If you change animation ABI or wrapper-visible behavior, rebuild the wasm package and confirm the wrapper still agrees on `abi_version()`.

## Reference Links

- [vizij-animation-core README](vizij-animation-core/README.md)
- [bevy_vizij_animation README](bevy_vizij_animation/README.md)
- [vizij-animation-wasm README](vizij-animation-wasm/README.md)
- [@vizij/animation-wasm README](../../npm/@vizij/animation-wasm/README.md)
