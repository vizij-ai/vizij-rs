import { readFileSync } from "node:fs";
const buf = readFileSync(process.argv[2]);
const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
const view = new DataView(ab);
const len = view.getUint32(12, true);
const json = JSON.parse(new TextDecoder().decode(new Uint8Array(ab, 20, len)));
const forms = {};
const formOf = (d) => d === null ? "null" : typeof d !== "object" ? typeof d : Object.keys(d).sort().join(",");
const perNode = [];
for (const n of json.nodes) {
  const rd = n.extensions?.RobotData; if (!rd) continue;
  const feats = {};
  for (const [k, f] of Object.entries(rd.features ?? {})) {
    const d = f.animated ? f.value?.default : f.value;
    const form = (f.animated ? "anim:" : "static:") + formOf(d) + (f.animated ? ":" + f.value?.type : "");
    forms[form] = (forms[form] ?? 0) + 1;
    feats[k] = { form, label: f.label, hasConstraints: f.animated ? "constraints" in (f.value ?? {}) : undefined, hasPub: f.animated ? "pub" in (f.value ?? {}) : undefined };
  }
  perNode.push({ name: n.name, type: rd.type, material: rd.material, morphTargets: rd.morphTargets?.length, tags: rd.tags, extraKeys: Object.keys(rd).filter(k => !["id","name","type","tags","features","material","morphTargets","rootBounds","root","children"].includes(k)), root: rd.root, rootBounds: rd.rootBounds, childrenIds: rd.children?.length, trs: { t: n.translation, r: n.rotation, s: n.scale }, extras: n.extras, mesh: n.mesh, children: n.children, featureKeys: Object.keys(feats) });
}
console.log(JSON.stringify({ forms, perNode }, null, 1));
