# The Vizij face standard

A compliant Vizij face is driven through **named controls on the store**, under
the `standard/vizij/` prefix. A caller — a behavior, a bridge, a performance —
writes a control; the face's own graphs turn that control into the motion of its
particular rig. The vocabulary is the contract between the two, so the same
command drives any face and any face can be swapped under the same command.

The controls live in [`vizij-arora-host`'s `standard`
module](../crates/interop/vizij-arora-host/src/standard.rs), which is the
authoritative source; this page mirrors it.

Everything is an `f32` weight in `[0, 1]` unless stated otherwise.

## Three tiers

The vocabulary runs coarse to fine. A face implements what it implements, and a
standard profile (like [ROS4HRI](ros4hri.md)) degrades to the tiers a face
covers.

1. **Gaze & lids** — where the eyes point and how open they are.
2. **Semantic** — one weight per named expression and per viseme shape.
3. **Muscle** — fine-grained controls, one per FACS action unit / ARKit
   blendshape.

## Gaze & lids

| Control path | Range | Meaning |
|---|---|---|
| `standard/vizij/left_eye/pos/x` | `[-1, 1]` | eye left→right (subject's own left is negative) |
| `standard/vizij/left_eye/pos/y` | `[-1, 1]` | eye down→up |
| `standard/vizij/right_eye/pos/x` | `[-1, 1]` | as above, right eye |
| `standard/vizij/right_eye/pos/y` | `[-1, 1]` | as above, right eye |
| `standard/vizij/left_eye_top_eyelid/pos/y` | `[0, 1]` | 0 open, 1 closed |
| `standard/vizij/right_eye_top_eyelid/pos/y` | `[0, 1]` | 0 open, 1 closed |

Per-eye positions (rather than a single gaze vector) let a profile command
vergence; the ROS4HRI profile computes them from a face-frame target.

## Semantic tier

### Expressions

One weight per named expression, at `standard/vizij/expression/<name>`. The
names are ROS4HRI's `hri_msgs/Expression` vocabulary (25). The standard does
**not** prescribe what an expression looks like — that is the face's authored
pose; it prescribes only the name a caller commands.

```
neutral   angry      sad         happy        surprised
disgusted scared     pleading    vulnerable   despaired
guilty    disappointed embarrassed horrified  skeptical
annoyed   furious    suspicious  rejected     bored
tired     asleep     confused    amazed       excited
```

### Visemes

One weight per viseme shape, at `standard/vizij/viseme/<shape>`. The shapes are
the industry 15-shape set (Oculus/Meta naming); `sil` is silence, the
closed-mouth rest shape.

```
sil PP FF TH DD kk CH SS nn RR aa E ih oh ou
```

## Muscle tier

Fine-grained controls at `standard/vizij/face/<control>`, cherry-picked from two
standards so each is reachable from either:

- **FACS supplies the taxonomy.** Each control names a facial action unit, so
  ROS4HRI's `hri_msgs/FacialActionUnits` maps onto the muscle tier losslessly.
  AU codes repeat across lateralized pairs — FACS does not split left/right at
  the code level.
- **ARKit supplies lateralization and naming.** Its blendshape arrays carry the
  left/right the FACS message cannot, and assets in the wild ship ARKit-named
  morph targets, so an ARKit name resolves directly to a control.

Controls exist for the union that makes sense on a *commanded* robot face.
ARKit's tracking-only shapes (`eyeLook*` — redundant with the gaze tier) and
FACS codes a command channel cannot express (visibility, head and eye
movement — owned by the gaze tier) have none.

| Control | AU | ARKit | Control | AU | ARKit |
|---|---|---|---|---|---|
| `brow_inner_up` | 1 | browInnerUp | `mouth_smile_left` | 12 | mouthSmileLeft |
| `brow_outer_up_left` | 2 | browOuterUpLeft | `mouth_smile_right` | 12 | mouthSmileRight |
| `brow_outer_up_right` | 2 | browOuterUpRight | `mouth_frown_left` | 15 | mouthFrownLeft |
| `brow_down_left` | 4 | browDownLeft | `mouth_frown_right` | 15 | mouthFrownRight |
| `brow_down_right` | 4 | browDownRight | `mouth_press_left` | 24 | mouthPressLeft |
| `eye_wide_left` | 5 | eyeWideLeft | `mouth_press_right` | 24 | mouthPressRight |
| `eye_wide_right` | 5 | eyeWideRight | `mouth_pucker` | 18 | mouthPucker |
| `eye_squint_left` | 7 | eyeSquintLeft | `mouth_funnel` | 22 | mouthFunnel |
| `eye_squint_right` | 7 | eyeSquintRight | `mouth_stretch_left` | 20 | mouthStretchLeft |
| `eye_closed_left` | 43 | eyeBlinkLeft | `mouth_stretch_right` | 20 | mouthStretchRight |
| `eye_closed_right` | 43 | eyeBlinkRight | `mouth_upper_up_left` | 10 | mouthUpperUpLeft |
| `cheek_raise_left` | 6 | cheekSquintLeft | `mouth_upper_up_right` | 10 | mouthUpperUpRight |
| `cheek_raise_right` | 6 | cheekSquintRight | `mouth_lower_down_left` | 16 | mouthLowerDownLeft |
| `cheek_puff` | 34 | cheekPuff | `mouth_lower_down_right` | 16 | mouthLowerDownRight |
| `nose_sneer_left` | 9 | noseSneerLeft | `jaw_open` | 26 | jawOpen |
| `nose_sneer_right` | 9 | noseSneerRight | `jaw_left` | — | jawLeft |
| `tongue_out` | 19 | tongueOut | `jaw_right` | — | jawRight |
| | | | `jaw_forward` | 29 | jawForward |

AU 45 (blink) aliases AU 43 (eyes closed) — both command the eyelid controls.

## Reaching the vocabulary in code

`vizij-arora-host`'s `standard` module exposes the constants and the path
helpers (`expression_path`, `viseme_path`, `face_path`), plus
`controls_for_au(code)` (the lateralized pair or single control an AU drives)
and `control_for_arkit(name)`. Callers should build paths through these rather
than hand-format strings.

## See also

- [The ROS4HRI profile](ros4hri.md) — the built-in mapping that fills this
  vocabulary from ROS4HRI's topics.
- [`vizij-bundle`](../crates/tools/vizij-bundle/README.md) — the tool that
  reports which tiers a face covers (`validate`) and embeds a standard profile
  into a face GLB (`add-standard`).
