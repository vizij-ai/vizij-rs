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
 * prunes simply stops being polled, and a page that stops stepping its
 * device (a hidden tab, a stopped device) stops polling every run. The
 * player stops its audio once it has gone unpolled for longer than its halt
 * bound, by this rule, which a custom {@link Play} hook must keep:
 * - The halt bound is `IDLE_STOP_MS`, or `HALT_POLLS` of the page's poll
 *   interval when the page polls slower than that: a page at a few frames a
 *   second polls hundreds of milliseconds apart, and one missed frame is not
 *   a halt.
 * - The poll interval is measured between two consecutive polls of one
 *   playback, and kept for the page: a playback's wait for its first poll is
 *   judged against the pace the page's earlier playbacks measured. The wait
 *   from `play` to the first poll is no interval of the page's, since `play`
 *   is called between two steps, when the synthesis lands.
 * - Until the page has measured its poll interval, no playback stops
 *   unpolled, so a halt before then lets the audio play out. The pace of a
 *   page that has not yet polled one playback twice is unknown, and any bound
 *   short of it would stop a slow page's playback before its first poll.
 */

/** The halt bound while the page polls at least this often. */
export const IDLE_STOP_MS = 250;

/** How many of the page's poll intervals without a poll mean a halt, where
 * that is longer than `IDLE_STOP_MS`. */
export const HALT_POLLS = 4;

// The page's poll interval in milliseconds, `null` until two polls of one
// playback have measured it: the widest recent gap, forgetting a one-off
// hiccup over the polls that follow.
let pollInterval: number | null = null;

/** The halt bound, `null` while the page's poll interval is unmeasured. */
function haltBound(): number | null {
  return pollInterval === null ? null : Math.max(IDLE_STOP_MS, HALT_POLLS * pollInterval);
}

/** Record a gap between two consecutive polls of one playback. */
function measure(gap: number): void {
  pollInterval =
    pollInterval === null || gap > pollInterval ? gap : 0.9 * pollInterval + 0.1 * gap;
}

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
  // When the playback was last polled; until its first poll, when it started.
  let lastPoll = performance.now();
  let polled = false;

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
    const bound = haltBound();
    if (bound !== null && performance.now() - lastPoll > bound) stop();
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
    const now = performance.now();
    if (polled) measure(now - lastPoll);
    polled = true;
    lastPoll = now;
    if (failure) throw failure;
    if (ended) return null;
    if (startedAt === null) return 0;
    return (ctx.currentTime - startedAt) * 1000;
  };
};
