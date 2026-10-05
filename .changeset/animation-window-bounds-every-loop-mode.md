---
"@vizij/animation": major
---

The play window bounds every loop mode: after `SetWindow`, `Loop` wraps within the window and `PingPong` reflects within it, as `Once` clamps into it; a player with no window plays as before. A player's `length` is the latest end over its instances, whatever its window, and `listPlayers()` reports `ended` while a `Once` player holds at the window bound it plays toward (the start when its speed is negative). `{ PlayAfter: { player, delay } }` starts playback `delay` seconds into the update that applies it, holding the playhead until then. A `Once` player reversed at its end moves back at once, and an instance with a negative `start_offset` starts partway into its clip.
