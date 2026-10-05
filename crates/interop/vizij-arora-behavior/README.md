# vizij-arora-behavior

Drives a Vizij node graph as an Arora **behavior interpreter**. The interpreter
type is `ProcessingGraph`, which implements
[`arora_behavior::BehaviorInterpreter`](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior/src/lib.rs):
each tick it reads its subscribed input paths from the shared data store,
evaluates the [`vizij-graph-core`](../../node-graph/vizij-graph-core/) graph for
`dt`, and writes the graph's outputs back. Vizij and Arora share one runtime
value type, so values cross the store and call boundaries with no conversion.

**How it works, with diagrams:** [`docs/node-graph.md`](docs/node-graph.md) walks
this interpreter against Arora's interpreter model — load, tick, the store seam,
module/animation calls, and graph editing — and draws the parallel with the
[behavior tree](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior-tree/docs/nodes.md).
It builds on Arora's
[interpreter workflow](https://github.com/semio-ai/arora-sdk/blob/main/crates/arora-behavior/docs/interpreter-workflow.md).

## Task runs

The interpreter hosts task runs — what the interpreter module's SPAWN
starts and HALT stops — as fragments of its one graph, each under
`task/<run id>/`, beside the main behavior. A run's status, feedback and
result keys live under `arora/tasks/<module>/<function>/<run id>/`.

- **Skills** (`gaze`, `viseme`, `speech`): a registered `TaskFragment`, the
  behavior of a described method as graph data.
- **`run_behavior(name, behavior)`** (`run`): the generic method the
  interpreter implements itself. Its spawn carries the graph to run, so a
  program — a face's motion graph, an editor's live graph — is a run like
  any other: started by SPAWN, stopped by HALT, its state its status key,
  its name a key beside it that any client reads (`run::runs`). A halt
  leaves the store as it is; returning a run's outputs to rest is the
  client's step.
- **Any other call**: the generic wrapper, a `TaskRun` node calling the
  module function each tick.

A run's nodes are the graph's nodes under its id: an EDIT there changes the
running behavior in place (`run::edit` builds one; kept nodes keep their
state), and a LOAD of the main behavior leaves the live runs running.
