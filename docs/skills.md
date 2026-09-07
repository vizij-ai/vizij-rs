# Skills

A skill is a device method whose behavior is **graph data**: a fragment the
device's node-graph interpreter grafts into the running graph per call (a
*run*), rather than host code. The exterior contract — the described
signature bridges discover over `DescribeMethods`, its `Status` return the
task-run marker — lives with the device; the behavior is a canonical JSON
asset in `vizij-arora-host` (`skills/<id>.json`), regenerable from its
builder (`vizij-bundle export-skill <id>`), drift-tested, and **overridable
per face**: a face GLB embedding a `skill::<id>` graph entry runs its own copy
instead of the built-in. A run speaks the placeholder contract: `task/<param>`
inputs are its parameters (live-updatable through the run's update keys),
`task/status` is its lifecycle, `task/feedback` and `task/result` land on the
handle's keys.

Three skills ship.

| Skill | Signature | What the run does |
|---|---|---|
| `look_at` | `(policy, target, frame) → Status` | ROS4HRI's gaze skill: tracks a target on the standard gaze surface until cancelled, or holds a glance/reset fixation then succeeds — see [ROS4HRI support](ros4hri.md#the-look_at-skill) |
| `play_viseme` | `(shape, weight) → Status` | plays one shape of the face standard's 15 through the lipsync envelope, crossfading the others out |
| `say` | `(text, voice) → Status` | speaks the text through the build's text-to-speech provider and drives the lips from the visemes it streams |

## The viseme players

Both players write the [face standard](face-standard.md#visemes)'s lipsync
surface — every `standard/vizij/viseme/<shape>` weight and the
current-viseme state `standard/vizij/viseme` — through one **lipsync
driver**: the selected shape's weight is driven to the envelope's value, every
other shape to zero, each through a smoother (30 ms half-life) whose state is
the face's own weight, read back from the store. So whichever run writes a
shape continues its fade from where the last one left it — a run taking over
never snaps, and a run ending leaves nothing mid-fade. A run reports the
current shape as its `task/feedback` and ends only once the lips have settled
(every weight under 2 %), because a run's writes stop with its fragment.

**`play_viseme(shape, weight)`** plays `shape` at `weight`: 80 ms in, a 250 ms
hold, 150 ms out — about half a second, a spoken viseme's span with room to
read — then `Success`. A new call **takes the lips over**: the run before it
ends preempted (its status a failure, as a preempted goal), and the new run
alone writes. A lipsync producer calls it once per viseme; `sil` plays rest.

**`say(text, voice)`** hosts the device's `say` call — the text-to-speech
provider's poll-on-tick function, re-invoked every tick while `Running` — on
the run's own argument bundle, and feeds the viseme the provider streams
through its mutable `viseme` parameter into the driver at full weight. The
provider's viseme is already a standard shape: the cloud provider maps AWS
Polly's viseme codes, the Piper provider maps espeak-ng phonemes. The run's
status is the call's, once the lips have settled after the utterance. See
[Speech (TTS)](../crates/vizij/README.md#speech-tts) for the providers.

## Calling a skill

A skill is a described device method, so a behavior calls it like any
module function, and a bridge spawns it as a task run (the interpreter
module's SPAWN, `arora-behavior`'s `TaskHandle` coming back with the run's
status, feedback, result and update keys). On ROS 2 every skill is an
action server: the ROS4HRI exposure preset binds `look_at` to the standard
`/skill/look_at` (`interaction_skills/action/LookAt`), and the bridge
synthesizes one action per described skill from its signature —
`/<namespace>/actions/play_viseme` (`arora/action/play_viseme`, goal
`shape`, `weight`) and `/<namespace>/actions/say` (`arora/action/say`, goal
`text`, `voice`), discovered over DDS and rmw_zenoh alike. Their `arora`
interfaces are synthesized, not a ROS package, so a client needs the
definitions to send a goal; `ros2 action info -t` shows them, `ros2 action
send_goal` cannot build the goal without the package.

## In code and on the web

`vizij_arora_host::skills` holds the registry (`SKILLS`, `skill(id)`,
`skill_source(id)`), the fragment generators, and the parameter lists each
contract derives from; `vizij_arora_behavior::{gaze, viseme, speech}` hold the
described signatures and the fragments a device registers (with the face's
rig prefix on the standard controls a fragment writes).
[`@vizij/runtime`](../npm/@vizij/runtime/README.md) serves the same registry
to the web (`skills()`, `skillSource(id)`), and the vizij-web authoring app
embeds and edits skill fragments from **File → Skills**.
