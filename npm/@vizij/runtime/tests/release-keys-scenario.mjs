// `releaseKeys` settles on a device nothing steps: the release asks for a
// step with no time in it, and in that step the released key takes its
// writer's value. A page that steps in the same turn applies it with its own
// step. Under `run()` the run's next step applies it, and the page adds no
// step.
//
// One scenario for both suites: `tests/release-keys.mjs` runs it in Node,
// `tests/browser/release-keys.mjs` in a page. It throws on the first failure.

/** Run the scenario over `vizij`, the package's module (its `dist/` entry). */
export async function releaseKeysSettles(vizij) {
  const check = (ok, message) => {
    if (!ok) throw new Error(message);
  };
  const same = (actual, expected, message) =>
    check(
      JSON.stringify(actual) === JSON.stringify(expected),
      `${message}: ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`,
    );
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  /** Let the microtasks queued so far run, and those they queue. */
  const microtasks = async () => {
    for (let i = 0; i < 4; i++) await Promise.resolve();
  };

  // The graph writes 0.5 to `out/held` at every step.
  const device = await vizij.startRuntime({
    nodes: [
      { id: "k", type: "constant", params: { value: { f32: 0.5 } } },
      { id: "out", type: "output", params: { path: "out/held" } },
    ],
    edges: [{ from: { node_id: "k" }, to: { node_id: "out", input: "in" } }],
  });
  const read = () => device.readValues(["out/held"])["out/held"];
  /** Hold `out/held` at 0.25, the page's value, through a step. */
  const holdAtPageValue = async () => {
    device.holdKeys(["out/held"]);
    device.setValue("out/held", { f32: 0.25 });
    await microtasks();
    same(read(), { f32: 0.25 }, "a held key keeps the page's value");
  };
  try {
    // Nothing steps the device: the release takes a step at a zero dt, in
    // which the graph writes the key again.
    await holdAtPageValue();
    device.releaseKeys(["out/held"]);
    await microtasks();
    same(read(), { f32: 0.5 }, "released on a device nothing steps, the graph writes the key");
    same(device.readValues(["arora/time"])["arora/time"], { u64: 0 }, "with no time elapsed");

    // `releaseKeys()` releases every hold the same way.
    await holdAtPageValue();
    device.releaseKeys();
    await microtasks();
    same(read(), { f32: 0.5 }, "releaseKeys() on a device nothing steps");

    // A page that steps in the same turn applies the release with its step.
    await holdAtPageValue();
    device.releaseKeys();
    device.step(16);
    same(read(), { f32: 0.5 }, "the page's step in the same turn applies the release");
    same(device.readValues(["arora/dt"])["arora/dt"], { u64: 16_000_000 }, "the page's step's dt");
    await microtasks();
    same(device.readValues(["arora/dt"])["arora/dt"], { u64: 16_000_000 }, "and no step at a zero dt follows");

    // `run()` steps the device: the release holds until the run's next step,
    // which waits on a timer no microtask lets through.
    const period = 100;
    await holdAtPageValue();
    const running = device.run(period);
    await sleep(period * 2);
    same(read(), { f32: 0.25 }, "under run(), the held key keeps the page's value");
    device.releaseKeys();
    await microtasks();
    same(read(), { f32: 0.25 }, "under run(), no step of the page's applies the release");
    for (let waited = 0; waited < period * 10 && read()?.f32 !== 0.5; waited += 10) await sleep(10);
    same(read(), { f32: 0.5 }, "the run's next step applies the release");
    check(device.readValues(["arora/dt"])["arora/dt"]?.u64 > 0, "the step that applied it was the run's");
    device.stop();
    await running;
  } finally {
    device.dispose();
  }
}
