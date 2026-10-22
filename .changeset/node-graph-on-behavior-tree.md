---
"@vizij/node-graph": minor
---

A graph runs on arora-behavior-tree. `WasmGraph` imports its spec as a behavior tree whose nodes call the node functions, and `evalAll` still evaluates every node and reports every port, each computing what it did. A node calls its function again only when its inputs, its parameters or the clock it reads changed. `setParam` sets a parameter's value in place. An edit that changes a node's ports (a split's sizes, a record's keys), an input's or output's path, or a parameter the node did not set, loads the whole graph again, as `loadGraph` does, and every node keeps its state. Measured natively, a frame of a graph whose inputs hold costs from 1.5 times more (a few dozen nodes) to 2.3 times less (19,000 nodes). A frame where every input moves costs up to 3 times more below a few thousand nodes, and 1.7 times less at 19,000. Loading a graph costs 3 to 11 times more.
