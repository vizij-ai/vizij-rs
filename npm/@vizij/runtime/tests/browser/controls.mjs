// A drag on a face's control, through the page's pointer: each drag sends a
// `start`, its `move`s and exactly one closing event, every one under one
// `gesture` and the modifiers the page last passed in. A drag the page
// cancels closes with a `cancel` carrying no pose, and sends nothing more,
// its release included; a drag let go of closes with an `end`, under a
// gesture of its own. Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, loadVizij, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser controls test");
  process.exit(0);
}

const { page, logs, close } = await open(FIXTURES);
try {
  await loadVizij(page, "a", "Quori_Current_Extended.glb", {
    program: "none",
    animations: false,
  });
  await page.evaluate(() =>
    window.vizijHarness.control({
      op: "add",
      key: "move",
      control: {
        kind: "transform",
        target: { kind: "instance", instance: "a" },
        modes: ["translate"],
      },
    }),
  );
  const h = (name, ...args) =>
    page.evaluate(([name, args]) => window.vizijHarness[name](...args), [name, args]);
  /** The latest overlay the page heard. */
  const overlay = () =>
    page.evaluate(
      () =>
        window.vizijHarness
          .events()
          .filter((event) => event.type === "overlay")
          .at(-1) ?? null,
    );
  const box = await (async () => {
    await page.waitForFunction(() =>
      window.vizijHarness
        .events()
        .some((event) => event.type === "overlay" && event.gizmos.some((g) => g.key === "move")),
    );
    return (await overlay()).gizmos.find((g) => g.key === "move").screen;
  })();

  // Over the control's box until one of its handles is under the pointer.
  let spot = null;
  for (let y = box.min_y; y <= box.max_y && !spot; y += 6) {
    for (let x = box.min_x; x <= box.max_x && !spot; x += 6) {
      await page.mouse.move(x, y);
      await h("frames", 2);
      const pointer = (await overlay())?.pointer;
      if (pointer?.key === "move" && !pointer.held) spot = { x, y };
    }
  }
  assert.ok(spot, `a handle of the control in ${JSON.stringify(box)}`);

  /** The `control` events heard since `from`. */
  const controls = async (from) =>
    (await h("events")).slice(from).filter((event) => event.type === "control");
  const drag = async () => {
    const from = (await h("events")).length;
    await page.mouse.move(spot.x, spot.y);
    await h("frames", 2);
    await page.mouse.down();
    await h("frames", 2);
    for (let step = 1; step <= 4; step++) {
      await page.mouse.move(spot.x + 6 * step, spot.y - 4 * step);
      await h("frames", 2);
    }
    return from;
  };
  const phases = (events) => events.map((event) => event.phase);
  /** Every event is the control's, under one gesture and the modifiers
   * `held`; a `cancel` carries no pose or scale, every other event does. */
  const oneDrag = (events, held) => {
    const gesture = events[0].gesture;
    assert.equal(typeof gesture, "number");
    for (const event of events) {
      assert.equal(event.key, "move");
      assert.equal(event.gesture, gesture, "one drag, one gesture");
      assert.deepEqual(event.modifiers, held);
      const cancel = event.phase === "cancel";
      assert.equal("pose" in event, !cancel, JSON.stringify(event));
      assert.equal("scale" in event, !cancel, JSON.stringify(event));
    }
    return gesture;
  };

  // Cancelled by the page while the button is down.
  const held = { alt: true, shift: false, meta: true, ctrl: false };
  await h("setModifiers", { alt: false, shift: true, meta: false, ctrl: false });
  await h("setModifiers", held);
  let from = await drag();
  await h("cancelDrag");
  await h("frames", 3);
  await page.mouse.move(spot.x + 40, spot.y - 30);
  await h("frames", 2);
  await page.mouse.up();
  await h("frames", 3);
  const cancelled = await controls(from);
  const sent = phases(cancelled);
  assert.equal(sent[0], "start", sent.join());
  assert.ok(sent.includes("move"), sent.join());
  assert.equal(sent.at(-1), "cancel", sent.join());
  assert.equal(sent.filter((phase) => phase === "end" || phase === "cancel").length, 1, sent.join());
  const first = oneDrag(cancelled, held);

  // Let go of: it ends, under a gesture of its own and what is held now.
  const none = { alt: false, shift: false, meta: false, ctrl: false };
  await h("setModifiers", none);
  from = await drag();
  await page.mouse.up();
  await h("frames", 3);
  const ended = await controls(from);
  const endedPhases = phases(ended);
  assert.equal(endedPhases[0], "start", endedPhases.join());
  assert.equal(endedPhases.at(-1), "end", endedPhases.join());
  assert.equal(endedPhases.filter((phase) => phase === "end" || phase === "cancel").length, 1);
  assert.notEqual(oneDrag(ended, none), first, "a gesture of its own");

  console.log("controls: a page-cancelled drag closes once with cancel, a released one with end");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
