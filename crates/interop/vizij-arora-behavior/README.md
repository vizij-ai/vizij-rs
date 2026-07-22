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
