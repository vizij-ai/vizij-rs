// The page's playback for the wasm `say` provider.
//
// The provider hands `play(audioBytes, marks)` the mp3 bytes and the Polly
// speech marks and keeps the `playhead()` it returns; it calls `playhead()`
// once per device tick and maps the marks to the current viseme shape itself.
//
// Contract of `playhead()`:
//   - a number: milliseconds since the audio started (0 while decoding or
//     while the AudioContext is still suspended — the lips wait with the
//     audio);
//   - `null`: playback has ended (or was stopped) — the run succeeds;
//   - a throw: playback failed — the run fails.
//
// The AudioContext is created here and unlocked by `unlock()`, which the page
// calls from a user-gesture handler (autoplay policy): the same click that
// asks for the microphone in the agent demo. The provider never sees the
// context; ticks run from requestAnimationFrame or the device's run loop,
// which are not gesture contexts, so a provider-side unlock could not work.
//
// The module ABI has no halt: a run the interpreter prunes simply stops
// being polled. `IDLE_STOP_MS` without a poll stops the audio, so halting
// the say run (the agent's interruption) silences it within that bound.
export const IDLE_STOP_MS = 250;

export function createPlayback() {
  const ctx = new AudioContext();

  /** Call from a user gesture before the first `say`. */
  const unlock = () => ctx.resume();

  const play = (bytes, marks) => {
    let source = null;
    let startedAt = null;
    let ended = false;
    let failure = null;
    let lastPoll = performance.now();

    const stop = () => {
      ended = true;
      clearInterval(watchdog);
      if (source) {
        try { source.stop(); } catch (_) { /* already stopped */ }
      }
    };
    const watchdog = setInterval(() => {
      if (performance.now() - lastPoll > IDLE_STOP_MS) stop();
    }, IDLE_STOP_MS / 2);

    const start = () => {
      if (ended) return;
      startedAt = ctx.currentTime;
      source.start();
    };
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    ctx.decodeAudioData(buffer).then((audio) => {
      if (ended) return;
      source = ctx.createBufferSource();
      source.buffer = audio;
      source.connect(ctx.destination);
      source.onended = () => { ended = true; clearInterval(watchdog); };
      if (ctx.state === "running") {
        start();
      } else {
        const onState = () => {
          if (ctx.state === "running") {
            ctx.removeEventListener("statechange", onState);
            start();
          }
        };
        ctx.addEventListener("statechange", onState);
      }
    }).catch((e) => { failure = e; stop(); });

    return function playhead() {
      lastPoll = performance.now();
      if (failure) throw failure;
      if (ended) return null;
      if (startedAt === null) return 0;
      return (ctx.currentTime - startedAt) * 1000;
    };
  };

  return { play, unlock, context: ctx };
}
