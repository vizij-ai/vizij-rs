---
"@vizij/runtime": patch
---

A device holds the store changes a page has not drained as the latest value per key, folded on every step: a page that never calls `drainChanges` no longer grows its memory with every step. `drainChanges` answers as before, each changed key's latest value (`null` for a cleared one) and the whole state on the first drain.
