# Face adaptations

Each file here is the `standard-adaptation` graph of one face GLB shipped in
[vizij-web](https://github.com/vizij-ai/vizij-web): the mapping from the
`vizij-face` profile's controls (`standard/vizij/*`) onto that face's own rig.
A sidecar is the reviewable source of truth; the GLB carries a copy, grafted
under the id `<faceId>_standard_adaptation`:

```bash
vizij-bundle add-graph   face.glb --graph <sidecar> --kind standard-adaptation \
                         --id <faceId>_standard_adaptation -o face.glb
vizij-bundle add-profile face.glb --profile vizij-face -o face.glb
vizij-bundle validate    face.glb --min-level 2
```

An adaptation addresses one rig — its prefix (`rig/<faceId>/`) and its own
paths — so a sidecar belongs to one GLB, not to a character: the shipped
Quoris and Hugos are rigged differently from file to file.

| Sidecar | GLB in vizij-web | Standard controls left unmapped |
|---|---|---|
| `quori/standard-adaptation.json` | `apps/vizij-authoring/public/assets/Quori_Current_Extended.glb` | none |
| `quori/Quori_Current.standard-adaptation.json` | `apps/vizij-authoring/public/assets/Quori_Current.glb` | top eyelids, gaze target, conversation |
| `quori/quori_temp.standard-adaptation.json` | `apps/vizij-authoring/public/assets/quori_temp.glb` | every expression and viseme |
| `quori/Quori_Live.standard-adaptation.json` | `apps/tutorial-agent-face/public/assets/Quori_Live.glb` | top eyelids, gaze target, conversation |
| `quori/Quori_Latest_Rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Quori_Latest_Rigged.glb` | top eyelids, gaze target, conversation |
| `quori/Quori_Speaks.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Quori_Speaks.glb` | top eyelids, gaze target, conversation |
| `quori/Quori_Std_Rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Quori_Std_Rigged.glb` | blink, gaze target, conversation |
| `hugo/Hugo_Current.standard-adaptation.json` | `apps/vizij-authoring/public/assets/Hugo_Current.glb` | top eyelids, gaze target, conversation |
| `hugo/Hugo_Current_Extended.standard-adaptation.json` | `apps/vizij-authoring/public/assets/Hugo_Current_Extended.glb` | top eyelids, gaze target, conversation |
| `hugo/Hugo_Latest_Rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Hugo_Latest_Rigged.glb` | top eyelids, gaze target, conversation |
| `hugo/hugo_rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/hugo_rigged.glb` | top eyelids, blink, gaze target, conversation |

A control is left unmapped when the face has nothing that reads it: no pose,
or an input no edge of its graphs leaves. Mapping a standard control onto such
an input would make `validate` count it as covered while it moves nothing. The
two faces whose active program is "Speaks", `Quori_Current_Extended.glb` and
`quori_temp.glb`, read the conversation state and the gaze target in that
program; their adaptations map neither.
`Toasty_Current.glb` and `@vizij/render`'s `example.glb` have no adaptation:
neither rig has a pose or an eye control the profile's controls could drive.

## How a sidecar maps

`quori/standard-adaptation.json` is the reference; the others are generated
from their GLB by vizij-web's `scripts/add-standard-adaptation.mjs --sidecar`,
which applies the same mapping to whatever controls the rig has:

- **Expressions** — the profile's 27 fold onto the seven emotion poses these
  faces author (anger, sad, concerned, surprise, happy, sleepy, neutral): each
  pose weight is the sum of the expressions that drive it, clamped to [0, 1].
- **Visemes** — the 15 shapes onto the letter poses (`PP` → `p`, `DD`/`nn` →
  `t`, …), each pose's sum scaled by the face's lipsync gain
  (`lipsync/gain`, resting at 1) before the clamp; `sil`, the closed-mouth
  rest, is listened to and drives nothing.
- **Gaze** — a rig that listens on `standard/vizij/*` needs nothing; a rig
  with the earlier `standard/<eye>/pos/*` inputs takes them one to one; a rig
  that moves its eyes only through `propsrig/<l|r>_eye/translation/*` takes
  the control through the input's authored range: -1 → min, 0 → its rest,
  1 → max.
- **Blink** — the rig's `lids/blink` or `blink` input.
- **Conversation state** — the rig's `speech/<state>` inputs, where a graph
  of the face reads them.

The reference sidecar alone also maps the **muscle tier**
(`standard/vizij/face/<control>`, each a `[0, 1]` weight) onto Quori's own
brow, lid and mouth inputs, which the script does not generate. Each rig
input is the weighted sum of the controls that drive it, clamped to the
input's authored range; a lateralized control drives its own side, a pair
onto one input averages. The signs follow the rig: a brow's `*_up_down` is up
when positive, its `in_left_right` toward the centre, the lower lid's
`pos/y` up, `corner_puller` the corners up, `stretch` the mouth wider, a top
sneer the upper lip up and a bottom sneer the lower lip down.

| Rig input (`standard/vizij/…`) | Range | Driven by |
|---|---|---|
| `<side>_eye_brow/morph/in_up_down` | [-1, 1] | `brow_inner_up` − `brow_down_<side>` |
| `<side>_eye_brow/morph/mid_up_down` | [-1, 1] | ½ `brow_inner_up` + ½ `brow_outer_up_<side>` − ½ `brow_down_<side>` |
| `<side>_eye_brow/morph/out_up_down` | [-1, 1] | `brow_outer_up_<side>` |
| `<side>_eye_brow/morph/in_left_right` | [-1, 1] | `brow_down_<side>` |
| `<side>_eye_bottom_eyelid/pos/y` | [0, 1] | ½ `eye_squint_<side>` + 0.3 `cheek_raise_<side>` |
| `<side>_eye_bottom_eyelid/morph/curve` | [0, 1] | `cheek_raise_<side>` |
| `mouth/morph/corner_puller` | [-1, 1] | ½ `mouth_smile_*` − ½ `mouth_frown_*`, both sides |
| `mouth/morph/stretch` | [-1, 1] | ½ `mouth_smile_*` + ½ `mouth_stretch_*`, both sides, − `mouth_pucker` − ½ `mouth_funnel` |
| `mouth/morph/<side>_top_sneer` | [-1, 1] | `mouth_upper_up_<side>` + ½ `nose_sneer_<side>` |
| `mouth/morph/<side>_bottom_sneer` | [-1, 1] | `mouth_lower_down_<side>` |
| `lids/blink` (not `standard/`) | [0, 1] | `blink` + ½ `eye_closed_*`, both sides |

A smile is the corners up and the mouth wider, the two components of Quori's
own `mouth/smile` input; the adaptation drives the components because Quori's
animations write `mouth/smile` itself. The eyes closed join the blink rather
than the top lids' `pos/y`, which the ROS4HRI mapping's blink writes. The jaw
(`jaw_open`) reaches the rig through the de-facto `mouth/morph/jaw_open`,
which the setter writes, so the sidecar does not map it again. Unmapped:
`eye_wide_*` (no rig input opens the top lids past their rest),
`cheek_puff`, `mouth_press_*`, `jaw_left`, `jaw_right`, `jaw_forward` and
`tongue_out`. Quori's "Speaks" program writes the same brow, lid and mouth
inputs and composes after the adaptation, so while it runs it owns them.
