// describe(glb) on a page reads what Quori's bundle carries for an app: its
// poses and pose groups, its rig inputs, its programs' labels, its
// animations and its metadata. Toasty's bundle declares no poses or
// animations, and reads empty.
// Needs `VIZIJ_FIXTURES`; skips without.
import assert from "node:assert/strict";
import { FIXTURES, open } from "./common.mjs";

if (!FIXTURES) {
  console.log("VIZIJ_FIXTURES unset — skipping the browser describe test");
  process.exit(0);
}

const { page, logs, close } = await open(FIXTURES);
try {
  const [quori, toasty] = await page.evaluate(async () => {
    const vizij = await import("../../dist/runtime/src/index.js");
    const describe = async (file) =>
      vizij.describe(new Uint8Array(await (await fetch(`/fixtures/${file}`)).arrayBuffer()));
    return [await describe("Quori_Current_Extended.glb"), await describe("Toasty_Current.glb")];
  });

  // Poses and their groups.
  assert.deepEqual(
    quori.poseGroups.map((g) => [g.id, g.name, g.path]),
    [
      ["default", "Visemes", "visemes"],
      ["emotionsv2", "Emotions", "emotions"],
    ],
  );
  assert.equal(quori.poses.length, 24);
  const groupIds = new Set(quori.poseGroups.map((g) => g.id));
  for (const pose of quori.poses) {
    assert.ok(pose.id && pose.name, `pose without id or name: ${JSON.stringify(pose)}`);
    assert.ok(pose.groupIds.length > 0, `${pose.id} belongs to no group`);
    for (const id of pose.groupIds) assert.ok(groupIds.has(id), `${pose.id}: unknown group ${id}`);
    assert.ok(Object.keys(pose.values).length > 0, `${pose.id} sets no input`);
  }
  const visemeA = quori.poses.find((p) => p.id === "pose_a");
  assert.equal(visemeA.name, "A");
  assert.equal(typeof visemeA.description, "string");
  assert.deepEqual(visemeA.groupIds, ["default"]);
  assert.ok(quori.poses.some((p) => p.groupIds.includes("emotionsv2")));

  // The rig's inputs: rig-relative paths with their ranges and defaults.
  assert.ok(quori.rigInputs.length > 0);
  for (const input of quori.rigInputs) {
    assert.ok(input.path && !input.path.startsWith("/"), `bad path ${input.path}`);
    assert.equal(typeof input.min, "number", input.path);
    assert.equal(typeof input.max, "number", input.path);
    assert.equal(typeof input.defaultValue, "number", input.path);
  }
  const cross = quori.rigInputs.find((i) => i.id === "gaze_left_right_copy");
  assert.deepEqual(cross, {
    id: "gaze_left_right_copy",
    path: "gaze/left_right_copy",
    label: "Cross",
    group: "gaze",
    defaultValue: 0,
    min: -1,
    max: 1,
  });

  // Programs with their labels and graphs; the bundle's active one.
  assert.deepEqual(
    quori.programs.map(({ id, label }) => [id, label]),
    [
      ["authoring.motiongraph.program.1", "Speaks"],
      ["authoring.motiongraph.main", "Live"],
    ],
  );
  for (const program of quori.programs) {
    assert.ok(program.graph.nodes.length > 0, `${program.id} carries its graph`);
  }
  assert.equal(quori.activeProgramId, "authoring.motiongraph.program.1");

  // Animations: ids, names, durations, tracks of time-ordered keyframes.
  assert.deepEqual(
    quori.animations.map((a) => [a.id, a.name, a.duration, a.tracks.length]),
    [
      ["authoring.timeline.clip.1", "Nonesense", 5, 4],
      ["authoring.timeline.main", "Stages", 15, 5],
    ],
  );
  const gaze = quori.animations[0].tracks[0];
  assert.equal(gaze.channel, "gaze/left_right");
  assert.equal(gaze.interpolation, "linear");
  assert.equal(gaze.keyframes.length, 4);
  assert.deepEqual(gaze.keyframes[0], { time: 0, value: -0.04, interpolation: "linear" });
  for (const animation of quori.animations) {
    for (const track of animation.tracks) {
      const times = track.keyframes.map((k) => k.time);
      assert.deepEqual(times, [...times].sort((a, b) => a - b), `${animation.id}/${track.channel}`);
    }
  }

  // The metadata, as authored.
  assert.equal(quori.metadata.faceId, "quori_latest");
  assert.equal(quori.metadata.activeMotionGraphId, "authoring.motiongraph.program.1");
  assert.equal(quori.metadata.speechConfig.voice, "Ruth");
  assert.equal(quori.metadata.speechConfig.visemeGroupId, "default");

  // A bundle without poses, animations or labelled programs reads empty, not
  // absent.
  assert.deepEqual(toasty.poses, []);
  assert.deepEqual(toasty.poseGroups, []);
  assert.deepEqual(toasty.animations, []);
  assert.ok(toasty.programs.every((program) => program.label === null));
  assert.ok(toasty.rigInputs.length > 0);
  assert.equal(toasty.metadata.faceId, toasty.faceId);

  console.log(
    `@vizij/runtime browser describe: ok (${quori.poses.length} poses, ` +
      `${quori.rigInputs.length} rig inputs, ${quori.animations.length} animations)`,
  );
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
