---
"@vizij/runtime": patch
---

The page's first `say` plays on a page that steps its device slowly. The page's player stops a playback that goes unpolled for longer than its halt bound, `IDLE_STOP_MS` (250 ms) or four of the page's poll intervals when the page polls slower than that, and it now measures that interval only between two polls of one playback. Until the page has measured it, no playback stops unpolled. On a page stepping about once a second, as under a software GPU, the first `say` was stopped before its first poll, so its run ended at once and no viseme played; it now plays through, and a later playback whose device stops stepping still stops within four of the page's poll intervals.
