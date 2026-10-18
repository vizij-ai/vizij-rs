// `loadGraph` and `applyGraphEdits` settle on a device nothing steps, through
// the published wrapper surface (dist/): the scenario the browser suite runs
// too.
import * as vizij from "../dist/runtime/src/index.js";
import { loadGraphSettles } from "./load-graph-scenario.mjs";

await loadGraphSettles(vizij);
console.log("@vizij/runtime loadGraph: ok");
process.exit(0);
