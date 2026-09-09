# Profiles and mappings

Vizij drives faces through **named store paths**. Two kinds of artifact
organize that vocabulary, and they are different in nature:

- A **profile** is an *interface*: the set of store paths one party exposes
  to another, each with its type, range, and default. It is a declaration —
  an API — and says nothing about how a path is produced or consumed.
- A **mapping** is an *operation*: a node graph that implements one profile
  in terms of another. It reads the paths of the profile it consumes and
  writes the paths of the profile it produces.

A face runs a chain of mappings between profiles. The ROS4HRI mapping
consumes the `ros4hri` profile and produces the `vizij-face` profile; the
face's own adaptation graph consumes `vizij-face` and produces the face's
private pose profile; the rig turns that into morphs and bones.

```
ros4hri profile ──▶ [ ros4hri mapping ] ──▶ vizij-face profile ──▶ [ adaptation ] ──▶ the face's pose ──▶ [ rig ]
(device-scoped)                            (face-scoped)                              (face-scoped)
```

Keeping the two apart is what makes the chain checkable: a mapping's inputs
can be validated against the profile it claims to consume and its outputs
against the profile it claims to produce, instead of the mapping being the
only surviving record of either.

## Profiles

A profile is data — a JSON asset under
[`crates/interop/vizij-arora-host/profiles/`](../crates/interop/vizij-arora-host/profiles/):

```json
{
  "id": "ros4hri",
  "version": "v1",
  "title": "ROS4HRI face command",
  "description": "…",
  "scope": "device",
  "keys": [
    { "path": "standard/ros4hri/expression/valence", "kind": "input",
      "value_type": "f32", "min": -1.0, "max": 1.0, "default_value": { "f32": 0.0 } },
    …
  ]
}
```

Each key is the shape of `arora-bridge-ws`'s `KeyInfo` — `path`, `kind`,
`value_type` (an arora type name: `f32`, `str`, `struct`, …), `min`, `max`,
`default_value` (an arora value) — so a profile round-trips through the same
descriptor the WS registry and the standalone app already speak, plus an
optional `meta` carrying standard metadata (the FACS action unit, the ARKit
blendshape, the tier).

`scope` says where the paths live:

| scope | paths are | addressed by |
|---|---|---|
| `device` | absolute — one instance per device, shared by every face (`standard/ros4hri/*`, as a bridge writes them) | as is |
| `face` | relative to one face — every face carries its own copy (`standard/vizij/*`) | prepending the face's rig prefix, `rig/<faceId>/` |

Vizij ships two:

| profile | scope | keys | declared by |
|---|---|---|---|
| `vizij-face` | face | 82 — gaze & lids, 25 expressions, 15 visemes, 36 muscle-tier controls | the [face standard](face-standard.md) |
| `ros4hri` | device | 40 — expression name/valence/arousal, gaze target and frame, 20 action units, 15 visemes | the [ROS4HRI key contract](ros4hri.md#the-standardros4hri-key-contract) |

Both are generated from the Rust constants
([`profile.rs`](../crates/interop/vizij-arora-host/src/profile.rs)) and held
to them by a drift test, the same contract the mapping assets have; that
direction inverts later, so the asset becomes the definition.

A face **declares** the profiles it is authored against in its bundle's
top-level `profiles` array — beside `graphs`, because a profile is not a
graph. That is what tells a reader which interface to hold the face to.

## Mappings

A mapping is a graph asset under
[`crates/interop/vizij-arora-host/mappings/`](../crates/interop/vizij-arora-host/mappings/),
generated from a builder and drift-tested like a profile. Its `input` nodes
read the profile it consumes; its `output` nodes write the profile it
produces. Vizij ships one, [ROS4HRI](ros4hri.md).

The assets are written for readers as much as for the runtime: node ids are
hierarchical and say what each node holds — `in/<key>` and `out/<control>`
for the two profiles, `<channel>/<item>/<step>` for the computation between
them, `const/<value>` for shared constants — so a mapping can be reviewed
channel by channel and diffed meaningfully (see [the ROS4HRI
mapping](ros4hri.md#embedding-and-editing-the-mapping)).

A mapping composes into a face's behavior between the face's own graphs and
any playing program, last writer wins. A face may **embed** a mapping
(`vizij-bundle add-standard`) as its pinned copy, which then replaces the
built-in of the same id.

The GLB entry kind for an embedded mapping is still spelled
`standard-profile`: faces in the wild carry that string and the authoring app
writes it, so the wire format keeps it until a read-both migration lands.
Only the names in code say what the entry is.

## Where each surfaces

| | profiles | mappings |
|---|---|---|
| Rust | `vizij_arora_host::profile` — `Profile`, `profile(id)`, `profiles_json()`, `surface(…)` | `vizij_arora_host::mappings` — `StandardMapping`, `standard_mapping_source(id, rigPrefix)` |
| CLI ([`vizij-bundle`](../crates/tools/vizij-bundle/README.md)) | `profiles`, `export-profile`, `add-profile`, `surface` | `mappings`, `export-mapping`, `add-standard` |
| npm ([`@vizij/runtime`](../npm/@vizij/runtime/README.md)) | `profiles()`, `profile(id, rigPrefix)` | `mappings()`, `mapping(id, rigPrefix)` |
| GLB bundle | `profiles: [ … ]` (declarations) | `graphs: [{ kind: "standard-profile", id: "standard::<id>", … }]` |

## Reconciling a mapping against a profile

`vizij-bundle surface <graph.json> --side <input|output>` lifts a profile
out of any mapping graph: the paths it actually reads or writes. Compared to
the declared profiles, the shipped ROS4HRI mapping shows:

| profile | declares | mapping touches | |
|---|---|---|---|
| `ros4hri` | 40 | 39 | `gaze/frame` is consumed by the `look_at` skill, not the mapping |
| `vizij-face` | 82 | 80 | `jaw_left` / `jaw_right` have no FACS code, so the action-unit channel cannot reach them |

Every path the mapping writes is declared.

A lifted surface is a bootstrap, never the source of truth: a mapping only
touches the part of a profile it needs, so the surface can be a strict subset
of the profile it claims. Compare, do not replace.

## The ROS side

The ROS 2 bridge's *exposure profile* (`ExposureProfile::ros4hri()` in
[`arora-bridge-ros2`](https://github.com/semio-ai/arora-sdk/tree/main/crates/arora-bridge-ros2))
is, in these terms, a mapping too — from ROS topics onto the `ros4hri`
profile's keys — expressed as bridge configuration rather than a graph. The
`ros4hri` profile is the seam between the two: what the bridge writes and
what the mapping reads.

## See also

- [The Vizij face standard](face-standard.md) — the `vizij-face` profile,
  tier by tier.
- [ROS4HRI support](ros4hri.md) — the `ros4hri` profile and its mapping.
- [`vizij-bundle`](../crates/tools/vizij-bundle/README.md) — the CLI.
