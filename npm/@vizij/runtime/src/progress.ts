/**
 * How far {@link init} has got loading the wasm module, and the listeners
 * that follow it.
 */

/**
 * The stage {@link init} is at. `download` counts the bytes of the wasm
 * received: `total` is the response's `Content-Length`, `null` when the
 * length is unknown or names the compressed size. `compile` lasts from the
 * last byte while the module compiles and instantiates; `start` is reported
 * once wasm-bindgen's init has the instance and has run the module's own
 * start, and `ready` once `init()` has resolved. A visible page paints a
 * frame between each of the last three. The module's start sets up no App:
 * the page's App is built by `mount()`, after `ready`.
 */
export type InitProgress =
  | { stage: "download"; loaded: number; total: number | null }
  | { stage: "compile" }
  | { stage: "start" }
  | { stage: "ready" };

/** The shortest interval between two `download` reports, in milliseconds. */
const DOWNLOAD_INTERVAL_MS = 50;

let current: InitProgress = { stage: "download", loaded: 0, total: null };
const listeners = new Set<(progress: InitProgress) => void>();

/**
 * Calls `listener` with the current stage at once, then with each change;
 * returns the unsubscribe. Before {@link init} is called the current stage
 * is a download with nothing received yet. `download` is reported at most
 * every 50 ms while bytes arrive, and once more with the final count.
 */
export function onInitProgress(listener: (progress: InitProgress) => void): () => void {
  listeners.add(listener);
  call(listener, current);
  return () => {
    listeners.delete(listener);
  };
}

const ORDER: Record<InitProgress["stage"], number> = { download: 0, compile: 1, start: 2, ready: 3 };

/** Make `progress` the current stage and tell every listener. The stages
 * only move forward: a report behind the current one is dropped. */
export function reportProgress(progress: InitProgress): void {
  if (ORDER[progress.stage] < ORDER[current.stage]) return;
  current = progress;
  for (const listener of [...listeners]) call(listener, progress);
}

/** A listener that throws must not fail the load: its error is rethrown
 * on its own task, where the page's error handling sees it. */
function call(listener: (progress: InitProgress) => void, progress: InitProgress): void {
  try {
    listener(progress);
  } catch (err) {
    setTimeout(() => {
      throw err;
    });
  }
}

function now(): number {
  return typeof performance !== "undefined" ? performance.now() : Date.now();
}

/** The byte count a response announces for its body: `Content-Length`,
 * unless a `Content-Encoding` says it counts the compressed bytes. */
function declaredLength(response: Response): number | null {
  const encoding = response.headers.get("Content-Encoding");
  if (encoding && encoding.toLowerCase() !== "identity") return null;
  const length = Number(response.headers.get("Content-Length") ?? NaN);
  return Number.isFinite(length) && length > 0 ? length : null;
}

/**
 * Report the download of `response`'s body while it is compiled from it.
 *
 * The bytes are counted on a clone — a tee of the body — and `response`
 * itself is what reaches `WebAssembly.instantiateStreaming`: the module
 * still compiles as it streams in, the browser can reuse its compiled code
 * on the next load (Chrome keys that cache on the fetched response, and a
 * `Response` built in script has none), and a host serving the wrong MIME
 * type still reaches wasm-bindgen's fallback. `counted` resolves once the
 * final count and `compile` are reported, or once the download has failed —
 * the compile reading the same body then rejects with the error.
 */
export function countDownload(response: Response): { response: Response; counted: Promise<void> } {
  if (!response.ok) {
    throw new Error(
      `@vizij/runtime: fetching the wasm module failed: ${response.status} ${response.statusText} ${response.url}`.trim(),
    );
  }
  let total = declaredLength(response);
  reportProgress({ stage: "download", loaded: 0, total });
  const copy = response.clone();
  if (!copy.body) {
    reportProgress({ stage: "compile" });
    return { response, counted: Promise.resolve() };
  }
  const reader = copy.body.getReader();
  const counted = (async () => {
    let loaded = 0;
    let reported = 0;
    let last = now();
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        loaded += value.byteLength;
        // A body longer than its declared length was declared compressed.
        if (total !== null && loaded > total) total = null;
        const t = now();
        if (t - last >= DOWNLOAD_INTERVAL_MS) {
          last = t;
          reported = loaded;
          reportProgress({ stage: "download", loaded, total });
        }
      }
    } catch {
      return;
    }
    if (reported !== loaded) reportProgress({ stage: "download", loaded, total });
    reportProgress({ stage: "compile" });
  })();
  return { response, counted };
}

/**
 * Resolves once the page has painted a frame, so what a listener drew for
 * the stage reported before it is on screen: the task after an animation
 * frame runs after that frame's paint. A hidden page paints no frame, and
 * a worker or Node has none: they get the next task, and a page hidden
 * while it waits stops waiting.
 *
 * The compile and the instantiation after the last byte take well under a
 * millisecond and leave the main thread free — V8 compiles a function when
 * it is first called, in `mount()` and the frames after — so without this
 * wait no frame falls between `compile`, `start` and `ready`.
 */
export function nextPaint(): Promise<void> {
  return new Promise((resolve) => {
    const visible =
      typeof document !== "undefined" &&
      document.visibilityState === "visible" &&
      typeof requestAnimationFrame === "function";
    if (!visible) {
      setTimeout(resolve, 0);
      return;
    }
    const hidden = () => {
      if (document.visibilityState !== "visible") done();
    };
    const done = () => {
      document.removeEventListener("visibilitychange", hidden);
      resolve();
    };
    document.addEventListener("visibilitychange", hidden);
    requestAnimationFrame(() => setTimeout(done, 0));
  });
}
