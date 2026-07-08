# Design note — behavior recursion & interpreter composition

**Status:** forward-looking design note, not a plan for this iteration. It records a *future to favor* so the seams we cut now (VIZ-53: the single node-graph `BehaviorInterpreter`, the `GraphHost` module-call seam, the `ModuleCall` node) don't foreclose it. The load-bearing primitive here (dispatching a behavior to an interpreter) is an **arora core** concern and should graduate to `arora-sdk` when it becomes concrete; it lives here for now because this iteration is its first motivating consumer.

## Where this comes from

The current step turns the Vizij orchestrator into **one graph run by one node-graph interpreter**, with a graph node that calls a module function (animation) via the `CallBridge`. Two adjacent realities show up almost immediately:

1. **A graph will contain other graphs.** A behavior graph deals with animation *and* other things, including a **user-provided graph**. That is a graph inside a graph — naturally expressed as *a node that contains a whole graph*.
2. **A node may target more than a module.** Today a node's call target is a module function. But if the node could target *an interpreter + a behavior*, then the graph becomes the glue that **combines several interpreters** — a graph node handing off to a behavior-tree interpreter, or another graph interpreter, or a module, through one seam.

Neither is built now. The point is to shape the current seams so both are cheap to add later.

## 1 — Recursion: a graph inside a graph (subgraph nodes)

A **subgraph node** holds a whole behavior graph; the node's ports are the inner graph's inputs/outputs.

- **Intelligibility.** A subgraph node presents as a collapsed, nested, *editable* graph — you descend into it to edit the inner behavior. This is coherent precisely because the editable representation is itself recursive (it ties directly to graph-edit on the trait, ARORA-53: a `BehaviorInterpreter` exposes the behavior it runs, and a behavior may contain behaviors).
- **Efficiency — two strategies, keep both on the table:**
  - **(a) Build-time inlining.** Flatten the subgraph into the parent `GraphSpec` before evaluation — exactly what `merged_with_options` already does for merge (it rewrites topology at build time). Zero runtime indirection, one `evaluate_all`. Best when the inner behavior is fixed at build time.
  - **(b) Runtime recursion.** The node evaluates the inner graph on demand. Needed when the inner behavior is chosen/swapped at runtime, or is run by a *different* interpreter (see §2). The shared store/blackboard and the golden clock keys thread down unchanged, so the inner behavior gets time and data for free.
- **Near-term stance:** a subgraph of the *same* interpreter can be build-time-inlined (reuse the merge machinery); leave the door open to runtime recursion for the cross-interpreter and live-swap cases.

## 2 — Composition: a node that calls an interpreter, not just a module

Generalise the node's call target from "a module function" to a small sum:

```
CallTarget =
  | ModuleFn { module, function }              // today's ModuleCall
  | Behavior { interpreter, behavior_ref }      // run a sub-behavior under a named interpreter
```

- The **module call is the degenerate case** — the "interpreter" is the engine's module dispatch. So this doesn't replace anything; it widens it.
- With `Behavior` targets, a **graph node can invoke a behavior-tree sub-behavior** (run by the BT interpreter), **another graph** (run by the graph interpreter), or a **module function** — all through one dispatch. That *is* "combining several interpreters": the graph is the composition surface, and any node can hand off to any interpreter over a sub-behavior, getting results back the same way a module call does.
- The **`CallBridge` is the natural home** for this generalisation: it already dispatches module functions and returns a `Value`. A sibling capability (or an extension) that dispatches a *behavior* to a *named interpreter* — returning results the same shape — makes interpreters first-class callables. Recursion (§1) is then just a `Behavior` target whose interpreter is the graph interpreter itself.

## How this favours the current iteration (concrete seam guidance)

None of the below is extra work now — it is *how to shape* the Step C/D seams so the future is additive rather than a rewrite:

- **Step C `GraphHost` seam:** define the host callout as a general `call(target, args) -> result` where `target` is an **opaque, extensible descriptor** — *not* narrowly `call_module(module_id, fn_id, args)`. The `ModuleCall` node is then one `target` kind; a future `BehaviorCall` (interpreter + behavior) is another, with no change to the seam. Keep `GraphHost` arora-agnostic (it already must be, to keep `vizij-graph-core` free of arora deps).
- **The `ModuleCall` node** should carry an **extensible target descriptor**, not a hardcoded `(module_id, function_id)` pair, so a subgraph/interpreter target slots into the same node family (or a sibling node) later.
- **Keep merge build-time.** `merged_with_options` stays the build-time rewrite; subgraph inlining reuses it; runtime recursion is purely additive on top.
- **Lean on the shared context.** The golden clock keys + the shared store already give any nested behavior a uniform context (time and data flow down). Recursion and interpreter hand-off inherit that for free — don't invent a parallel channel.

## Non-goals (this iteration)

Do **not** build subgraph nodes or interpreter composition now. The only ask is: an **extensible call-target descriptor** and a **general host-call seam**, so neither future needs the seam re-cut.

## Open questions (for when this becomes concrete, likely in arora-sdk)

- **Editing recursion (ARORA-53):** how graph-edit descends into subgraph nodes; how a `Behavior` target's inner behavior is surfaced for editing.
- **Interpreter identity:** how interpreters are named/resolved for a `Behavior` target (a small registry? a well-known id, like modules have?).
- **Termination / cycles:** a behavior invoking itself needs cycle detection — the same shape of problem we just solved in the module-authoring codegen's recursive-type walk (cycle-safe, a visited set). Worth reusing the mental model.
- **Boundary typing:** how a subgraph's port values (and a `Behavior` target's args/results) are typed across the boundary — ties to the typed-`Value` / structure work (the call boundary already crosses as a generic `Value`).
