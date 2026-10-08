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
| `quori/Quori_Current.standard-adaptation.json` | `apps/vizij-authoring/public/assets/Quori_Current.glb` | top eyelids, conversation |
| `quori/quori_temp.standard-adaptation.json` | `apps/vizij-authoring/public/assets/quori_temp.glb` | every expression and viseme |
| `quori/Quori_Live.standard-adaptation.json` | `apps/tutorial-agent-face/public/assets/Quori_Live.glb` | top eyelids, conversation |
| `quori/Quori_Latest_Rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Quori_Latest_Rigged.glb` | top eyelids, conversation |
| `quori/Quori_Speaks.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Quori_Speaks.glb` | top eyelids, conversation |
| `quori/Quori_Std_Rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Quori_Std_Rigged.glb` | blink, conversation |
| `hugo/Hugo_Current.standard-adaptation.json` | `apps/vizij-authoring/public/assets/Hugo_Current.glb` | top eyelids, conversation |
| `hugo/Hugo_Current_Extended.standard-adaptation.json` | `apps/vizij-authoring/public/assets/Hugo_Current_Extended.glb` | top eyelids, conversation |
| `hugo/Hugo_Latest_Rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/Hugo_Latest_Rigged.glb` | top eyelids, conversation |
| `hugo/hugo_rigged.standard-adaptation.json` | `apps/vizij-showcase/public/assets/hugo_rigged.glb` | top eyelids, blink, conversation |

A control is left unmapped when the face has nothing that reads it: no pose,
or an input no edge of its graphs leaves. Mapping a standard control onto such
an input would make `validate` count it as covered while it moves nothing.
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
  `t`, …); `sil`, the closed-mouth rest, is listened to and drives nothing.
- **Gaze** — a rig that listens on `standard/vizij/*` needs nothing; a rig
  with the earlier `standard/<eye>/pos/*` inputs takes them one to one; a rig
  that moves its eyes only through `propsrig/<l|r>_eye/translation/*` takes
  the control through the input's authored range: -1 → min, 0 → its rest,
  1 → max.
- **Blink** — the rig's `lids/blink` or `blink` input.
- **Conversation state** — the rig's `speech/<state>` inputs, where a graph
  of the face reads them.
