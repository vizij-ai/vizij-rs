import path from "path";
import fs from "fs";
const specPath = process.argv[2];
if (!specPath) {
  console.error("Usage: node run-graph.mjs /path/to/graph-spec.json");
  process.exit(2);
}
const modulePath = path.join(path.dirname(new URL(import.meta.url).pathname), 'dist/src/index.js');
const { init, createGraph, normalizeGraphSpec } = await import(modulePath);
const spec = JSON.parse(fs.readFileSync(specPath, "utf8"));
await init();
const normalized = await normalizeGraphSpec(spec);
const graph = await createGraph(normalized);
const result = graph.evalAll();
console.log(JSON.stringify(result.nodes.pose_blend, null, 2));
