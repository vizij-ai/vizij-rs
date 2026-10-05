---
"@vizij/animation": major
---

A player's playback state is independent of its speed (vizij-animation-core 2.0.0). `Play`, `Pause` and `Stop` set whether its time advances; `SetSpeed` only sets the multiplier it advances at while playing. A player keeps its speed through `Pause` and `Stop`, so `Play` resumes at the speed it was given; `SetSpeed` neither resumes a paused player nor pauses a playing one (at 0 a playing player holds its time and still reads `Playing`). `listPlayers` reports `state` as the last `Play`, `Pause` or `Stop` left it and `speed` as set, so a paused player's `speed` is its multiplier, not 0. A `Seek` leaves a stopped player `Paused` at the time it seeks to. A new player plays at speed 1.
