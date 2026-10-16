// A static server for the browser harness: the package root (for dist/ and
// the harness page), the face GLBs under /fixtures, and a stand-in for the
// TTS cloud function under /tts (four bytes of "audio" and three speech
// marks for any text), so a `say` on a page never reaches the network. A
// file's `?encoding=gzip` serves it gzip-compressed, `?type=<mime>` under
// another content type — what a misconfigured host does — and `?paced` in
// 256 KB chunks 10 ms apart, as a slow network delivers it.
import http from "node:http";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

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
      let body = await fs.readFile(file);
      const headers = {
        "content-type": url.searchParams.get("type") ?? types[path.extname(file)] ?? "application/octet-stream",
        "cache-control": "no-store",
      };
      if (url.searchParams.get("encoding") === "gzip") {
        body = gzipSync(body);
        headers["content-encoding"] = "gzip";
      }
      headers["content-length"] = body.length;
      res.writeHead(200, headers);
      if (url.searchParams.has("paced")) {
        const CHUNK = 256 * 1024;
        for (let at = 0; at < body.length; at += CHUNK) {
          res.write(body.subarray(at, at + CHUNK));
          await new Promise((resolve) => setTimeout(resolve, 10));
        }
        return res.end();
      }
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
