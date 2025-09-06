# bevy_vizij_graph

Bevy ECS adapter for `vizij-graph-core`.

- Holds a `GraphSpec` resource and a `GraphRuntime` (time `t` + outputs map).

- Each frame, evaluates the graph and exposes the outputs for other systems.

- Includes an event API to set node params from gameplay code.
