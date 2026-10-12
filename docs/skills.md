# Skills

A skill is a device method whose behavior is **graph data**: a fragment the
device's node-graph interpreter grafts into the running graph per call (a
*run*), rather than host code. The exterior contract — the function's id,
name and signature, which bridges discover over `DescribeMethods`, its
`Status` return the task-run marker — is declared once per skill in
`vizij_arora_host::skills`, as a trait (`#[arora_module::contract]`). No
module implements a skill but `say`: the other fragments carry the
contracts' descriptions, so the device's interpreter describes the methods,
listed under the interpreter module, and a remote spawns them through it.
`say`'s fragment wraps the text-to-speech provider's call, which the provider
describes under its own module id. The behavior is a canonical JSON
asset in `vizij-arora-host` (`skills/<id>.json`), regenerable from its
builder (`vizij-bundle export-skill <id>`), drift-tested, and — for `look_at`,
`play_viseme` and `say` — **overridable per face**: a face GLB embedding a
`skill::<id>` graph entry runs its own copy instead of the built-in. A run speaks the placeholder contract: `task/<param>`
inputs are its parameters (live-updatable through the run's update keys),
`task/status` is its lifecycle, `task/feedback` and `task/result` land on the
handle's keys.

| Skill | Signature | What the run does |
|---|---|---|
| `look_at` | `(policy, target, frame) → Status` | ROS4HRI's gaze skill: tracks a target on the standard gaze surface until cancelled, or holds a glance/reset fixation then succeeds — see [ROS4HRI support](ros4hri.md#the-look_at-skill) |
| `set_expression` | `(expression, valence, arousal) → Status` | holds the face's expression: the named one at the arousal as its intensity, or the valence/arousal point blended over the circumplex |
| `set_facial_action_units` | `(codes: [u8], intensities: [f32]) → Status` | holds the face's expression as FACS action units on the muscle tier |
| `play_viseme` | `(shape, weight) → Status` | plays one shape of the face standard's 15 through the lipsync envelope, crossfading the others out |
| `set_viseme` | `(viseme: u8) → Status` | holds the shape with that ROS4HRI code at full weight — the shape at the playhead, for a live producer |
| `play_visemes` | `(visemes: [u8], starts: [f32], durations: [f32]) → Status` | plays a viseme timeline and ends after the last span |
| `say` | `(text, voice?, &mut viseme) → Status` | speaks the text through the build's text-to-speech provider and drives the lips from the visemes it streams |

`set_*` skills set a state that holds until changed: their runs never end on
their own. `play_*` skills, and `say`, play something that ends.

## Who owns a part of the face

Skills that command one part of the face in different ways conflict, so the
device makes each such group an **exclusivity domain**: one run owns the part
at a time, and a new run of any skill of the domain halts the live ones first
— they end preempted, as a preempted goal — whichever skill they ran. The new
run continues from where the face is, so the handover is a crossfade, never
a snap.

- **The expression**: `set_expression` and `set_facial_action_units`.
- **The lips**: `play_viseme`, `set_viseme` and `play_visemes`. A viseme
  streamed into the ROS4HRI mapping plays through `play_viseme`, so it takes
  the lips over from any of them.

A domain is a name the device gives a fragment when it registers it
(`TaskFragment::exclusive_in`); the interpreter halts by domain at spawn. A
skill can instead be exclusive among its own runs only
(`TaskFragment::exclusive`): `say`, whose new utterance takes over from the
one speaking.

`say` is outside the lips' domain because a lips command must not cut speech
short: the audio would stop mid-sentence for a mouth shape. The lips are
**layered** over it instead. The interpreter evaluates the runs in spawn
order, after the main behavior, and the later writer of a key wins, so a
lips player spawned while `say` speaks writes the viseme weights and the
current-viseme state after `say` and shows over it.
`say` keeps speaking and ends with its own status; once the player ends, its
lips pick up again from where the face is — the driver's smoothers read the
face's weights back — so the handback is a crossfade too. A `say` run's
feedback keeps reporting the provider's viseme while it is overlaid.

## The expression setters

Both setters write the face's whole expression — every
`standard/vizij/expression/<name>` weight of the standard, every muscle-tier
control `standard/vizij/face/<control>`, and the de-facto jaw
`standard/vizij/mouth/morph/jaw_open` — each through a smoother at the ROS4HRI
mapping's half-life (140 ms) whose state is the face's own weight read back
from the store. What one setter does not command it rests, so the one that
takes the face over fades the other's expression out as its own fades in.

**`set_expression(expression, valence, arousal)`** computes the weights with
the ROS4HRI mapping's math: a non-empty name — any of the standard's 27 —
selects that expression alone at the arousal as its intensity, an empty one
blends ROS4HRI's 25 by the circumplex. The muscle tier rests.

**`set_facial_action_units(codes, intensities)`** drives each code's
muscle-tier controls (the lateralized pair where the standard splits one) at
its intensity, clamped to `[0, 1]`; AU 26 also drives the de-facto jaw, and
AU 45 (blink) counts as AU 43. It is sparse: a unit the command does not list
rests at zero, since the command describes the whole face as a named
expression does. Codes the muscle tier does not express are ignored, and
lists of mismatched lengths drive nothing. The named expressions rest. A face
shows what its adaptation maps: Quori's reference adaptation maps the brows,
lids, eye closure, lip corners, width and sneers onto its rig (see
[Face adaptations](../fixtures/faces/README.md#how-a-sidecar-maps)), and the
other shipped adaptations map none of the muscle tier, so on those faces only
AU 26, through the de-facto jaw, moves the face.

**The ROS4HRI mapping's expression channel** writes the same weights from the
`standard/ros4hri/expression/*` keys. It runs in the face's graph, before the
runs, so a live setter run writes last and owns the face; once the run is
halted, the mapping takes the expression back from where the run left it (its
smoothers read the face's weights back too). The mapping's blink reads the
face's own `asleep` weight, so `asleep` closes the lids whoever sets it.

## The viseme players

The players write the [face standard](face-standard.md#visemes)'s lipsync
surface — every `standard/vizij/viseme/<shape>` weight and the
current-viseme state `standard/vizij/viseme` — through one **lipsync
driver**: the selected shape's weight is driven to the envelope's value, every
other shape to zero, each through a smoother (30 ms half-life) whose state is
the face's own weight, read back from the store. So whichever run writes a
shape continues its fade from where the last one left it — a run taking over
never snaps, and a run ending leaves nothing mid-fade. A run reports the
current shape and how hard it is driven as its `task/feedback` — a record
`{viseme, intensity}`, the pair a client needs to mirror the lips, and the
fields ROS4HRI's `Say` feedback carries them under as Vizij extends it; the
code-addressed players, `set_viseme` and `play_visemes`, add the shape's code
as `value`, the `u8` of Vizij's `Viseme` message — and a player that ends
does so only once the lips have settled (every weight under 2 %), because a run's
writes stop with its fragment.

**`play_viseme(shape, weight)`** plays `shape` at `weight`: 80 ms in, a 250 ms
hold, 150 ms out — about half a second, a spoken viseme's span with room to
read — then `Success`. A new lips command **takes the lips over** (see [Who owns a part of the
face](#who-owns-a-part-of-the-face)). A lipsync producer calls it once per
viseme; `sil` plays rest.

**`set_viseme(viseme)`** drives the shape whose code is `viseme` — an index
into the standard's 15 shapes, ROS4HRI's order, `0` (`sil`) at rest — at full
weight and holds it until the next lips command.

**`play_visemes(visemes, starts, durations)`** plays an alignment: the shape
`visemes[i]` from `starts[i]` (seconds since the run's start) for
`durations[i]`, rest between spans, at full weight through the driver's
crossfade; it succeeds once the last span has ended and the lips have
settled. Spans are expected not to overlap; where they do, the higher code
shows.

**`say(text, voice)`** hosts the device's `say` call — the text-to-speech
provider's poll-on-tick function, re-invoked every tick while `Running` — on
the run's own argument bundle, and feeds the viseme the provider streams
through its mutable `viseme` parameter into the driver at full weight.
`voice` is optional: without one, the provider speaks in its own default.
`viseme` and `speech` are required arguments like any other; the fragment
passes `sil` and an empty string for them, stated in its task-run node's
`value`, under the bundle's own arguments. The
provider's viseme is already a standard shape: the cloud provider maps AWS
Polly's viseme codes, the Piper provider maps espeak-ng phonemes. The
provider also reports the utterance through its mutable `speech` parameter
while its audio plays — from the moment playback starts, whether or not
synthesis had finished, empty before and after — and the run writes it as
the face's speech state, `standard/vizij/speech`, for whatever follows
speech (subtitles: the ROS4HRI mapping relays it to the topic the bridge
publishes). The run's status is the call's, once the lips have settled after
the utterance. See [Speech (TTS)](../crates/vizij/README.md#speech-tts) for
the providers.

## Who plays the visemes

A player is called, never subscribed to: some producer decides what the mouth
does. `say` is its own producer — the run's provider streams the visemes of
the speech it synthesizes. For anything else, the caller supplies them: a
behavior, an action client, or a phoneme aligner calling `play_viseme` once
per shape. A producer that keeps its own envelope bypasses the players and
writes the weights: each `standard/vizij/viseme/<shape>` is a free input,
under `--ros2` a subscribed topic ([ROS4HRI
support](ros4hri.md#driving-a-key-from-ros-2)).

One producer exists outside the players and does not reach them:

- **The web.** vizij-web runs its own lipsync in JS, against the face's pose
  weights directly (`rig/<faceId>/poses/<poseId>.weight`) rather than the
  standard's viseme surface — `@vizij/speech-react`'s Polly speech-mark cursor
  in vizij-standalone, a phoneme aligner in the agent-face tutorial, each with
  its own crossfade. The skills registry
  ([`@vizij/runtime`](../npm/@vizij/runtime/README.md)) serves the fragments to
  the web, but the standalone app does not yet register the viseme players,
  so the two platforms lip-sync by different means.

## Calling a skill

A skill is a described device method, so a behavior calls it like any
module function, and a bridge spawns it as a task run (the interpreter
module's SPAWN, `arora-behavior`'s `TaskHandle` coming back with the run's
status, feedback, result and update keys). On ROS 2 every skill is an
action server: the ROS4HRI exposure preset binds `look_at` to the standard
`/skill/look_at` (`interaction_skills/action/LookAt`), and the bridge
binds `say` to ROS4HRI's speech skill, `/skill/say`
(`communication_skills/action/Say`): the goal's `input` is the utterance, and
the feedback carries the run's `{viseme, intensity}` in the fields Vizij adds
to the standard's `Say` feedback — a lipsync stream any client built from
that definition reads. The preset also binds the face skills: the topic
skills `/skill/set_expression` (`interaction_skills/SetExpression`),
`/skill/set_facial_action_units` and `/skill/set_viseme`
(`vizij_face_msgs`), each message spawning a run, and the action
`/skill/play_visemes` (`vizij_face_msgs/action/PlayVisemes`), whose feedback
is the run's current viseme code. These skills are exposed under the ROS4HRI
exposure profile only. The bridge also synthesizes an action from the signature of each
described skill whose parameters ROS 2 can carry —
`/<namespace>/actions/play_viseme` (`arora/action/play_viseme`, goal `shape`,
`weight`) — discovered over DDS and rmw_zenoh alike, but its `arora`
interface is not a ROS package, so a client cannot build its goal until the
definition is generated for it. `say` has no synthesized action: ROS 2 has no
form for its optional `voice`, so `/skill/say` is its action.

## In code and on the web

`vizij_arora_host::skills` holds the skill contracts (`LookAt`,
`SetExpression`, `SetFacialActionUnits`, `PlayViseme`, `SetViseme`,
`PlayVisemes`, `Say`, with their ids under `<skill>::ids`), the registry (`SKILLS`, `skill(id)`, `skill_source(id)`) and the
fragment generators; `vizij_arora_behavior::{gaze, expression, viseme,
speech}` re-export the contracts beside the fragments a device registers
(with the face's rig prefix on the standard controls a fragment writes, and
their exclusivity domains, `expression::EXPRESSION` and `viseme::LIPS`) — all
but say's `described`, so the interpreter describes their methods. A text-to-speech
provider implements `Say` under its own module id and registers with
`HostModule::from_exports(id, say::exports(provider))`.
[`@vizij/runtime`](../npm/@vizij/runtime/README.md) serves the same registry
to the web (`skills()`, `skillSource(id)`), and the vizij-web authoring app
embeds and edits skill fragments from **File → Skills**.
