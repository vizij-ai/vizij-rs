# Plan — one `Value`: Vizij on arora-types

Vizij runs on Arora; the store, the modules, the behaviors, and Studio all
speak `arora_types::value::Value`. Keeping a second general value enum
(`vizij_api_core::Value`) means converting at every seam on every tick and
maintaining two Shape/serde stories. This plan removes it.

**Decided constraints** (Victor, 2026-07-09): breaking the serialized forms
between layers is fine; anything persisted (notably `.glb` face bundles) gets a
migration tool; bindeps are acceptable but come **last**, to promote the wasm
animation player after the native path has carried development.

## Target model

- `vizij_api_core::Value` (the enum) is deleted. `vizij-api-core` depends on
  `arora-types` and re-exports its `Value`, becoming the **vizij vocabulary
  crate** over it:
  - the vizij composite **type records + ids** (vec2/vec3/vec4/quat/color-rgba/
    transform — today in `vizij-arora`), declared once, shared by module
    codegen and Studio introspection;
  - **constructors** (`vec3([f32;3]) -> Value`, `transform(..)`, …) building
    the corresponding `Value::Structure`;
  - **accessors** (`as_vec3(&Value) -> Option<[f32;3]>`, …) reading them back;
  - Shape metadata as the established **sidecar** (`meta/<path>` keys), moved
    from `vizij-arora`;
  - `WriteBatch`, coercion, and blend helpers reworked over arora `Value`.
- **Kernels never hold the dynamic enum.** Animation sampling/accumulation and
  graph evaluation do their math on plain Rust types (`f32`, `[f32;3]`,
  `[f32;4]`, small structs); `Value` exists at the seams (store, module ABI,
  wasm bridge) and is decoded once per tick, not per operation. This is the
  rule that keeps the migration from regressing hot paths.

### Type mapping (canonical; today's `vizij-arora` table, promoted)

| vizij (removed)        | arora                                            |
| ---------------------- | ------------------------------------------------ |
| `Float`                | `F32`                                            |
| `Bool`                 | `Boolean`                                        |
| `Text`                 | `String`                                         |
| `Vector`               | `ArrayF32`                                       |
| `Vec2/3/4`, `Quat`, `ColorRgba`, `Transform` | `Structure` with the vizij type ids |
| `Record`               | `KeyValue`                                       |
| `Array`/`List`/`Tuple` | `ArrayValue` (one sequence kind; the distinction carried no semantics) |
| `Enum(tag, value)`     | `Enumeration` (arora's native one — replaces the tagged-structure encoding) |

## Stages

1. **Boundaries** — the interop crates (`vizij-arora-store`, `vizij-arora-hal`,
   `vizij-arora-behavior`) and the orchestrator blackboard stop materializing
   vizij values where the vizij side doesn't need them; `OrchestratorBehavior`
   drops its JSON round-trip (`set_input` takes values, not JSON).
2. **The core** — `vizij-api-core` becomes the vocabulary crate (keystone);
   then `vizij-animation-core`, `vizij-graph-core`, `vizij-orchestrator-core`,
   and the bevy bridges migrate against it, kernels moving to PODs. This is
   the bulk (~1200 references across the workspace) and lands as one reviewed
   branch, crate by crate.
3. **Wire and assets** — the wasm bridges (`vizij-api-wasm`,
   `vizij-graph-wasm`, `vizij-orchestrator-wasm`, …) serialize arora `Value`
   forms; `@vizij/value-json` (and consumers in vizij-web) update to them; the
   **GLB migration tool** rewrites Value-bearing documents embedded in face
   bundles (scope from the GLB audit; expected: JSON in glTF `extras` —
   stored animations, node-graph specs).
4. **Module promotion** — `vizij-animation-module` ships as the wasm artifact
   (bindep) with the native cdylib as the development path; the arora engine
   picks the executor, no vizij-side branching.

## Non-goals here

- The orchestrator-as-`ModuleCall`-node rework (rides the arora graph-model /
  predetermined-I/O PRs, separately).
- The `arora-buffers` array-of-structures wire question — still gated on its
  own discussion.
