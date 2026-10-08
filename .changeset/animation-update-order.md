---
"@vizij/animation": patch
---

The changes of an update come in a stable order: per player, in creation order, then each key in the order the player's instances first write it. They came in an order that varied from step to step.
