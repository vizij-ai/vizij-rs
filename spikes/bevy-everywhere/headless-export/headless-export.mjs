// T1: export a face GLB from the JS world model with no renderer and no
// mounted react-three-fiber objects, through the real @vizij/render functions.
import { readFileSync, writeFileSync } from "node:fs";
import * as THREE from "three";

const WEB = "/Users/victor.paleologue/Code/Semio/vizij-web";
const RENDER = `${WEB}/packages/@vizij/render/src`;
const { loadGLTFFromBlobWithBundle, parseGlbJsonChunk } = await import(`${RENDER}/functions/load-gltf.ts`);
const { exportScene } = await import(`${RENDER}/functions/export.ts`);
const { selectExportableGroupEntries } = await import(`${RENDER}/functions/exportable-bodies.ts`);
const { createStoredRenderable } = await import(`${RENDER}/functions/create-stored-data.ts`);
const { applyDefaultsToRobotData } = await import(`${WEB}/apps/vizij-authoring/src/utils/robotData.ts`);

const GLB = process.argv[2] ?? `${WEB}/apps/vizij-authoring/public/assets/Quori_Current_Extended.glb`;
const OUT = process.argv[3] ?? "quori";
const NS = "default";
const AGGRESSIVE = process.argv.includes("--aggressive");

// ---- browser shims: only what exportScene's download helper touches -------
const capturedBlobs = [];
const nativeCreateObjectURL = URL.createObjectURL;
globalThis.document = {
  createElement: () => ({ style: {}, click() {} }),
  body: { appendChild() {}, removeChild() {} },
};
const nativeRevokeObjectURL = URL.revokeObjectURL;
async function withObjectUrlCapture(fn) {
  URL.createObjectURL = (blob) => { capturedBlobs.push(blob); return `blob:captured:${capturedBlobs.length}`; };
  URL.revokeObjectURL = () => {};
  try { return await fn(); } finally { URL.createObjectURL = nativeCreateObjectURL; URL.revokeObjectURL = () => {}; }
}
async function withoutObjectUrl(fn) {
  // Forces load-gltf's ArrayBuffer branch (GLTFLoader.parse), no fetch, no blob URL.
  URL.createObjectURL = undefined;
  URL.revokeObjectURL = undefined;
  try { return await fn(); } finally { URL.createObjectURL = nativeCreateObjectURL; URL.revokeObjectURL = () => {}; }
}

// ---- arora Value forms -> raw values (same rules as utils/rig/animatable-metadata.ts) ----
const SCALAR_KEYS = ["f32", "f64", "i64", "i32", "u64", "u32", "float"];
function unwrapScalar(v) {
  if (typeof v === "number") return v;
  if (v && typeof v === "object") for (const k of SCALAR_KEYS) if (typeof v[k] === "number") return v[k];
  return undefined;
}
function rawDefault(anim) {
  const d = anim?.default;
  const s = unwrapScalar(d);
  if (s !== undefined) return s;
  if (d && typeof d === "object" && d.struct?.fields) {
    const n = d.struct.fields.map((f) => unwrapScalar(f?.value) ?? 0);
    switch (anim.type) {
      case "vector3": case "euler": return { x: n[0], y: n[1], z: n[2] };
      case "vector2": return { x: n[0], y: n[1] };
      case "rgb": return { r: n[0], g: n[1], b: n[2] };
      case "hsl": return { h: n[0], s: n[1], l: n[2] };
      default: return n;
    }
  }
  return d;
}
const isVec3 = (o) => o && typeof o === "object" && o.x !== undefined && o.y !== undefined && o.z !== undefined;
const isRGB = (o) => o && typeof o === "object" && o.r !== undefined && o.g !== undefined && o.b !== undefined;
const isHSL = (o) => o && typeof o === "object" && o.h !== undefined && o.s !== undefined && o.l !== undefined;
const isNum = (o) => typeof o === "number";

// ---- material naming (copy of shape.tsx deriveMaterialName) ----
const MATERIAL_FEATURE_KEYS = ["color", "opacity", "roughness", "metalness", "shininess", "emissive", "emissiveIntensity", "specular"];
const MATERIAL_NAME_SUFFIXES = [" color", " colours", " colour", " opacity", " roughness", " metalness", " shininess", " emissive intensity", " emissive", " specular"];
function deriveMaterialName(shape, values) {
  for (const key of MATERIAL_FEATURE_KEYS) {
    const f = shape.features[key];
    if (f && f.animated) {
      const name = values[f.value]?.name;
      if (!name) continue;
      const trimmed = name.trim(); const lowered = trimmed.toLowerCase();
      for (const suffix of MATERIAL_NAME_SUFFIXES) if (lowered.endsWith(suffix)) return trimmed.slice(0, trimmed.length - suffix.length).trim();
      return trimmed;
    }
  }
  return undefined;
}
function makeMaterial(kind) {
  switch (kind) {
    case "basic": return new THREE.MeshBasicMaterial({ side: THREE.DoubleSide });
    case "lambert": return new THREE.MeshLambertMaterial({ side: THREE.DoubleSide });
    case "phong": return new THREE.MeshPhongMaterial({ side: THREE.DoubleSide });
    case "normal": return new THREE.MeshNormalMaterial({ side: THREE.DoubleSide });
    default: return new THREE.MeshStandardMaterial({ side: THREE.DoubleSide });
  }
}

// ---- the headless scene graph: what group.tsx / shape.tsx mount, minus React ----
function applyFeatures(features, animatables, values, callbacks) {
  for (const key of Object.keys(callbacks)) {
    const f = features[key];
    if (!f) continue;
    if (!f.animated) { callbacks[key](f.value); continue; }
    const anim = animatables[f.value];
    if (!anim) { console.error(`animated feature ${key} -> missing animatable ${f.value}`); continue; }
    const live = values?.get(`${NS}:${anim.id}`);
    callbacks[key](live !== undefined ? live : rawDefault(anim));
  }
}
function transformCallbacks(obj, isGroup) {
  return {
    translation: (p) => { if (isVec3(p)) obj.position.set(p.x, p.y, p.z); else if (isGroup && p && p.x !== undefined && p.y !== undefined) obj.position.set(p.x, p.y, obj.position.z); },
    rotation: (r) => { if (isVec3(r)) obj.rotation.set(r.x, r.y, r.z, "ZYX"); else if (isGroup && isNum(r)) { obj.rotation.set(0, 0, 0); obj.rotateZ(r); } },
    scale: (s) => {
      if (isVec3(s)) { if (isGroup && (s.x === null || s.y === null || s.z === null)) obj.scale.set(0.1, 0.1, 0.1); else obj.scale.set(s.x, s.y, s.z); }
      else if (isNum(s)) obj.scale.set(s, s, s); else obj.scale.set(1, 1, 1);
    },
  };
}
function buildHeadless(world, animatables, id, chain, values, out) {
  const entry = world[id];
  if (!entry) throw new Error(`world has no entry ${id}`);
  const animVals = {};
  for (const f of Object.values(entry.features)) if (f.animated && animatables[f.value]) animVals[f.value] = animatables[f.value];
  const robotData = createStoredRenderable(entry, animVals);
  let obj;
  if (entry.type === "group") {
    obj = new THREE.Group();
    obj.uuid = `${NS}.${entry.id}`;
    obj.name = entry.name;
    obj.userData = { gltfExtensions: { RobotData: robotData } };
    applyFeatures(entry.features, animatables, values, transformCallbacks(obj, true));
    for (const child of entry.children ?? []) obj.add(buildHeadless(world, animatables, child, [...chain, id], values, out));
  } else if (entry.type === "shape") {
    const geometry = entry.geometry.clone();
    if (entry.name) geometry.name = entry.name;
    const material = makeMaterial(entry.material);
    obj = new THREE.Mesh(geometry, material);
    obj.name = entry.name;
    obj.castShadow = true; obj.receiveShadow = true;
    obj.userData = { gltfExtensions: { RobotData: robotData }, selection: { id, namespace: NS, type: "shape" } };
    const morphHandlers = {};
    if (entry.morphTargets) {
      obj.morphTargetDictionary = Object.fromEntries(entry.morphTargets.map((t, i) => [t, i]));
      obj.morphTargetInfluences = entry.morphTargets.map(() => 0);
      entry.morphTargets.forEach((t, i) => { morphHandlers[t] = (v) => { if (isNum(v)) obj.morphTargetInfluences[i] = v; }; });
    }
    material.name = deriveMaterialName(entry, animVals) ?? entry.name ?? "";
    applyFeatures(entry.features, animatables, values, {
      ...transformCallbacks(obj, false),
      opacity: (op) => { if (isNum(op)) { material.opacity = op; material.transparent = op < 1.0; } },
      color: (c) => { if (material.color) { if (isRGB(c)) material.color.setRGB(c.r, c.g, c.b); else if (isVec3(c)) material.color.setRGB(c.x, c.y, c.z); else if (isHSL(c)) material.color.setHSL(c.h, c.s, c.l); } },
      roughness: (v) => { if (material.roughness !== undefined && isNum(v)) material.roughness = v; },
      metalness: (v) => { if (material.metalness !== undefined && isNum(v)) material.metalness = v; },
      shininess: (v) => { if (material.shininess !== undefined && isNum(v)) material.shininess = v; },
      emissive: (v) => { if (material.emissive && isRGB(v)) material.emissive.setRGB(v.r, v.g, v.b); },
      emissiveIntensity: (v) => { if (material.emissiveIntensity !== undefined && isNum(v)) material.emissiveIntensity = v; },
      specular: (v) => { if (material.specular && isRGB(v)) material.specular.setRGB(v.r, v.g, v.b); },
      ...morphHandlers,
    });
    for (const child of entry.children ?? []) obj.add(buildHeadless(world, animatables, child, [...chain, id], values, out));
  } else if (entry.type === "ellipse" || entry.type === "rectangle") {
    // drei <Circle args={[1,100]}> / <Plane args={[1,1]}> equivalents; the stroke Line is not reproduced.
    const geometry = entry.type === "ellipse" ? new THREE.CircleGeometry(1, 100) : new THREE.PlaneGeometry(1, 1);
    obj = new THREE.Mesh(geometry, new THREE.MeshStandardMaterial());
    obj.name = entry.name;
    obj.userData = { gltfExtensions: { RobotData: robotData }, selection: { id, namespace: NS, type: entry.type } };
    applyFeatures(entry.features, animatables, values, transformCallbacks(obj, false));
  } else {
    throw new Error(`unhandled type ${entry.type}`);
  }
  if (entry.refs?.[NS]) entry.refs[NS].current = obj; // what store.setReference records
  out.set(id, obj);
  return obj;
}

// ---- export through the real exportScene, capturing the download blob ----
async function runExport(root, bundle, fileName) {
  const before = capturedBlobs.length;
  await withObjectUrlCapture(() => new Promise((resolve, reject) => {
    exportScene(root, { fileName, bundle, onComplete: resolve, onError: reject });
  }));
  if (capturedBlobs.length !== before + 1) throw new Error("export produced no download");
  return capturedBlobs.at(-1).arrayBuffer();
}

// ---- diff helpers ----
function deepDiff(a, b, path = "", acc = [], tol = 0) {
  if (acc.length > 400) return acc;
  if (typeof a === "number" && typeof b === "number") { if (Math.abs(a - b) > tol && !(Number.isNaN(a) && Number.isNaN(b))) acc.push({ path, a, b }); return acc; }
  if (a === b) return acc;
  if (a === null || b === null || typeof a !== "object" || typeof b !== "object") { acc.push({ path, a, b }); return acc; }
  if (Array.isArray(a) !== Array.isArray(b)) { acc.push({ path, a: Array.isArray(a) ? "array" : "object", b: Array.isArray(b) ? "array" : "object" }); return acc; }
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const k of keys) deepDiff(a[k], b[k], `${path}/${k}`, acc, tol);
  return acc;
}
function summarizeNodes(json) {
  return (json.nodes ?? []).map((n) => ({
    name: n.name, mesh: n.mesh, children: (n.children ?? []).map((i) => json.nodes[i]?.name),
    trs: { t: n.translation, r: n.rotation, s: n.scale }, extras: n.extras,
    robotData: n.extensions?.RobotData, hasBundle: Boolean(n.extensions?.VIZIJ_bundle),
  }));
}
function byName(list) { const m = new Map(); const seen = {}; for (const x of list) { const k = seen[x.name] ? `${x.name}#${seen[x.name]}` : x.name; seen[x.name] = (seen[x.name] ?? 0) + 1; m.set(k, x); } return m; }
function meshSummary(json) {
  return (json.meshes ?? []).map((m) => ({
    name: m.name, weights: m.weights, targetNames: m.extras?.targetNames,
    primitives: m.primitives.map((p) => ({ attributes: Object.keys(p.attributes).sort(), targets: (p.targets ?? []).length, targetAttrs: p.targets?.[0] ? Object.keys(p.targets[0]).sort() : [], material: json.materials?.[p.material]?.name, mode: p.mode, indices: p.indices !== undefined })),
  }));
}
function compareGlb(label, srcJson, srcBytes, outJson, outBytes) {
  const rep = { label, bytes: { source: srcBytes.byteLength, output: outBytes.byteLength }, jsonChunk: { source: parseGlbJsonChunk(srcBytes) ? new DataView(srcBytes).getUint32(12, true) : null, output: new DataView(outBytes).getUint32(12, true) } };
  rep.binChunk = { source: srcBytes.byteLength - 20 - new DataView(srcBytes).getUint32(12, true) - 8, output: outBytes.byteLength - 20 - new DataView(outBytes).getUint32(12, true) - 8 };
  rep.asset = { source: srcJson.asset, output: outJson.asset };
  rep.extensionsUsed = { source: srcJson.extensionsUsed, output: outJson.extensionsUsed };
  rep.scenes = { source: srcJson.scenes, output: outJson.scenes?.map((s) => ({ ...s, extensions: s.extensions ? Object.keys(s.extensions) : undefined })) };
  rep.counts = {};
  for (const k of ["nodes", "meshes", "materials", "accessors", "bufferViews", "buffers", "images", "textures", "animations"]) rep.counts[k] = { source: (srcJson[k] ?? []).length, output: (outJson[k] ?? []).length };
  const sn = summarizeNodes(srcJson), on = summarizeNodes(outJson);
  rep.nodeOrder = { same: JSON.stringify(sn.map((n) => n.name)) === JSON.stringify(on.map((n) => n.name)), source: sn.map((n) => n.name), output: on.map((n) => n.name) };
  const sm = byName(sn), om = byName(on);
  rep.nodes = [];
  for (const [name, s] of sm) {
    const o = om.get(name);
    if (!o) { rep.nodes.push({ name, missingInOutput: true }); continue; }
    rep.nodes.push({
      name,
      robotDataDiff: deepDiff(s.robotData, o.robotData, "", [], 0),
      trsDiff: deepDiff(s.trs, o.trs, "", [], 1e-6),
      extrasDiff: deepDiff(s.extras, o.extras),
      childrenDiff: deepDiff(s.children, o.children),
      bundle: { source: s.hasBundle, output: o.hasBundle },
      meshName: { source: srcJson.meshes?.[s.mesh]?.name, output: outJson.meshes?.[o.mesh]?.name },
    });
  }
  for (const name of om.keys()) if (!sm.has(name)) rep.nodes.push({ name, missingInSource: true });
  const srcBundle = srcJson.nodes.find((n) => n.extensions?.VIZIJ_bundle)?.extensions.VIZIJ_bundle ?? srcJson.extensions?.VIZIJ_bundle;
  const outBundle = outJson.nodes.find((n) => n.extensions?.VIZIJ_bundle)?.extensions.VIZIJ_bundle ?? outJson.extensions?.VIZIJ_bundle;
  rep.bundleDiff = deepDiff(srcBundle, outBundle);
  rep.bundleEqualJson = JSON.stringify(srcBundle) === JSON.stringify(outBundle);
  rep.meshDiff = deepDiff(byNameObj(meshSummary(srcJson)), byNameObj(meshSummary(outJson)), "", [], 1e-6);
  rep.materialDiff = deepDiff(byNameObj(srcJson.materials ?? []), byNameObj(outJson.materials ?? []), "", [], 1e-6);
  rep.materialOrder = { source: (srcJson.materials ?? []).map((m) => m.name), output: (outJson.materials ?? []).map((m) => m.name) };
  return rep;
}
function byNameObj(list) { const o = {}; for (const x of list) o[x.name ?? "?"] = x; return o; }

function stripWorld(world) {
  const out = {};
  for (const [id, e] of Object.entries(world)) { const { refs, geometry, ...rest } = e; out[id] = { ...rest, geometryAttrs: geometry ? Object.keys(geometry.attributes).sort() : undefined, morphAttrs: geometry?.morphAttributes ? Object.fromEntries(Object.entries(geometry.morphAttributes).map(([k, v]) => [k, v.length])) : undefined }; }
  return out;
}

// ================================ run =================================
const t0 = performance.now();
const srcBuf = readFileSync(GLB);
const srcBytes = srcBuf.buffer.slice(srcBuf.byteOffset, srcBuf.byteOffset + srcBuf.byteLength);
const srcJson = parseGlbJsonChunk(srcBytes);

const asset = await withoutObjectUrl(() => loadGLTFFromBlobWithBundle(new Blob([srcBytes]), [NS], AGGRESSIVE));
const tLoad = performance.now();
const rootEntries = selectExportableGroupEntries(asset.world);
const rootId = rootEntries[0]?.id;
console.log(`loaded: ${Object.keys(asset.world).length} world entries, ${Object.keys(asset.animatables).length} animatables, root=${rootId} (${asset.world[rootId]?.name}), bundle=${Boolean(asset.bundle)}, ${(tLoad - t0).toFixed(0)} ms`);

// 1) headless: rebuild from the world model, no renderer, no mounted objects
const objects = new Map();
const headlessRoot = buildHeadless(asset.world, asset.animatables, rootId, [], undefined, objects);
const bodiesFromRefs = selectExportableGroupEntries(asset.world, [rootId]).map((e) => e.refs[NS].current).filter(Boolean);
console.log(`headless graph: ${objects.size} objects; store-style body lookup via refs -> ${bodiesFromRefs.length} body (${bodiesFromRefs[0]?.name})`);
applyDefaultsToRobotData([headlessRoot], asset.animatables, {});
const tBuild = performance.now();
const headlessBytes = await runExport(headlessRoot, asset.bundle, `${OUT}_headless.glb`);
const tExport = performance.now();
writeFileSync(`${OUT}_headless.glb`, Buffer.from(headlessBytes));
console.log(`headless export: ${headlessBytes.byteLength} bytes, build ${(tBuild - tLoad).toFixed(0)} ms, export ${(tExport - tBuild).toFixed(0)} ms`);

// 2) control: the app's fallback body (the loader's three scene, never mounted)
applyDefaultsToRobotData([asset.scene], asset.animatables, {});
const fallbackBytes = await runExport(asset.scene, asset.bundle, `${OUT}_fallback.glb`);
writeFileSync(`${OUT}_fallback.glb`, Buffer.from(fallbackBytes));
console.log(`fallback export: ${fallbackBytes.byteLength} bytes`);

// 3) compare JSON chunks
const headlessJson = parseGlbJsonChunk(headlessBytes);
const fallbackJson = parseGlbJsonChunk(fallbackBytes);
const report = {
  source: GLB,
  headless: compareGlb("headless-vs-source", srcJson, srcBytes, headlessJson, headlessBytes),
  fallback: compareGlb("fallback-vs-source", srcJson, srcBytes, fallbackJson, fallbackBytes),
};

// 4) round trip: reload the headless GLB through the same loader and compare the world model
const reloaded = await withoutObjectUrl(() => loadGLTFFromBlobWithBundle(new Blob([headlessBytes]), [NS], AGGRESSIVE));
report.roundTrip = {
  worldDiff: deepDiff(stripWorld(asset.world), stripWorld(reloaded.world), "", [], 0),
  animatablesDiff: deepDiff(asset.animatables, reloaded.animatables),
  bundleDiff: deepDiff(asset.bundle, reloaded.bundle),
  animations: { source: asset.animations.length, reloaded: reloaded.animations.length },
  rootBounds: { source: asset.world[rootId]?.rootBounds, reloaded: reloaded.world[rootId]?.rootBounds },
};
report.timingsMs = { load: tLoad - t0, build: tBuild - tLoad, export: tExport - tBuild, total: performance.now() - t0 };
report.memoryMB = { rss: process.memoryUsage().rss / 1e6, heapUsed: process.memoryUsage().heapUsed / 1e6 };
writeFileSync(`${OUT}_report.json`, JSON.stringify(report, null, 1));

// 5) console summary
for (const key of ["headless", "fallback"]) {
  const r = report[key];
  const nodeIssues = r.nodes.filter((n) => n.missingInOutput || n.missingInSource || n.robotDataDiff?.length || n.trsDiff.length || n.extrasDiff.length || n.childrenDiff.length || n.bundle.source !== n.bundle.output || n.meshName.source !== n.meshName.output);
  console.log(`\n== ${r.label} ==`);
  console.log(` counts:`, JSON.stringify(r.counts));
  console.log(` extensionsUsed:`, JSON.stringify(r.extensionsUsed), ` scenes:`, JSON.stringify(r.scenes));
  console.log(` node order same: ${r.nodeOrder.same}; nodes with any diff: ${nodeIssues.length}/${r.nodes.length}`);
  for (const n of nodeIssues.slice(0, 30)) { if (n.missingInOutput || n.missingInSource) { console.log(`   - ${n.name}: ${n.missingInOutput ? "missing in output" : "only in output"}`); continue; } console.log(`   - ${n.name}: rd=${n.robotDataDiff.length} trs=${n.trsDiff.length} extras=${n.extrasDiff.length} children=${n.childrenDiff.length} bundle=${n.bundle.source}/${n.bundle.output} mesh=${n.meshName.source}/${n.meshName.output}`, n.robotDataDiff.slice(0, 3).map((d) => d.path).join(","), n.trsDiff.slice(0, 2).map((d) => `${d.path}:${d.a}->${d.b}`).join(","), n.extrasDiff.slice(0, 2).map((d) => `extras${d.path}:${JSON.stringify(d.a)}->${JSON.stringify(d.b)}`).join(","), n.childrenDiff.length ? `children ${JSON.stringify(n.childrenDiff.slice(0,2))}` : ""); }
  console.log(` bundle equal (JSON string): ${r.bundleEqualJson}; bundle diffs: ${r.bundleDiff.length}`);
  console.log(` mesh diffs: ${r.meshDiff.length}`, r.meshDiff.slice(0, 8).map((d) => `${d.path}:${JSON.stringify(d.a)}->${JSON.stringify(d.b)}`).join(" | "));
  console.log(` material diffs: ${r.materialDiff.length}`, r.materialDiff.slice(0, 8).map((d) => `${d.path}:${JSON.stringify(d.a)}->${JSON.stringify(d.b)}`).join(" | "));
  console.log(` material order same: ${JSON.stringify(r.materialOrder.source) === JSON.stringify(r.materialOrder.output)}`);
  console.log(` bytes: ${r.bytes.source} -> ${r.bytes.output}; json ${r.jsonChunk.source} -> ${r.jsonChunk.output}; bin ${r.binChunk.source} -> ${r.binChunk.output}`);
}
console.log(`\n== round trip (headless GLB reloaded) == world diffs: ${report.roundTrip.worldDiff.length}, animatable diffs: ${report.roundTrip.animatablesDiff.length}, bundle diffs: ${report.roundTrip.bundleDiff.length}`);
for (const d of report.roundTrip.worldDiff.slice(0, 10)) console.log(`   world ${d.path}: ${JSON.stringify(d.a)} -> ${JSON.stringify(d.b)}`);
console.log(` timings ms:`, JSON.stringify(Object.fromEntries(Object.entries(report.timingsMs).map(([k, v]) => [k, Math.round(v)]))), ` rss MB: ${report.memoryMB.rss.toFixed(0)}`);
