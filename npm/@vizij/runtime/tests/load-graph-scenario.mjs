// `loadGraph` and `applyGraphEdits` settle on a device nothing steps: a step
// with no time in it applies each, and the graph it leaves runs in that same
// step. Under `run()` the run's next step applies them, and the page adds no
// step. The swap keeps what is not the graph: a run runs on, and a held key
// keeps the page's value. A graph that would fail every evaluation is
// refused, and what runs goes on.
//
// One scenario for both suites: `tests/load-graph.mjs` runs it in Node,
// `tests/browser/load-graph.mjs` in a page. It throws on the first failure.

/** Run the scenario over `vizij`, the package's module (its `dist/` entry). */
export async function loadGraphSettles(vizij) {
  const check = (ok, message) => {
    if (!ok) throw new Error(message);
  };
  const same = (actual, expected, message) =>
    check(
      JSON.stringify(actual) === JSON.stringify(expected),
      `${message}: ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}`,
    );
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  // An operation a step at a zero dt applies settles in a microtask or two;
  // one still pending after `ms` waits for a step nobody takes.
  const within = (pending, what, ms = 2_000) =>
    Promise.race([
      pending,
      sleep(ms).then(() => {
        throw new Error(`${what} did not settle within ${ms} ms`);
      }),
    ]);
  /** A graph writing `value` to each of `paths`, its nodes named by `tag`. */
  const writes = (tag, value, ...paths) => ({
    nodes: [
      { id: `${tag}_k`, type: "constant", params: { value: { f32: value } } },
      ...paths.map((path, i) => ({ id: `${tag}_out${i}`, type: "output", params: { path } })),
    ],
    edges: paths.map((_, i) => ({
      from: { node_id: `${tag}_k` },
      to: { node_id: `${tag}_out${i}`, input: "in" },
    })),
  });
  /** The edits adding `writes(tag, value, path)` to the running graph. */
  const adds = (tag, value, path) => {
    const graph = writes(tag, value, path);
    return { upsert_nodes: graph.nodes, upsert_edges: graph.edges };
  };

  const device = await vizij.startRuntime(writes("first", 0.5, "out/first"));
  const read = (path) => device.readValues([path])[path];
  try {
    // Nothing steps the device: a step at a zero dt applies the LOAD, and
    // the loaded graph runs in it, with no time elapsed.
    await within(device.loadGraph(writes("loaded", 0.25, "out/loaded")), "loadGraph, unstepped");
    same(read("out/loaded"), { f32: 0.25 }, "the loaded graph ran in the step that applied it");
    same(read("out/first"), null, "the graph it replaced never ran");
    same(read("arora/time"), { u64: 0 }, "no time elapsed");
    await within(device.applyGraphEdits(adds("edited", 0.125, "out/edited")), "applyGraphEdits, unstepped");
    same(read("out/edited"), { f32: 0.125 }, "the edit ran in the step that applied it");
    same(read("out/loaded"), { f32: 0.25 }, "the nodes the edit leaves run on");
    same(read("arora/time"), { u64: 0 }, "still no time elapsed");

    // A turn's LOAD and EDIT share one step, applied in the order issued:
    // the edit lands on the graph the load installed.
    const loaded = device.loadGraph(writes("turn", 0.5, "out/turn"));
    const edited = device.applyGraphEdits(adds("turn_edit", 0.0625, "out/turn_edit"));
    await within(Promise.all([loaded, edited]), "a turn's loadGraph and applyGraphEdits");
    same(read("out/turn"), { f32: 0.5 }, "the turn's graph runs");
    same(read("out/turn_edit"), { f32: 0.0625 }, "its edit, applied after the load, runs");
    same(read("out/edited"), { f32: 0.125 }, "the key the previous graph wrote keeps its value");

    // The swap keeps the runs and the holds.
    const run = await within(
      device.invoke("run_behavior", {
        name: "program",
        behavior: await vizij.behaviorValue(writes("program", 0.75, "out/program")),
      }),
      "invoke, unstepped",
    );
    same(vizij.runStatus(read(run.status)), "running", "the run runs");
    device.holdKeys(["out/held"]);
    device.setValue("out/held", { f32: 0.0625 });
    device.setValue("out/program", { f32: 0 });
    await within(
      device.loadGraph(writes("swapped", 0.375, "out/swapped", "out/held")),
      "loadGraph beside a run",
    );
    same(read("out/swapped"), { f32: 0.375 }, "the new graph runs");
    same(read("out/program"), { f32: 0.75 }, "the run runs on through the swap");
    same(vizij.runStatus(read(run.status)), "running", "the run still runs");
    same(read("out/held"), { f32: 0.0625 }, "a held key keeps the page's value against the new graph");
    device.releaseKeys();
    device.step(16);
    same(read("out/held"), { f32: 0.375 }, "released, the new graph writes it");

    // A LOAD or an EDIT leaving a Remap's `in` unwired would fail every
    // step, the run's with it: it is refused, naming the node and the port,
    // and the graph that ran and the run go on.
    const unwired = {
      nodes: [
        { id: "unwired", type: "remap" },
        { id: "unwired_out", type: "output", params: { path: "out/unwired" } },
      ],
      edges: [{ from: { node_id: "unwired" }, to: { node_id: "unwired_out", input: "in" } }],
    };
    for (const [what, refused] of [
      ["applyGraphEdits", () => device.applyGraphEdits({ upsert_nodes: unwired.nodes, upsert_edges: unwired.edges })],
      ["loadGraph", () => device.loadGraph(unwired)],
    ]) {
      device.setValue("out/swapped", { f32: 0 });
      device.setValue("out/program", { f32: 0 });
      const error = await within(
        refused().then(
          () => null,
          (e) => String(e),
        ),
        what,
      );
      check(error !== null, `${what} of an unwired Remap is refused`);
      check(error.includes("'unwired'") && error.includes("'in'"), `${what} names the node and the port: ${error}`);
      device.step(16);
      same(read("out/unwired"), null, `${what}: nothing of the refused graph ran`);
      same(read("out/swapped"), { f32: 0.375 }, `${what}: the graph that ran runs on`);
      same(read("out/program"), { f32: 0.75 }, `${what}: the run runs on`);
      same(vizij.runStatus(read(run.status)), "running", `${what}: the run still runs`);
      same(device.behaviorError, undefined, `${what}: the behavior stays healthy`);
    }

    // `run()` steps the device: the run's next step applies the LOAD and
    // the EDIT, and the page adds no step. A step at a zero dt would apply an
    // operation in the microtasks after it is issued, and the run's next step
    // waits on a timer, which no microtask lets through: so the operation is
    // still unapplied once those microtasks have run, however late the
    // run's timer fires on a loaded page.
    const period = 300;
    const running = device.run(period);
    await sleep(50);
    for (const [what, pending, path, value] of [
      ["loadGraph under run()", () => device.loadGraph(writes("ran", 0.625, "out/ran")), "out/ran", 0.625],
      ["applyGraphEdits under run()", () => device.applyGraphEdits(adds("ran_edit", 0.875, "out/ran_edit")), "out/ran_edit", 0.875],
    ]) {
      const settled = pending();
      await Promise.resolve();
      await Promise.resolve();
      same(read(path), null, `${what}: no step of the page's applied it`);
      await within(settled, what, period * 4);
      same(read(path), { f32: value }, `${what}: the run's step applied it and ran it`);
      check(read("arora/dt")?.u64 > 0, `${what}: the step that applied it was the run's`);
    }
    device.stop();
    await within(running, "run() after stop()");
  } finally {
    device.dispose();
  }
}
