// The calls an authoring viewport makes, on a page:
// - a press where the page shows no element reports a miss of the Vizij;
// - the selection glow outlines the chosen element and not another;
// - outputs held against the device stay put while a program drives them,
//   and follow it again once released;
// - a static feature changes in place, with no reload;
// - a structural edit's reload from GLB bytes keeps the previous scene on
//   screen until the new one is ready (each reload is timed and printed),
//   and a GLB exported from the authoring world round-trips with zero
//   differences — given `VIZIJ_EXPORTED_QUORI`, the path of
//   `Quori_Current.glb` as the authoring app's exporter wrote it back.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { promises as fs } from "node:fs";
import { PNG } from "pngjs";
import { FIXTURES, loadVizij, meanDiff32, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser authoring test");
  process.exit(0);
}

const CANVAS = { x: 0, y: 0, width: 763, height: 486 };
const QUORI = "Quori_Current.glb";
const PROGRAMMED = "Quori_Current_Extended.glb";

const { page, logs, close } = await open(FIXTURES);
const harness = (name, ...args) =>
  page.evaluate(([name, args]) => window.vizijHarness[name](...args), [name, args]);
const shot = async () => PNG.sync.read(await page.screenshot({ clip: CANVAS }));
const shotBuffer = async () => page.screenshot({ clip: CANVAS });
const frames = (n) =>
  page.evaluate(
    (n) =>
      new Promise((resolve) => {
        const tick = (left) => (left ? requestAnimationFrame(() => tick(left - 1)) : resolve());
        tick(n);
      }),
    n,
  );
const at = (png, x, y) => {
  const i = (y * png.width + x) * 4;
  return [png.data[i], png.data[i + 1], png.data[i + 2]];
};
// The pixels where `after` turned to the glow's red from `before`.
const reddened = (before, after) => {
  const out = [];
  for (let y = 0; y < after.height; y++) {
    for (let x = 0; x < after.width; x++) {
      const [r0, g0, b0] = at(before, x, y);
      const [r, g, b] = at(after, x, y);
      const changed = Math.abs(r - r0) + Math.abs(g - g0) + Math.abs(b - b0) > 60;
      if (changed && r > 150 && r > g + 80 && r > b + 80) out.push([x, y]);
    }
  }
  return out;
};
const meanX = (points) => points.reduce((sum, [x]) => sum + x, 0) / points.length;
// A press held over a frame (a software GPU's frame can outlast a click);
// what it picked. The browser can deliver one press twice (headless
// Chromium's pointer and mouse events), so the picks it produced must agree.
const press = async (x, y) => {
  await harness("picks");
  await page.mouse.move(x, y);
  await page.waitForTimeout(300);
  await page.mouse.down();
  await page.waitForTimeout(300);
  await page.mouse.up();
  await page.waitForTimeout(300);
  const picks = await harness("picks");
  for (const pick of picks) assert.deepEqual(pick, picks[0], `one press, ${JSON.stringify(picks)}`);
  return picks.slice(0, 1);
};

try {
  const quori = await harness("describe", `/fixtures/${QUORI}`);
  const element = (name) => quori.elements.find((e) => e.name === name);

  await loadVizij(page, "q", QUORI, { program: "none", audio: false });
  await frames(30);

  // A press on no element is the Vizij's miss; one on an element names it.
  {
    const png = await shot();
    // The background is black: find a point that shows it, inside the canvas.
    let empty = null;
    for (let y = 4; y < CANVAS.height && !empty; y += 8) {
      for (let x = 4; x < CANVAS.width && !empty; x += 8) {
        if (at(png, x, y).every((c) => c < 8)) empty = [x, y];
      }
    }
    assert.ok(empty, "no background pixel on the canvas");
    let picks = [];
    for (let attempt = 0; attempt < 5 && picks.length === 0; attempt++) {
      picks = await press(...empty);
    }
    assert.deepEqual(picks, [{ vizijId: "q", elementId: null }], `a press at ${empty}`);
    picks = [];
    for (let attempt = 0; attempt < 5 && picks.length === 0; attempt++) {
      picks = await press(CANVAS.width / 2, CANVAS.height / 2);
    }
    assert.equal(picks.length, 1, JSON.stringify(picks));
    assert.ok(
      quori.elements.some((e) => e.id === picks[0].elementId),
      `the centre picked ${JSON.stringify(picks)}`,
    );
    console.log(`picks: a miss at ${empty}, ${picks[0].elementId} at the centre`);
  }

  // The glow outlines the selected eye, and not the other one.
  {
    const left = element("L_Eye");
    const right = element("R_Eye");
    assert.ok(left && right, "Quori declares L_Eye and R_Eye");
    const plain = await shot();
    await harness("select", "q", [left.id]);
    await frames(10);
    const leftGlow = reddened(plain, await shot());
    await harness("select", "q", [right.id]);
    await frames(10);
    const rightGlow = reddened(plain, await shot());
    await harness("select", "q", []);
    await frames(10);
    const cleared = reddened(plain, await shot());
    console.log(
      `glow: ${leftGlow.length} px around L_Eye (x̄ ${meanX(leftGlow).toFixed(0)}), ` +
        `${rightGlow.length} px around R_Eye (x̄ ${meanX(rightGlow).toFixed(0)})`,
    );
    assert.ok(leftGlow.length > 50, "no glow on L_Eye");
    assert.ok(rightGlow.length > 50, "no glow on R_Eye");
    // The face's left eye is on the image's right.
    assert.ok(meanX(leftGlow) > CANVAS.width / 2, "L_Eye's glow is not over L_Eye");
    assert.ok(meanX(rightGlow) < CANVAS.width / 2, "R_Eye's glow is not over R_Eye");
    // Sampled at each glow's pixels, the other selection shows none.
    const key = ([x, y]) => `${x},${y}`;
    const rightSet = new Set(rightGlow.map(key));
    assert.equal(leftGlow.filter((p) => rightSet.has(key(p))).length, 0, "the glows overlap");
    assert.equal(cleared.length, 0, "the cleared selection still glows");
  }

  // A static feature, set in place: Background's colour made static in the
  // GLB, then set to red — the scene changes with no reload.
  {
    const background = element("Background");
    await page.evaluate(
      async ([url, name]) => {
        const source = new Uint8Array(await (await fetch(url)).arrayBuffer());
        const view = new DataView(source.buffer);
        const jsonLength = view.getUint32(12, true);
        const gltf = JSON.parse(new TextDecoder().decode(source.subarray(20, 20 + jsonLength)));
        const node = gltf.nodes.find((n) => n.name === name);
        node.extensions.RobotData.features.color = {
          animated: false,
          value: { r: 0.2, g: 0.2, b: 0.2 },
        };
        let json = new TextEncoder().encode(JSON.stringify(gltf));
        const padded = new Uint8Array(Math.ceil(json.length / 4) * 4).fill(0x20);
        padded.set(json);
        json = padded;
        const rest = source.subarray(20 + jsonLength);
        const out = new Uint8Array(20 + json.length + rest.length);
        const outView = new DataView(out.buffer);
        out.set(source.subarray(0, 12));
        outView.setUint32(8, out.length, true);
        outView.setUint32(12, json.length, true);
        outView.setUint32(16, 0x4e4f534a, true);
        out.set(json, 20);
        out.set(rest, 20 + json.length);
        await window.vizijHarness.loadBytes("s", out, { program: "none", audio: false });
      },
      [`/fixtures/${QUORI}`, "Background"],
    );
    await page.waitForFunction(() => window.vizijHarness.ready("s"), null, { timeout: 240_000 });
    await frames(30);
    const loads = () => logs.filter((l) => l.includes("face s: loading")).length;
    const loadsBefore = loads();
    const before = await shot();
    await harness("setStatic", "s", background.id, "color", { r: 1, g: 0, b: 0 });
    await frames(10);
    const after = await shot();
    const red = reddened(before, after);
    console.log(`static feature: ${red.length} px turned red`);
    assert.ok(red.length > 1000, "the static colour did not show");
    assert.equal(loads(), loadsBefore, "the static feature reloaded the Vizij");
    assert.ok(await harness("ready", "s"), "the Vizij left ready");
    await harness("unload", "s");
  }

  // Held outputs stay put while the bundle's program drives them.
  {
    await harness("unload", "q");
    await loadVizij(page, "p", PROGRAMMED, { program: "auto", audio: false });
    const outputs = Object.keys((await harness("describe", `/fixtures/${PROGRAMMED}`)).animatables);
    // Each output's store key: the animatable id, under the rig's prefix.
    const keys = Object.keys(await harness("snapshot", "p"));
    const keyOf = Object.fromEntries(
      outputs.map((id) => [id, keys.find((key) => key.endsWith(id))]),
    );
    const read = async () => {
      const snapshot = await harness("snapshot", "p");
      return outputs.map((id) => JSON.stringify(snapshot[keyOf[id]] ?? null));
    };
    // What the program moves over two seconds.
    const first = await read();
    await page.waitForTimeout(2000);
    const second = await read();
    const driven = outputs.filter((_, i) => first[i] !== second[i]);
    console.log(`hold: the program drives ${driven.length} of ${outputs.length} outputs`);
    assert.ok(driven.length > 0, "the program drives no output");
    const drivenA = await shotBuffer();
    await page.waitForTimeout(1500);
    const moving = meanDiff32(drivenA, await shotBuffer());
    assert.ok(moving > 0, "the face does not move under its program");

    await harness("hold", "p", outputs);
    await frames(5);
    const heldA = await shotBuffer();
    const storeA = await read();
    await page.waitForTimeout(1500);
    const heldB = await shotBuffer();
    const storeB = await read();
    const heldDiff = meanDiff32(heldA, heldB);
    console.log(`hold: frame diff ${moving.toFixed(3)} driven, ${heldDiff.toFixed(3)} held`);
    assert.ok(
      storeA.some((v, i) => v !== storeB[i]),
      "the device stopped while its outputs were held",
    );
    assert.equal(heldDiff, 0, "a held output moved");

    await harness("release", "p");
    await frames(5);
    const releasedA = await shotBuffer();
    await page.waitForTimeout(1500);
    assert.ok(meanDiff32(releasedA, await shotBuffer()) > 0, "released outputs stay put");
    await harness("unload", "p");
  }

  // A structural edit reloads from GLB bytes: the previous scene stays on
  // screen meanwhile, and the swap lands once the new one is indexed.
  {
    await loadVizij(page, "r", QUORI, { program: "none", audio: false });
    await frames(30);
    const settled = await shotBuffer();
    const timings = [];
    for (let i = 0; i < 5; i++) {
      await page.evaluate(
        ([url]) => {
          window.reloading = window.vizijHarness.reload("r", url, { program: "none" });
        },
        [`/fixtures/${QUORI}`],
      );
      const during = await shotBuffer();
      const timing = await page.evaluate(() => window.reloading);
      assert.equal(timing.readyAt, false, "the Vizij read ready before its new scene was");
      await frames(5);
      const diffDuring = meanDiff32(settled, during);
      const diffAfter = meanDiff32(settled, await shotBuffer());
      assert.ok(diffDuring < 1, `the canvas changed during reload ${i} (${diffDuring})`);
      assert.ok(diffAfter < 1, `the reloaded face differs (${diffAfter})`);
      timings.push(timing);
    }
    // Two reloads in one turn: the second supersedes the first's pending
    // scene too, and the one face left is the last loaded.
    const retired = () => logs.filter((l) => l.includes("face r: reloaded")).length;
    const retiredBefore = retired();
    await page.evaluate(
      ([url]) =>
        Promise.all([
          window.vizijHarness.reload("r", url, { program: "none" }),
          window.vizijHarness.reload("r", url, { program: "none" }),
        ]),
      [`/fixtures/${QUORI}`],
    );
    await frames(30);
    assert.equal(retired() - retiredBefore, 2, "a back-to-back reload left a face behind");
    assert.ok(meanDiff32(settled, await shotBuffer()) < 1, "the face after a double reload differs");
    const ms = (key) => timings.map((t) => t[key].toFixed(0)).join(", ");
    console.log(`reload of ${QUORI} (ms): device built ${ms("loadMs")}; scene ready ${ms("readyMs")}`);

    // An exported GLB round-trips: what it declares and what it draws.
    const exported = process.env.VIZIJ_EXPORTED_QUORI;
    if (exported) {
      const bytes = await fs.readFile(exported);
      await page.evaluate(
        async ([data]) => {
          const glb = Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
          window.exportedDescription = await window.vizijHarness.describeBytes(glb);
          await window.vizijHarness.loadBytes("r", glb, { program: "none", audio: false });
        },
        [bytes.toString("base64")],
      );
      await page.waitForFunction(() => window.vizijHarness.ready("r"), null, { timeout: 240_000 });
      await frames(30);
      const description = await page.evaluate(() => window.exportedDescription);
      assert.deepEqual(description, quori, "the exported GLB declares something else");
      const diff = meanDiff32(settled, await shotBuffer());
      console.log(`round trip of the exported ${QUORI}: description equal, frame diff ${diff}`);
      assert.equal(diff, 0, "the exported GLB draws differently");
    } else {
      console.log("VIZIJ_EXPORTED_QUORI unset — skipping the exported round trip");
    }
  }

  const state = await harness("state");
  assert.deepEqual(state.stepErrors, []);
  console.log("@vizij/runtime browser authoring: ok");
} catch (e) {
  console.error(logs.slice(-40).join("\n"));
  throw e;
} finally {
  await close();
}
