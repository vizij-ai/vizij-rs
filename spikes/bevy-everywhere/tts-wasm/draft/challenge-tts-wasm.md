# Both viseme fragments run on the browser interpreter in headless Chrome and on the native interpreter in `cargo test`; the wasm say branch streams visemes at the page's playhead, once per tick, never after terminal
<!-- tags: family=project; type=reference; project_status=proposal; topics=runtime,speech,tts,visemes,wasm,browser,agent,testing -->
> **Last updated:** 2026-09-18

> **Related documents:** [Central plan](../bevy_everywhere-plan.md) · [speech-and-agent](speech-and-agent.md) · [arora-wasm-constraints](arora-wasm-constraints.md) · [arora-sdk async-functions](https://github.com/semio-ai/arora-sdk/blob/main/docs/async-functions.md)

Compile-and-test spike: `scratchpad/spikes/tts-wasm/` (`src/lib.rs` the provider with
the cfg split and the device composition, `tests/browser.rs` the wasm-bindgen tests,
`tests/native.rs` the native twin, `tests/debug_native.rs` the `behavior_error()`
trace, `www/play.js` the page's playback callback, `webdriver.json` the headless
Chrome binary, `run-{native,browser,device}.sh` the exact commands, `logs/` the raw
outputs). Registry versions are vizij-rs's lock: arora 9.11.1 (arora-engine 4.1.0,
arora-behavior 8.4.0, arora-types 2.5.1), reqwest 0.13.5, tokio 1.53.1, rodio 0.20.1,
wasm-bindgen 0.2.128, wasm-bindgen-test 0.3.78; vizij-rs at `9406f66`, clean.
Toolchain: rustc 1.96.0, wasm-pack 0.15.0, Chrome for Testing 149.0.7827.55
(Playwright `chromium-1228`), ChromeDriver 149.0.7827.22.

## Findings

1. **`reqwest = { version = "0.13", features = ["json"] }` — the crate's declaration
   as it stands — compiles for `wasm32-unknown-unknown`, and `synthesize` ports
   unchanged.** reqwest's manifest puts the native stack (hyper, rustls, tokio,
   h2/h3) under `cfg(not(wasm32))` and pulls js-sys / wasm-bindgen /
   wasm-bindgen-futures / web-sys with only the `fetch` features on wasm
   (`reqwest-0.13.5/Cargo.toml:245-280`); the default features (`default-tls`,
   `http2`, `system-proxy`) are inert there. `Client::new`, `post`, `json(&body)`,
   `send`, `error_for_status`, `bytes`, `json::<T>` all exist on the wasm client.
   `cargo check --target wasm32-unknown-unknown`: clean, 0 warnings, 54 s on the warm
   arora-wasm target dir (`logs/check-wasm32.log`). The fetch itself is not exercised
   from a page (finding 10 scripts synthesis); it is the P3b gate's item.
2. **Only the producer is native-bound, so the split is one module with two bodies.**
   tokio `rt-multi-thread` is the compile error on wasm (`tokio-1.53.1/src/lib.rs:463`),
   rodio's blocking loop and `Instant` are the rest. The spike keeps the contract,
   `synthesize`, the marks→shape mapper (`shape_at`), the run map and the `say` entry
   shared, and gates `platform::{Run, spawn, poll}` per target (`src/lib.rs:241`
   `mod native` under `cfg(not(target_arch = "wasm32"))`, `:346` `mod web` under
   `cfg(target_arch = "wasm32")`). tokio and rodio move under
   `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`; the wasm side adds
   wasm-bindgen, wasm-bindgen-futures, js-sys, serde-wasm-bindgen and nothing else
   (`Cargo.toml`). The native branch builds and tests clean (`logs/check-native.log`,
   4 m 01 s cold; `logs/test-native-5.log`).
3. **The wasm producer is `spawn_local` + a JS callback, and the tick only samples.**
   `spawn` spawns a future that awaits `synthesize`, then calls the page's
   `play(Uint8Array, marks)` and keeps the `playhead()` function it returns in the
   run's `Rc<RefCell<Phase>>`; `poll` calls `playhead()` each tick — a number is the
   playhead in ms (mapped through `shape_at`), `null` ends the run in `Success`, a
   throw ends it in `Failure` (`src/lib.rs:369-437`). The wasm binary carries no Web
   Audio binding: no `AudioContext` web-sys feature, no decode, no scheduling.
4. **The run map lives in the module closure, not in a `static`.** A wasm run holds a
   `js_sys::Function`, which is `!Send`; the product's
   `static RUNS: LazyLock<Mutex<HashMap<u64, Run>>>` (`vizij-arora-tts/src/lib.rs:96`)
   requires `Run: Send` and does not compile on wasm32. A `HostModule` function is
   `Box<dyn FnMut(Call) -> …>` with no `Send` bound (`arora-engine-4.1.0/src/module.rs:37`),
   so a `Provider { config, synth, runs }` captured by the `described_function` closure
   is the map's home on both targets (`src/lib.rs:164-181`). Runs stay content-keyed
   (`utterance_key`, `:513`; the ABI carries no run id) but are per module instance.
5. **`API_URL` becomes a constructor parameter on every target.** `Config { api_base }`
   replaces `std::env::var("API_URL")` (`vizij-arora-tts/src/lib.rs:187`, silently the
   default on wasm); on wasm `Config` also carries `play: js_sys::Function`
   (`src/lib.rs:148-156`). The device composition is the only caller that knows either.
6. **The `AudioContext` unlock is the page's, at the gesture that starts the session.**
   The provider's `say` runs inside device ticks — `requestAnimationFrame` or
   `AroraWeb::run`'s loop — and neither is a user-activation context, so a
   provider-side `resume()` stays `suspended` under the autoplay policy. The page
   creates one context and calls `unlock()` from the click that also requests the
   microphone. `www/play.js` handles a still-suspended context: `playhead()` returns 0
   until `statechange` reports `running`, so the lips wait with the audio (`Running` +
   `sil`), and playback starts on the context's own clock (`ctx.currentTime - startedAt`).
   Not exercised by the tests (the playhead is scripted); the P3b gate's item.
7. **Halting a say run reaches the provider only as silence.** The interpreter's halt
   prunes the fragment, so the provider's `say` stops being invoked; nothing in the
   module ABI tells it. `play.js` turns the missing poll into a stop: no `playhead()`
   call for `IDLE_STOP_MS` = 250 ms stops the source (`www/play.js:23`). Neither the
   spike's native producer (`play` loops until the sink is empty, `src/lib.rs:327`)
   nor the product crate (no idle rule in `vizij-arora-tts/src/lib.rs`) has that rule,
   so a halted native run plays its audio to the end; the same last-poll rule in that
   loop is what makes "interruption is the run's halt" (plan 3.4) true on both targets.
   Not exercised by the tests.
8. **The say fragment asks nothing of a provider beyond the poll-on-tick `Status` and
   the `viseme` out-parameter, once per tick, never after terminal.** `eval_task_run`
   dispatches `call_module_with_outputs`, latches a terminal status and re-emits it
   without calling again (`vizij-graph-core/src/eval/eval_node.rs:394-470`); dispatch
   is `CallBridgeFunctions::dispatch` → `arora_call`, the engine's synchronous
   in-process path (`vizij-arora-behavior/src/lib.rs:62-90`), the same on wasm.
   `device.rs:1222` `a_say_run_streams_the_provider_s_visemes_to_the_lips` is that
   contract as a test: a scripted provider, none called after `Success`.
9. **Both fragments run on the native interpreter in a plain `cargo test`, over the
   store and the interpreter with no HAL** (`logs/test-native-5.log`, 3 s warm,
   `tests/native.rs`):
   - `play_viseme_runs_on_the_native_interpreter`: ok — `aa` > 0.8 and
     `standard/vizij/viseme = "aa"` with `Running` after 0.2 s; `aa` < 0.1, `sil` and
     `Success` after 0.65 s; the twin of `device.rs:1152`
     `a_play_viseme_run_plays_the_shape_through_its_envelope`.
   - `say_ends_with_the_native_producer_s_status`: ok — the say fragment over the
     native producer against a closed local port (`http://127.0.0.1:1`): `Running` and
     `sil` on tick 1, `Failure` after 4 ticks (83 ms), `aa` < 0.01 throughout. The
     taskrun node, SPAWN through the interpreter module, the tokio task and the
     no-op-waker `JoinHandle` poll are exercised without the cloud or an audio device.
     The native test file: 2 passed, 0.37 s.
   - The product's own `device.rs` tests (`a_play_viseme_run_plays_the_shape_through_its_envelope`,
     `a_new_play_viseme_run_takes_over_and_crossfades`,
     `a_say_run_streams_the_provider_s_visemes_to_the_lips`), over the ros4hri
     device with a `RigHal`: @@DEVICE@@ (`logs/test-vizij-device-native-3.log`);
     the cold build of the `vizij` binary crate (Bevy, wasmtime) is 26 m 04 s on a
     machine at load 250 (`logs/test-vizij-device-native-2.log`).
10. **Both fragments run on the browser interpreter, in headless Chrome.**
    `wasm-pack test --headless --chrome --chromedriver … -- --test browser`: 2 passed,
    0 failed, 0.74 s in the browser, 16 s wall on a warm target
    (`logs/wasm-pack-test-chrome-4.log`); the same with a per-tick trace: @@TRACE_SUMMARY@@
    (`logs/wasm-pack-test-chrome-5.log`).
    - `play_viseme_runs_on_the_browser_interpreter`: the native assertions (aa > 0.8
      after 0.2 s; `sil` and `Success` after 0.65 s) hold on wasm32; SPAWN goes through
      `interpreter_module::encode_spawn` → `arora.call` → the interpreter module, the
      taskrun node dispatches the viseme module in-process.
    - `say_streams_the_visemes_at_the_page_s_playhead`: `say("hello")` on the wasm
      provider with a scripted synthesizer awaited through `spawn_local` and a JS
      `play` whose playhead advances 40 ms per poll and returns `null` past 260 ms
      (`tests/browser.rs:178-189`). The trace (`step_traced`, `tests/browser.rs:56`):
      @@TRACE_LINES@@
      Tick 1 reports `Running` with no JS call yet; the audio bytes (4) and marks (3)
      reach `play` after one microtask yield; the playhead is polled exactly once per
      tick while playing (8 polls for 0..=280 ms in 40 ms steps) and not once during the
      19 ticks after the terminal status.
11. **The alternative — `say` native-only, a JS lipsync producer driving
    `play_viseme(shape, weight)` — costs a spawn per mark and keeps the JS producer
    alive.** `play_viseme` is an exclusive 480 ms envelope (attack 80 / hold 250 /
    release 150 ms, `vizij-arora-host/src/skills.rs:108-110`; `.exclusive()` at
    `vizij-arora-behavior/src/viseme.rs:42`) spawned through `call()` → SPAWN as JSON;
    Polly marks arrive every ~50–120 ms, so each mark preempts the previous run rather
    than the say driver's continuous stream, and one JSON round trip per mark rides
    the device's step. It keeps `useSpeechPlayback` + `visemeMapping` + `poseUtils` +
    `pollyApi` (974 lines) or the tutorial's aligner path (`useVisemeMouth` +
    `audioManager`, 1,132 lines) as the lipsync owner, and `speaking` stays a JS fact
    instead of a run status. Findings 9 and 10 remove its one argument: the same
    `say` fragment runs on the browser interpreter.
12. **An unsatisfied `input` in the base graph stops every fragment, silently.** An
    input node with neither a staged store value nor a `value` default fails the whole
    evaluation (`vizij-graph-core/src/eval/eval_node.rs:2387`); `Arora::step` returns
    `Ok` and the message is only in `behavior_error()`, while no fragment output is
    written (`logs/test-native-5.log`, the `debug_native` trace: three ticks, status
    and viseme keys `None`). Both test twins write `sensor/x` before the first tick
    (`stage_base_input`, `tests/native.rs:48`, `tests/browser.rs:55`); the product's
    device composes over the ros4hri mapping, whose inputs are shaped. On the browser
    device, a composed base graph with an unstaged input is a `say` that moves nothing
    and reports nothing.
13. **wasm-pack 0.15 fetches the chromedriver of Chrome for Testing's Stable channel,
    not of the installed Chrome.** With Chrome 152.0.7977.64 installed and Stable at
    153.0.8010.52 the session is refused ("This version of ChromeDriver only supports
    Chrome version 153", `logs/wasm-pack-test-chrome-2.log:324-346`). The run that
    passes names both halves: `--chromedriver` pointing at a cached driver
    (`~/Library/Caches/.wasm-pack/chromedriver-f07dc4f5987c4c26`, 149.0.7827.22) and
    `webdriver.json` in the crate dir setting `goog:chromeOptions.binary` to
    Playwright's Chrome for Testing 149.0.7827.55; the runner reads that file
    (`WASM_BINDGEN_TEST_WEBDRIVER_JSON`, `wasm-bindgen-cli-0.2.128/src/wasm_bindgen_test_runner/headless.rs:142`)
    and merges its own `headless` / `no-sandbox` args. wasm-pack installs the
    wasm-bindgen CLI matching the lock by `cargo install` (0.2.128, 4 m 45 s release
    build, no prebuilt download; `logs/wasm-pack-test-chrome-2.log:132-318`). Neither
    `CHROMEDRIVER` nor the Playwright MCP is needed; CI needs a Chrome and a
    chromedriver of one version, installed together.

## Questions answered

**The cfg split, and where the unlock happens.** Shared: the contract, `polly_shape`,
`SpeechMark`, `synthesize`, `shape_at`, `Config`, the `Provider` with its run map,
`say`'s argument parsing, `compose()`. Native: `Run { JoinHandle<Value>, Arc<Mutex<&str>> }`,
a tokio task for the fetch, `spawn_blocking` for rodio, the viseme cell at
`Sink::get_pos()` (rodio 0.20.1 `sink.rs:351`), `Future::poll` with `Waker::noop()`.
wasm32: `Run { Rc<RefCell<Phase>> }`, `Fetching | Playing { playhead, marks, next,
current } | Ended(Value)`, a `spawn_local` future, `playhead.call0()` per tick. The
unlock is the page's, in the gesture handler that opens the session; every Web Audio
concern stays in `play.js` (findings 2–7).

**What compiles, what needed cfg.** wasm32: clean (finding 1). Native: clean
(finding 2). The cfg needed: the two `[target]` dependency sections, `mod native` /
`mod web`, the `play` field of `Config`, and a scripted-synthesizer seam that is
wasm-only (an `Rc` future cannot cross onto the tokio runtime; natively the scripted
seam is refused with a log line and the fetch runs). `compose()` registers the say and
play_viseme fragments, routes `SAY_ID` to the provider's module id, and mounts the
viseme and tts host modules — the browser twin of `device.rs:345-400`
(`src/lib.rs:445-465`); it builds on both targets with `arora = { default-features = false }`.

**Do the say / play_viseme fragments run on the browser interpreter?** Yes, measured:
both are spawned through `interpreter_module::encode_spawn` (SPAWN) into the
interpreter module, the taskrun node dispatches the viseme module and the tts module
in-process, and the runs reach `Success` with the same envelope as natively
(finding 10). HALT is the same module's other call and is not exercised (finding 7).

**Does the wasm say branch stream visemes at the page playhead, once per tick, never
after terminal?** Yes, measured: the viseme follows the marks at the scripted playhead
(`PP` at 0–40 ms, `aa` past the 100 ms mark, `sil` at 200 ms), `playhead()` is called
exactly once per tick while `Running` (8 polls) and zero times during the 19 ticks
after `Success` (finding 10's trace). What the tests script instead of running: the
`fetch` of `get-audio` / `get-visemes`, the mp3 decode, the `AudioContext` unlock, the
idle-stop on halt.

**Which is simpler, which honors "the device keeps say".** For a one-page demo the JS
producer is fewer new Rust lines (zero) but not less work: the pose-id viseme map and
the Polly cursor stay, the `speaking` state stays JS, and every mark is a SPAWN. The
wasm branch is 93 lines of Rust behind cfg plus an 84-line `play.js`, and it makes the
browser demo run the same `say` the native app runs — acceptance criterion 5 ("lips
driven by the viseme players from the same fragment") is only met by it. The JS
producer honors "the device keeps play_viseme", not "the device keeps say".

**Is P3b de-risked?** The interpreter half is: the say and play_viseme fragments run on
the browser interpreter in Chrome through the same SPAWN / taskrun / dispatch path as
natively, the wasm producer's `spawn_local` future advances between ticks, and the
page's playhead drives the lips once per tick with no poll after the terminal status
(findings 9, 10). What remains is page-side and belongs to the P3b gate's Playwright
spec: the real fetch of `get-audio` / `get-visemes` from a page (CORS, the mp3 decode)
over reqwest's `fetch` backend; the `AudioContext` unlock from a gesture and the
suspended-context wait (`play.js` handles it, untested); halt-by-silence (present in
`play.js`, absent from both native producers, untested on either); and the
content-keyed run collision when an utterance is repeated while it plays — an arora
ABI limit, not a P3b item.

## Evidence

### Commands and results

| Check | Command (in the spike dir, `PATH` with `~/.cargo/bin`) | Result |
|---|---|---|
| wasm32 provider + composition | `CARGO_TARGET_DIR=…/arora-wasm/target cargo check --target wasm32-unknown-unknown` | clean, 0 warnings, 54 s (`logs/check-wasm32.log`) |
| native provider + tests build | `CARGO_TARGET_DIR=…/arora-wasm/target-host cargo check --tests` | clean, 4 m 01 s cold (`logs/check-native.log`) |
| native tests, all targets (`run-native.sh`) | `CARGO_TARGET_DIR=…/arora-wasm/target-host cargo test -- --nocapture` | 3 s warm (build 1.24 s): lib 1 passed; `native` 2 passed, 0.37 s (say → `Failure` after 4 ticks / 83 ms); `debug_native` 1 passed, 0.14 s (`logs/test-native-5.log`) |
| product device.rs tests (`run-device.sh`) | `cd vizij-rs && CARGO_TARGET_DIR=…/tts-wasm/target cargo test --locked -p vizij --bin vizij -- device::tests::a_play_viseme_run… a_new_play_viseme_run… a_say_run…` | @@DEVICE_TABLE@@ (`logs/test-vizij-device-native-3.log`); cold build 26 m 04 s, 3 passed, 3.63 s (`logs/test-vizij-device-native-2.log`) |
| browser tests, wasm-pack's own driver | `CARGO_TARGET_DIR=…/arora-wasm/target wasm-pack test --headless --chrome -- --test browser` | 429 s: wasm32 test build 1 m 31 s, wasm-bindgen CLI 0.2.128 install 4 m 45 s, chromedriver 153 fetched, session refused against Chrome 152 (`logs/wasm-pack-test-chrome-2.log`) |
| browser tests (`run-browser.sh`) | `CARGO_TARGET_DIR=…/arora-wasm/target wasm-pack test --headless --chrome --chromedriver ~/Library/Caches/.wasm-pack/chromedriver-f07dc4f5987c4c26/chromedriver -- --test browser -- --nocapture` with `webdriver.json` | 16 s warm: 2 passed, 0 failed, 0.74 s (`logs/wasm-pack-test-chrome-4.log`); with the per-tick trace, @@TRACE_TABLE@@ (`logs/wasm-pack-test-chrome-5.log`); 76 s after a source change (`logs/wasm-pack-test-chrome-3.log`) |
| fragment unit tests (prior study) | `cargo test -p vizij-arora-behavior -p vizij-arora-host --lib` | 4 + 5 pass (`spikes/speech-and-agent/test-behavior-host.log`) |

The failing shape of the native twin without the staged input is in `logs/test-native.log`
and `logs/test-native-2.log` (`standard/vizij/viseme/aa: not a float: None`, with the
`debug_native` trace of `behavior_error()`).

### The JS playback contract (`www/play.js`)

`createPlayback()` → `{ play, unlock, context }`. `play(bytes: Uint8Array, marks:
{time, value}[]) → playhead: () => number | null`: `0` while decoding or while the
context is suspended, ms since `source.start()` afterwards, `null` once `onended` or
stopped; a decode failure is rethrown from `playhead()`. A 250 ms poll gap stops the
source. The provider passes the marks as plain objects (`serde_wasm_bindgen`); the
page never needs them except to display. The browser test's `play` is a scripted
twin of this contract (`tests/browser.rs:178-189`).

### Files read

- vizij-rs `9406f66`: `crates/interop/vizij-arora-{tts,behavior,host,web}/**`,
  `crates/vizij/src/{device.rs,viseme.rs}`,
  `crates/node-graph/vizij-graph-core/src/eval/{eval_node.rs,node_function.rs}`,
  `Cargo.lock`, `.github/workflows/ci.yml`.
- arora-sdk `docs/async-functions.md`; registry `arora-9.11.1`,
  `arora-engine-4.1.0/src/module.rs`, `arora-behavior-8.4.0/src/interpreter_module.rs`,
  `arora-web-6.1.0/src/lib.rs`, `reqwest-0.13.5`, `tokio-1.53.1/src/lib.rs`,
  `rodio-0.20.1/src/sink.rs`, `wasm-bindgen-cli-0.2.128/src/wasm_bindgen_test_runner/`.
- vizij-web (line counts only): `packages/@vizij/speech-react/src/**`,
  `apps/tutorial-agent-face/src/{hooks/useVisemeMouth.ts,utils/audioManager.ts}`.

## Implications for the target picture

- **Section 3.4 holds, with the callback returning a playhead function.** The provider
  hands the page `{audioBytes, marks}` and receives `playhead()`; it never reads
  `AudioContext.currentTime` itself. The page's clock is the viseme clock, read through
  the function, once per tick. `Config { api_base, play }` is the constructor the
  section calls "API_URL becomes a constructor parameter".
- **One crate, two producers, the run map in the module closure.** The branch belongs
  in `vizij-arora-tts` under `[target]` sections: contract, `synthesize`, mapper and
  run map are shared; the `static RUNS` goes, each mounted provider owns its runs, and
  the native clock moves to `Sink::get_pos()` in the same pass.
- **The browser device's composition is `compose()`**: `set_function_modules(SAY_ID →
  tts module)`, `set_task_fragment` for say and play_viseme with the face's rig prefix,
  `with_host_module` for the viseme and tts modules — what `vizij-arora-web::start`
  lacks today (it sets function modules only, `lib.rs:106`) and what `vizij::device`
  unifies with `device.rs:345-400`. `start()` gains the page's `play` callback and the
  rig prefix.
- **Section 3.4's "the fragments' `taskrun` node on the browser interpreter runs
  through the same synchronous in-process dispatch as natively" is measured**
  (finding 10); its "the headless-Chrome run of that test is not recorded" is retired.
- **A halt must stop the audio on both targets.** A last-poll timestamp in each
  producer (250 ms idle → stop) is the rule until the ABI carries a halt; the spike
  has it in `play.js` and not in the native loop; without it the agent's interruption
  stops the lips and not the voice.
- **The composed base graph must stage its inputs.** A device whose base graph reads
  a store key nobody writes runs no fragment (finding 12); the browser device's
  `loadFace` writes the face's input defaults, or the graph declares them, before the
  first `say`.
- **The demo runs the device's `say`.** The JS lipsync producer is retired as the
  plan proposes; the audio-agent shape (`tutorial-agent-face`) keeps its aligner
  because it has no text to hand to `say`, and stays the second shape, not the demo.

## Implications for the plan

1. **P3b's interpreter risk is retired** (findings 9, 10): the cfg split (93 Rust
   lines in `vizij-arora-tts`), `play.js` (84 lines) shipped with `@vizij/runtime`,
   the registration in `start()`. Of section 5's open technical questions, "the
   `taskrun` fragments on the browser interpreter in headless Chrome" and
   "`spawn_local` in the device's tick" are answered here; "reqwest's wasm client in
   the device's tick" is compile-verified only (finding 1) and "the `AudioContext`
   user-gesture unlock" is designed, not run (finding 6): both are the P3b gate's
   Playwright spec.
2. **Add the two wasm-bindgen tests to `vizij-arora-tts`** (`tests/browser.rs` is the
   draft; `step_traced` stays as the evidence printer) and run them in CI with
   `wasm-pack test --headless --chrome`; ci.yml runs no browser test today, and
   `vizij-arora-web/tests/proof.rs` is likewise unrun. CI needs Chrome and
   chromedriver of one version installed together (wasm-pack's own fetch follows the
   Stable channel, finding 13), and the wasm-bindgen CLI pinned to the lock (0.2.128;
   a `cargo install` unless a prebuilt is cached).
3. **Write the idle-stop rule into both producers** with the branch (the native loop
   at `src/lib.rs:327` and the product crate have none), and state in `docs/skills.md`
   that a halted say run goes silent within 250 ms.
4. **The publish prerequisite stands**: the crate chain (tts 2.0.0, behavior 1.1.0,
   host 2.1.0) is on main and unpublished (S:speech-and-agent 24); the wasm branch is
   a MINOR of `vizij-arora-tts` on top of it.
5. **`@vizij/runtime` 3.0's `start()` signature carries the playback callback**;
   document `createPlayback()` and the unlock rule on the guidebook's integrate page.
6. **The P3b gate's Playwright spec** runs Chrome with
   `--autoplay-policy=no-user-gesture-required`, calls `say` against the cloud
   function, and asserts `standard/vizij/viseme/<shape>` changes over time and that
   the hook received bytes and marks — the three scripted seams of finding 10 made
   real, plus a halt mid-utterance that goes silent within 250 ms.

## Open questions

- **Real audio, real fetch, headless.** The tests script the synthesizer and the
  playhead; the P3b gate's Playwright spec closes the gap (decode of the mp3,
  `onended`, CORS of `get-audio`/`get-visemes` from a page — the web demo already
  calls both).
- **Run identity.** Content-keyed runs collide when an agent repeats itself: two
  fragments poll one `Run`, the first to observe the end removes it and the other
  re-spawns the utterance (S:speech-and-agent open questions); unchanged by the branch,
  and an arora-sdk ABI question.
- **Halt in the ABI.** Whether arora's module ABI gains a halt/abandon notification,
  which would replace the idle-stop heuristic on both targets.
- **Decode latency accounting.** `decodeAudioData` of a whole mp3 precedes the first
  audible sample; the run reports `Running`/`sil` meanwhile, so lips and audio stay
  aligned, but the agent sees a longer "speaking" than the audio by that much.
- **Where `play.js` ships.** With `@vizij/runtime` (proposed) or inside the demo app;
  the former makes `say` work for every consumer of the browser device.
- **Base-graph input staging on the browser device.** Which layer guarantees every
  `input` of a composed face graph is staged before the first tick (finding 12): the
  exporter's defaults, `loadFace`, or a graph-level rule that unstaged inputs of
  declared shape evaluate to their null.
