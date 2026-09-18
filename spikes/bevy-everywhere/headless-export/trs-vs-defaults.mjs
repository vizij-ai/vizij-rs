// Does a shipped face's node TRS / mesh weights equal its own RobotData defaults?
import { readFileSync } from "node:fs";
const buf = readFileSync(process.argv[2]);
const ab = buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength);
const len = new DataView(ab).getUint32(12, true);
const json = JSON.parse(new TextDecoder().decode(new Uint8Array(ab, 20, len)));
const num = (v) => typeof v === "number" ? v : v?.f32 ?? v?.f64 ?? undefined;
const vec = (d) => d?.struct?.fields ? d.struct.fields.map((f) => num(f.value)) : d && typeof d === "object" && "x" in d ? [d.x, d.y, d.z] : undefined;
const close = (a, b, tol = 1e-5) => a !== undefined && b !== undefined && Math.abs(a - b) <= tol;
function eulerToQuat(x, y, z) { // three.js Euler ZYX -> quaternion [x,y,z,w]
  const c1 = Math.cos(x / 2), c2 = Math.cos(y / 2), c3 = Math.cos(z / 2), s1 = Math.sin(x / 2), s2 = Math.sin(y / 2), s3 = Math.sin(z / 2);
  return [s1 * c2 * c3 - c1 * s2 * s3, c1 * s2 * c3 + s1 * c2 * s3, c1 * c2 * s3 - s1 * s2 * c3, c1 * c2 * c3 + s1 * s2 * s3];
}
let rows = [];
for (const n of json.nodes) {
  const rd = n.extensions?.RobotData; if (!rd) continue;
  const f = rd.features;
  const dT = f.translation?.animated ? vec(f.translation.value.default) : vec(f.translation?.value);
  const dR = f.rotation?.animated ? vec(f.rotation.value.default) : vec(f.rotation?.value);
  const dS = f.scale ? (f.scale.animated ? vec(f.scale.value.default) ?? num(f.scale.value.default) : f.scale.value) : undefined;
  const t = n.translation ?? [0, 0, 0], r = n.rotation ?? [0, 0, 0, 1], s = n.scale ?? [1, 1, 1];
  const tOk = dT ? dT.every((v, i) => close(v, t[i])) : null;
  const q = dR ? eulerToQuat(...dR) : null;
  const rOk = q ? (q.every((v, i) => close(v, r[i], 1e-4)) || q.every((v, i) => close(-v, r[i], 1e-4))) : null;
  const sArr = Array.isArray(dS) ? dS : typeof dS === "number" ? [dS, dS, dS] : undefined;
  const sOk = sArr ? sArr.every((v, i) => close(v, s[i])) : null;
  // morph weights vs feature defaults
  let wOk = null, wDetail = "";
  if (n.mesh !== undefined && rd.morphTargets?.length) {
    const w = json.meshes[n.mesh].weights ?? rd.morphTargets.map(() => 0);
    const defaults = rd.morphTargets.map((name) => { const ft = f[name]; return ft ? (ft.animated ? num(ft.value.default) : ft.value) : 0; });
    wOk = defaults.every((v, i) => close(v ?? 0, w[i] ?? 0));
    if (!wOk) wDetail = rd.morphTargets.map((name, i) => `${name}:${defaults[i]}->${w[i]}`).filter((x, i) => !close(defaults[i] ?? 0, w[i] ?? 0)).slice(0, 3).join(" ");
  }
  rows.push({ node: n.name, type: rd.type, translationAtDefault: tOk, rotationAtDefault: rOk, scaleAtDefault: sOk, weightsAtDefault: wOk, t: dT && !tOk ? `${JSON.stringify(dT)} vs ${JSON.stringify(t)}` : undefined, s: sArr && !sOk ? `${JSON.stringify(sArr)} vs ${JSON.stringify(s)}` : undefined, w: wDetail || undefined });
}
const off = rows.filter((r) => r.translationAtDefault === false || r.rotationAtDefault === false || r.scaleAtDefault === false || r.weightsAtDefault === false);
console.log(`${process.argv[2].split("/").pop()}: ${rows.length} RobotData nodes; ${off.length} with TRS/weights != own defaults`);
for (const r of off) console.log("  ", JSON.stringify(r));
