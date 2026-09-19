// A static server for the browser harness: the package root (for dist/ and
// the harness page), the face GLBs under /fixtures, and a stand-in for the
// TTS cloud function under /tts (four bytes of "audio" and three speech
// marks for any text), so a `say` on a page never reaches the network.
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
    if (url.pathname === "/tts/get-audio" || url.pathname === "/tts/get-visemes") {
      const headers = { "access-control-allow-origin": "*", "access-control-allow-headers": "*" };
      if (req.method === "OPTIONS") {
        res.writeHead(204, headers);
        return res.end();
      }
      if (url.pathname === "/tts/get-audio") {
        res.writeHead(200, { ...headers, "content-type": "audio/mpeg" });
        return res.end(Buffer.from([1, 2, 3, 4]));
      }
      res.writeHead(200, { ...headers, "content-type": "application/json" });
      return res.end(
        JSON.stringify({
          visemes: [
            { time: 0, value: "p" },
            { time: 100, value: "a" },
            { time: 200, value: "sil" },
          ],
        }),
      );
    }
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
