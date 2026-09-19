// A static server for the browser harness: the package root (for dist/ and
// the harness page) and the face GLBs under /fixtures.
import http from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
const types = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".mjs": "text/javascript",
  ".wasm": "application/wasm",
  ".glb": "model/gltf-binary",
  ".json": "application/json",
};

/** Serve `packageRoot` and `fixturesDir` (as `/fixtures`); resolves to the base URL. */
export function serve(fixturesDir) {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, "http://localhost");
    const file = url.pathname.startsWith("/fixtures/")
      ? path.join(fixturesDir, url.pathname.slice("/fixtures/".length))
      : path.join(packageRoot, url.pathname);
    try {
      const body = await fs.readFile(file);
      res.writeHead(200, {
        "content-type": types[path.extname(file)] ?? "application/octet-stream",
        "cache-control": "no-store",
      });
      res.end(body);
    } catch {
      res.writeHead(404);
      res.end();
    }
  });
  return new Promise((resolve) => {
    server.listen(0, "127.0.0.1", () => {
      resolve({ base: `http://127.0.0.1:${server.address().port}`, close: () => server.close() });
    });
  });
}
