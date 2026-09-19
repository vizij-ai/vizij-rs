/**
 * The page's playback for the browser `say` provider.
 *
 * The provider hands `play(audioBytes, marks)` the mp3 bytes and the speech
 * marks and keeps the `playhead()` it returns; it calls `playhead()` once per
 * device tick and maps the marks to the current viseme shape itself.
 *
 * Contract of `playhead()`:
 * - a number: milliseconds since the audio started (0 while decoding or while
 *   the `AudioContext` is still suspended — the lips wait with the audio);
 * - `null`: playback has ended, or was stopped — the run succeeds;
 * - a throw: playback failed — the run fails.
 *
 * The `AudioContext` is the page's, created once and unlocked by
 * {@link unlockAudio} from a user-gesture handler (the autoplay policy): the
 * same click that asks for the microphone in a conversation. The provider
 * never sees the context; its ticks run from `requestAnimationFrame` or the
 * device's run loop, which are no gesture contexts.
 *
 * A halt is silence: the module ABI has no halt, so a run the interpreter
 * prunes simply stops being polled. `IDLE_STOP_MS` without a poll stops the
 * audio, so halting a `say` run (a conversation's interruption) silences it
 * within that bound. A custom {@link Play} hook must keep the same rule.
 */

/** How long playback goes on unpolled before it stops itself. */
export const IDLE_STOP_MS = 250;

/** A speech mark, as the provider hands them to the hook. */
export interface SpeechMark {
  /** Milliseconds into the audio. */
  time: number;
  /** The viseme code (AWS Polly's set). */
  value: string;
}

/** The playback hook: start the audio, return its playhead. */
export type Play = (audioBytes: Uint8Array, marks: SpeechMark[]) => () => number | null;

let context: AudioContext | null = null;

function audioContext(): AudioContext {
  if (!context) {
    context = new AudioContext();
  }
  return context;
}

/**
 * Resume the page's `AudioContext`. Call it from a user-gesture handler
 * before the first `say` — the autoplay policy keeps a context suspended
 * until then, and a `say` spoken into a suspended context waits, lips and
 * all, until it resumes.
 */
export function unlockAudio(): Promise<void> {
  return audioContext().resume();
}

/** The default {@link Play}: Web Audio over the page's context. */
export const play: Play = (bytes, marks) => {
  void marks;
  const ctx = audioContext();
  let source: AudioBufferSourceNode | null = null;
  let startedAt: number | null = null;
  let ended = false;
  let failure: unknown = null;
  let lastPoll = performance.now();

  const stop = () => {
    ended = true;
    clearInterval(watchdog);
    if (source) {
      try {
        source.stop();
      } catch {
        // already stopped
      }
    }
  };
  const watchdog = setInterval(() => {
    if (performance.now() - lastPoll > IDLE_STOP_MS) stop();
  }, IDLE_STOP_MS / 2);

  const start = () => {
    if (ended || !source) return;
    startedAt = ctx.currentTime;
    source.start();
  };
  const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  ctx
    .decodeAudioData(buffer as ArrayBuffer)
    .then((audio) => {
      if (ended) return;
      source = ctx.createBufferSource();
      source.buffer = audio;
      source.connect(ctx.destination);
      source.onended = () => {
        ended = true;
        clearInterval(watchdog);
      };
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
    })
    .catch((e: unknown) => {
      failure = e;
      stop();
    });

  return function playhead() {
    lastPoll = performance.now();
    if (failure) throw failure;
    if (ended) return null;
    if (startedAt === null) return 0;
    return (ctx.currentTime - startedAt) * 1000;
  };
};
