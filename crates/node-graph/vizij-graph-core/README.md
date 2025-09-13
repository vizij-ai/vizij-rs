# vizij-graph-core

Engine-agnostic runtime node graph with **runtime-declared ports** and values.
This refactor removes any `bevy_ecs` dependency and provides a pure Rust evaluator.

### TODO:

Inverse kinematics should be able to solve for dynamic number of arms with specified joint constraints. Need a 2d and 3d variant.

Need to update the data type to incorporate labels for nodes and values.

Need to be able to group nodes into repeatable functional blocks (note - they are not new nodes, but grouping nodes should clarify the inputs and outputs of a functional block).

Need to be able to load structured labeled data blocks.

Need to be able to load vectors and do weighted averages of them. 