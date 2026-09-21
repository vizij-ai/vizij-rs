---
"@vizij/runtime": minor
---

The face reports what it is saying: while a `say` run's audio plays, the utterance is the face's speech state, `standard/vizij/speech` (empty before playback starts and once it ends), readable like any other key. The browser provider counts the audio as playing once the page's playback hook reports a playhead past 0.
