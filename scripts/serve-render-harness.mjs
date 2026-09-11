/**
 * Serves the render harness, with caching off.
 *
 * A rebuilt wasm keeps the same URL, so a cached copy is served against fresh
 * bindgen glue that expects different exports — which fails as a blank page
 * rather than as an error. No-store costs a re-download per reload and removes
 * the class of confusion entirely.
 */
import { createServer } from "node:http";
import { createReadStream, statSync } from "node:fs";
import { extname, join, normalize, resolve } from "node:path";

const dir = resolve(process.cwd(), "crates/render/vizij-render-web/harness");
const port = Number(process.env.PORT ?? 8391);

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".glb": "model/gltf-binary",
  ".json": "application/json",
  ".ts": "text/plain; charset=utf-8",
};

createServer((request, response) => {
  const path = decodeURIComponent(new URL(request.url, "http://localhost").pathname);
  // Resolve inside the harness directory only; a traversal outside it is a 403
  // rather than a read of whatever the path escapes to.
  const file = join(dir, normalize(path === "/" ? "/index.html" : path));
  if (!file.startsWith(dir)) {
    response.writeHead(403).end("outside the harness directory");
    return;
  }
  let size;
  try {
    size = statSync(file).size;
  } catch {
    response.writeHead(404).end("not found");
    return;
  }
  response.writeHead(200, {
    "content-type": TYPES[extname(file)] ?? "application/octet-stream",
    "content-length": size,
    "cache-control": "no-store, must-revalidate",
  });
  createReadStream(file).pipe(response);
}).listen(port, () => {
  console.log(
    `harness on http://localhost:${port}/  (no-store; a plain reload picks up a rebuild)`,
  );
});
