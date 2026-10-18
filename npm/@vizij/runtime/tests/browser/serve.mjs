// A static server for the browser harness: the package root (for dist/ and
// the harness page), the face GLBs under /fixtures, and two stand-ins for
// the TTS cloud function, so a `say` on a page never reaches the network:
// under /tts, four bytes of "audio" and three speech marks 100 ms apart for
// any text; under /audible/tts, audio the page's own player decodes — a WAV
// of silence, two seconds for each word of the text — and the same marks two
// seconds apart. Two seconds is more than a step of the slowest page a test
// drives (one a second), so whatever the phase of its steps against the
// audio, a poll lands within each mark's span. A file's `?encoding=gzip`
// serves it gzip-compressed, `?type=<mime>` under another content type —
// what a misconfigured host does — and `?paced` in 256 KB chunks 10 ms
// apart, as a slow network delivers it.
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

/** A request's body, as text. */
async function body(req) {
  const chunks = [];
  for await (const chunk of req) chunks.push(chunk);
  return Buffer.concat(chunks).toString("utf8");
}

/** `seconds` of silence as a WAV: 8 kHz, mono, 8-bit PCM. */
function silence(seconds) {
  const rate = 8000;
  const samples = rate * seconds;
  const wav = Buffer.alloc(44 + samples, 0x80);
  wav.write("RIFF", 0);
  wav.writeUInt32LE(36 + samples, 4);
  wav.write("WAVEfmt ", 8);
  wav.writeUInt32LE(16, 16); // the fmt chunk's size
  wav.writeUInt16LE(1, 20); // PCM
  wav.writeUInt16LE(1, 22); // mono
  wav.writeUInt32LE(rate, 24);
  wav.writeUInt32LE(rate, 28); // bytes per second
  wav.writeUInt16LE(1, 32); // bytes per frame
  wav.writeUInt16LE(8, 34); // bits per sample
  wav.write("data", 36);
  wav.writeUInt32LE(samples, 40);
  return wav;
}

/** Serve `packageRoot` and `fixturesDir` (as `/fixtures`); resolves to the base URL. */
export function serve(fixturesDir) {
  const server = http.createServer(async (req, res) => {
    const url = new URL(req.url, "http://localhost");
    const tts = url.pathname.match(/^(\/audible)?\/tts\/(get-audio|get-visemes)$/);
    if (tts) {
      const [, audible, endpoint] = tts;
      const headers = { "access-control-allow-origin": "*", "access-control-allow-headers": "*" };
      if (req.method === "OPTIONS") {
        res.writeHead(204, headers);
        return res.end();
      }
      if (endpoint === "get-audio" && audible) {
        const { text } = JSON.parse(await body(req));
        res.writeHead(200, { ...headers, "content-type": "audio/wav" });
        return res.end(silence(2 * text.trim().split(/\s+/).length));
      }
      if (endpoint === "get-audio") {
        res.writeHead(200, { ...headers, "content-type": "audio/mpeg" });
        return res.end(Buffer.from([1, 2, 3, 4]));
      }
      const apart = audible ? 2000 : 100;
      res.writeHead(200, { ...headers, "content-type": "application/json" });
      return res.end(
        JSON.stringify({
          visemes: [
            { time: 0, value: "p" },
            { time: apart, value: "a" },
            { time: 2 * apart, value: "sil" },
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
