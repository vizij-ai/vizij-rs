import { readFileSync } from "node:fs";
const path = process.argv[2];
const buf = readFileSync(path);
const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
const view = new DataView(ab);
const len = view.getUint32(12, true);
const json = JSON.parse(new TextDecoder().decode(new Uint8Array(ab, 20, len)));
const nodes = json.nodes ?? [];
const withRD = nodes.filter(n => n.extensions?.RobotData);
const withBundle = nodes.filter(n => n.extensions?.VIZIJ_bundle);
const meshes = json.meshes ?? [];
const morph = meshes.filter(m => m.primitives?.some(p => p.targets?.length)).length;
console.log(JSON.stringify({
  file: path.split("/").pop(),
  bytes: buf.length, jsonChunkBytes: len,
  asset: json.asset,
  extensionsUsed: json.extensionsUsed, extensionsRequired: json.extensionsRequired,
  rootExtensions: Object.keys(json.extensions ?? {}),
  scenes: json.scenes?.map(s => ({ name: s.name, nodes: s.nodes, ext: Object.keys(s.extensions ?? {}) })),
  scene: json.scene,
  nodeCount: nodes.length, robotDataNodes: withRD.length, bundleNodes: withBundle.map(n => n.name),
  robotDataTypes: withRD.reduce((a, n) => { const t = n.extensions.RobotData.type; a[t] = (a[t] ?? 0) + 1; return a; }, {}),
  nodesWithoutRobotData: nodes.filter(n => !n.extensions?.RobotData).map(n => n.name),
  meshCount: meshes.length, meshesWithMorph: morph,
  morphTargetNamesSample: meshes.filter(m => m.extras?.targetNames).slice(0, 2).map(m => ({ name: m.name, targetNames: m.extras.targetNames })),
  materialCount: (json.materials ?? []).length,
  materialSample: (json.materials ?? []).slice(0, 3),
  images: (json.images ?? []).length, textures: (json.textures ?? []).length, samplers: (json.samplers ?? []).length,
  accessors: (json.accessors ?? []).length, bufferViews: (json.bufferViews ?? []).length, buffers: json.buffers,
  animations: (json.animations ?? []).length, skins: (json.skins ?? []).length, cameras: (json.cameras ?? []).length,
  bundleKeys: withBundle[0] ? Object.keys(withBundle[0].extensions.VIZIJ_bundle) : null,
  bundleBytes: withBundle[0] ? JSON.stringify(withBundle[0].extensions.VIZIJ_bundle).length : 0,
  sampleRobotData: withRD.slice(0, 1).map(n => n.extensions.RobotData),
}, null, 2));
