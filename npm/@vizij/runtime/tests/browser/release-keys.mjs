// `releaseKeys` in a page settles on a device nothing steps
// (`../release-keys-scenario.mjs`, as the Node suite runs it).
import { open } from "./common.mjs";

const { page, logs, close } = await open();
try {
  await page.evaluate(async () => {
    const vizij = await import("../../dist/runtime/src/index.js");
    const { releaseKeysSettles } = await import("../release-keys-scenario.mjs");
    await releaseKeysSettles(vizij);
  });
  console.log("@vizij/runtime browser releaseKeys: ok");
} catch (e) {
  console.error(logs.slice(-30).join("\n"));
  throw e;
} finally {
  await close();
}
