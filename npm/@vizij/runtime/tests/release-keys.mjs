// `releaseKeys` settles on a device nothing steps, through the published
// wrapper surface (dist/): `./release-keys-scenario.mjs`, in Node.
import * as vizij from "../dist/runtime/src/index.js";
import { releaseKeysSettles } from "./release-keys-scenario.mjs";

await releaseKeysSettles(vizij);
console.log("@vizij/runtime releaseKeys: ok");
process.exit(0);
