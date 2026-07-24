---
"@vizij/runtime": minor
---

Add `Runtime.applyGraphEdits`: apply a spec-level graph diff (`upsert_nodes` / `remove_nodes` / `upsert_edges` / `remove_edges`) to the running graph in place (VIZ-79). An edit patches the graph — unchanged nodes keep their runtime state — instead of reloading the whole spec.
