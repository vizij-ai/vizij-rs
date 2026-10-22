---
"@vizij/runtime": minor
---

A face's graphs run on arora-behavior-tree: each step runs the nodes the graphs' outputs read whose inputs changed, and the graphs compute what they did. Measured natively against the evaluation this replaces, a step of a 950-node graph costs 1.2 times less when its inputs hold and 2.4 times more when every input moves, and a step of a 19,000-node graph costs 5.6 and 2.8 times less. Loading a graph, or editing it, costs 2 to 6 times more.
