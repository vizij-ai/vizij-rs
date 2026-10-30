# The Vizij face standard

A compliant Vizij face is driven through **named controls on the store**, under
the `standard/vizij/` prefix. A caller — a behavior, a bridge, a performance —
writes a control; the face's own graphs turn that control into the motion of its
particular rig. The vocabulary is the contract between the two, so the same
command drives any face and any face can be swapped under the same command.

The controls live in [`vizij-arora-host`'s `standard`
module](../crates/interop/vizij-arora-host/src/standard.rs), which is the
authoritative source; this page mirrors it. As data, the same vocabulary is
the **`vizij-face` profile**
([`profiles/vizij-face.json`](../crates/interop/vizij-arora-host/profiles/vizij-face.json)):
an interface of 98 typed paths, face-scoped — every face carries its own copy
under its rig prefix — with each key's tier, FACS action unit, and ARKit
blendshape as metadata. See [profiles and mappings](profiles-and-mappings.md).

Everything is an `f32` weight in `[0, 1]` unless stated otherwise.

## Three tiers and the conversation state

The pose vocabulary runs coarse to fine. A face implements what it implements,
and a standard mapping (like [ROS4HRI](ros4hri.md)) degrades to the tiers a
face covers.

1. **Gaze & lids** — where the eyes point, how open they are, the blink, and
   the gaze target with its weight.
2. **Semantic** — one weight per named expression and per viseme shape.
3. **Muscle** — fine-grained controls, one per FACS action unit / ARKit
   blendshape.

Beside them, the **conversation state** tells the face what the dialogue it
takes part in is doing — state it may react to, not a pose.

In the profile, each key's `tier` is one of `gaze`, `expression`, `viseme`,
`muscle` and `conversation`. `vizij-bundle validate` grades a face's
compliance L0–L3 on the first four, in that order; it reports the
conversation state, and the gaze target and its weight within the gaze
tier, without grading them.

## Gaze & lids

| Control path | Range | Meaning |
|---|---|---|
| `standard/vizij/left_eye/pos/x` | `[-1, 1]` | eye left→right (subject's own left is negative) |
| `standard/vizij/left_eye/pos/y` | `[-1, 1]` | eye down→up |
| `standard/vizij/right_eye/pos/x` | `[-1, 1]` | as above, right eye |
| `standard/vizij/right_eye/pos/y` | `[-1, 1]` | as above, right eye |
| `standard/vizij/left_eye_top_eyelid/pos/y` | `[0, 1]` | 0 open, 1 closed |
| `standard/vizij/right_eye_top_eyelid/pos/y` | `[0, 1]` | 0 open, 1 closed |
| `standard/vizij/blink` | `[0, 1]` | both eyes: 0 open, 1 closed |
| `standard/vizij/gaze/target/x` | `[0, 1]` | where the eyes look, across the face's frame: 0 its left edge, 1 its right edge; rests at 0.5 |
| `standard/vizij/gaze/target/y` | `[0, 1]` | where the eyes look, down the face's frame: 0 its top edge, 1 its bottom edge; rests at 0.5 |
| `standard/vizij/gaze/target/weight` | `[0, 1]` | how strongly the eyes hold the target: 0 not at all, 1 fully; rests at 0 |

Per-eye positions (rather than a single gaze vector) let a mapping command
vergence; the ROS4HRI mapping computes them from a face-frame target, with
the face's [eye spacing and gaze range](#settings).

The **blink** is a level, not a trigger. Whoever blinks the face shapes each
blink over time — the close and the reopen, about 0.2 s in all — and the face
renders the weight as is, with no transition of its own: the same split as the
visemes, which keeps a face's adaptation stateless. The blink is independent of
the eyelid positions, which keep their meaning: a face closes each lid at least
as far as the stronger of the two, so a blink passes over lids held
half-closed and returns them there. The ROS4HRI mapping does not write it; its
idle blink drives the eyelid positions.

The **gaze target** is a point on the face's frame, in screen conventions: the
origin at the frame's top-left corner, `x` to the right and `y` downward, each
a fraction of the frame as the viewer sees the face. It rests at the centre,
(0.5, 0.5): the face looks straight ahead. A caller directs the gaze with it
without knowing how the face moves its eyes; toward the edges of the frame
the eyes reach the ends of their range.

The target's **weight** says how strongly the eyes hold it:

- at 0, the eyes do what the face itself does — its program's motion, or its
  rest — whatever the target says;
- at 1, the eyes hold the target, with no more than micro-motion;
- in between, a blend of the two.

The weight rests at 0. Since the target rests at the centre, the weight is
what tells "no target" from "look straight ahead": a caller sets the target,
then raises the weight.

A face implements the target as its author chooses, like any control, in one
of two places:

- **its program**, when a program drives the eyes: the program blends the
  target with its own eye motion by the weight — Quori's conversational
  program moves the centre of its gaze toward the target and fades its slow
  wander as the weight rises, keeping its jitter. On such a face the eye
  positions are the program's output, and the target is how a caller
  directs the gaze;
- **its standard adaptation**, for a face without such a program: the
  adaptation maps the target onto the eyes statelessly — adding the
  target's offset from the centre, scaled by the weight, to the eye
  positions, for instance.

Never both in one face: both would write the eyes, and a key has one writer.
`validate` reports whether the face's graphs read the target and its
weight, in the gaze tier, without grading them: the eye positions the tier
grades already reach every gaze the target directs. The ROS4HRI mapping
writes neither; it drives the eye positions from ROS4HRI's own target, a
point in space.

## Semantic tier

### Expressions

One weight per named expression, at `standard/vizij/expression/<name>`, 27 in
all. The standard does **not** prescribe what an expression looks like — that
is the face's authored pose; it prescribes only the name a caller commands.

ROS4HRI's `hri_msgs/Expression` vocabulary (25), which the ROS4HRI mapping
commands by name:

```
neutral   angry      sad         happy        surprised
disgusted scared     pleading    vulnerable   despaired
guilty    disappointed embarrassed horrified  skeptical
annoyed   furious    suspicious  rejected     bored
tired     asleep     confused    amazed       excited
```

and the expressions faces implement beyond it, which no ROS4HRI name reaches:

```
concerned sleepy
```

### Visemes

One weight per viseme shape, at `standard/vizij/viseme/<shape>`. The shapes are
the industry 15-shape set (Oculus/Meta naming); `sil` is silence.

```
sil PP FF TH DD kk CH SS nn RR aa E ih oh ou
```

The weights are raw: a face maps each onto its own poses, scaled by its
lipsync gain, with no transition of its own. Rest is the face's neutral — every viseme weight at
zero, `sil` included (a viseme player writes the `sil` weight, never drives
it). The timing is the player's: the attack, hold and release of a played
viseme and the crossfade to the next shape come from the viseme players
Vizij ships as [skills](skills.md), which share one lipsync driver:
`play_viseme(shape, weight)`, one shape through a lipsync envelope;
`set_viseme(viseme)`, one shape held until the next lips command;
`play_visemes(visemes, starts, durations)`, a viseme timeline; and
`say(text, voice)`, whose run streams the visemes of the speech it
synthesizes.

The **lipsync gain**, at `standard/vizij/lipsync/gain`, is a
[setting](#settings) of the face rather than a control: how strongly it shows
the viseme weights. The
face's adaptation multiplies every viseme weight by it before mapping it onto
a pose, each pose clamped to [0, 1]. A face whose mouth shapes read too strong
or too weak is tuned once rather than on every call, and a caller's weight
keeps its meaning. The gain rests at the value the adaptation is authored
with, 1 unless the face says otherwise, and a deployment overrides it by
writing the key. The profile declares it as an `input` with no tier, so it
doesn't count toward a face's compliance level.

The face's **current viseme** is state, at `standard/vizij/viseme`: one of the
shapes as a string, `sil` at rest, written by whichever player is driving the
lips — for anything that follows speech (subtitles, a mirror face, a
monitor). Next to it, `standard/vizij/speech` is **what the face is saying**:
the utterance of a `say` run from the moment its audio starts playing until it
ends, empty at rest. Neither is a command: the players are the actions; this
is what they report, and what their runs feed back. Both are declared as the
`vizij-face` profile's `output` keys, the rest being its `input` keys.

## Muscle tier

Fine-grained controls at `standard/vizij/face/<control>`, cherry-picked from two
standards so each is reachable from either:

- **FACS supplies the taxonomy.** Each control names a facial action unit, so
  an `hri_msgs/FacialActionUnits` message maps onto the muscle tier
  losslessly.
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

One more muscle-tier path, `standard/vizij/mouth/morph/jaw_open`, is the same
muscle as `jaw_open` (AU 26, ARKit `jawOpen`) under the de-facto name every
current face implements.

ROS4HRI has no counterpart the mapping reads: it publishes action units only
as perception output (`/humans/faces/<faceID>/facs`, one message per detected
face) and defines no command for them, so the ROS4HRI mapping writes none of
this tier.

## Conversation state

| Path | Range | Meaning |
|---|---|---|
| `standard/vizij/conversation/speaking` | `[0, 1]` | the face is speaking: 1 from the moment its utterance starts (a `say` run is spawned) until it ends or is stopped |
| `standard/vizij/conversation/user_speaking` | `[0, 1]` | the person the face is talking with is speaking |
| `standard/vizij/conversation/thinking` | `[0, 1]` | the agent is preparing its reply |

Each is written 0 or 1 by whoever runs the dialogue — the agent the face speaks
for — and rests at 0. They describe the dialogue, not the face: a face reacts as
its author chooses (a listening pose, an averted gaze while thinking) or not at
all, and nothing in the standard writes them on its own. ROS4HRI has no
counterpart the mapping reads.

## Settings

| Path | Type | Rests at | Meaning |
|---|---|---|---|
| `standard/vizij/lipsync/gain` | f32 ≥ 0 | 1 | how strongly the face shows the viseme weights (see [Visemes](#visemes)) |
| `standard/vizij/gaze/eye_spacing` | f32 ≥ 0, metres | 0.06 | the distance between the centres of the face's eyes, as the robot shows them |
| `standard/vizij/gaze/range` | f32 in [0.01, 0.9], radians | 0.78 | the gaze angle off the face's axis at which an eye reaches the end of its travel |

A setting says what the face is like rather than what it is told to do. The
face authors it in its own graphs: an `input` node on the setting's path, in
its standard adaptation, whose default — an `f32` — is the face's value —

```json
{ "id": "in_gaze_range", "type": "input",
  "params": { "path": "rig/<faceId>/standard/vizij/gaze/range", "value": 0.6 } }
```

— and a face that authors none rests at the table's value. A mapping composed
over the face reads the setting under the face's prefix and rests it where
the face does, so a mapping is tuned to a face by the face's own content,
never by a forked copy of the mapping: the mapping is the same for every face,
the setting is what differs. A deployment overrides a setting by writing the
key, and a `reset` returns it to the face's value. Nothing animates a setting.
The profile declares each as an `input` in no tier, so none counts toward a
face's compliance level.

The eye settings are what a mapping aiming the eyes at a point in space needs
to know of the face. The ROS4HRI mapping turns each eye toward the point from
its own centre, half the spacing to either side of the face's, and maps an
angle of ±range onto an eye position of ±1. The values a face authoring none
rests at are the incumbent ROS4HRI face's, which the shipped faces share. The
spacing is the eyes' distance as displayed, so a deployment whose display
shows the face larger or smaller writes it. The range stops at 0.9 rad, past
which the mapping's arctangent no longer reaches the end of the eyes' travel;
the mapping reads a value outside a setting's span at the nearest bound.

## Caption

| Path | Type | Meaning |
|---|---|---|
| `standard/vizij/caption/text` | string | the caption the face shows, empty at rest |
| `standard/vizij/caption/speaker` | string | who says it: `__system__` for the face itself, any other id for someone else, empty when unknown |

The caption is text the host draws over the face — a command of the face's
display, not of its rig, so it is in no tier and no face's graphs read it.
Each new value of the pair is a new caption; writing the same caption again
does not show it again. The host shows it at the bottom of the face's own
rectangle, the way the incumbent ROS4HRI face does:

- in chunks of eight words, each held for its words at 200 words a minute
  but at least 2 s, the last one of a caption 1.5 s longer;
- after the chunks still queued, so a caption arriving while another shows
  waits its turn;
- in blue (sRGB `#0066B3`) when the face itself speaks, in white otherwise,
  over a black drop shadow;
- a line one fifteenth of the rectangle's height, centred in a band a fifth
  of the height tall, whose bottom stands a thirtieth of the height above
  the rectangle's.

`__system__` is the id ROS4HRI's `ClosedCaption` gives the robot itself,
taken as is, so the [ROS4HRI mapping](ros4hri.md) relays a closed caption
without translating its speaker. A face whose composition writes neither
key has both open to remote writers, so any client captions it; with the
ROS4HRI mapping composed, the mapping writes them, from `/closed_captions`.
The native view draws captions (the `vizij` crate's `captions` feature, on
by default); the browser module does not.

## Reaching the vocabulary in code

`vizij-arora-host`'s `standard` module exposes the constants (the gaze
target's among them, `GAZE_TARGET_X` and `GAZE_TARGET_Y`) and the path
helpers (`expression_path`, `viseme_path`, `face_path`, `conversation_path`),
the name sets (`expression_names()` — `ROS4HRI_EXPRESSION_NAMES` then
`VIZIJ_EXPRESSION_NAMES` — `VISEME_SHAPES`, `CONVERSATION_STATES`), the
settings (`LIPSYNC_GAIN`, `GAZE_EYE_SPACING`, `GAZE_RANGE`, listed as
`FACE_SETTINGS`), the caption's (`CAPTION_TEXT`, `CAPTION_SPEAKER`,
`CAPTION_SPEAKER_SELF`), plus
`controls_for_au(code)` (the lateralized pair or single control an AU drives)
and `control_for_arkit(name)`. Callers should build paths through these rather
than hand-format strings.

## See also

- [Profiles and mappings](profiles-and-mappings.md) — what a profile is, and
  how a mapping implements one in terms of another.
- [ROS4HRI](ros4hri.md) — the built-in mapping that fills this vocabulary from
  ROS4HRI's topics.
- [Skills](skills.md) — the goal-driven behaviors that write it: the gaze
  skill, the expression setters and the viseme players.
- [`vizij-bundle`](../crates/tools/vizij-bundle/README.md) — the tool that
  reports which tiers a face covers (`validate`), declares a profile on a face
  GLB (`add-profile`), and embeds a standard mapping into it
  (`add-standard`).
