import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const typedocDir = resolve(rootDir, "target/docs-typedoc");
const rustdocDir = resolve(rootDir, "target/docs-rust/doc");
const siteDir = resolve(rootDir, "target/docs-site");

if (!existsSync(typedocDir)) {
  throw new Error(
    "Missing TypeDoc output at target/docs-typedoc. Run `pnpm run docs:typedoc` first."
  );
}

if (!existsSync(rustdocDir)) {
  throw new Error(
    "Missing rustdoc output at target/docs-rust/doc. Run `pnpm run docs:rust` first."
  );
}

rmSync(siteDir, { recursive: true, force: true });
mkdirSync(siteDir, { recursive: true });

cpSync(typedocDir, resolve(siteDir, "npm"), { recursive: true });
cpSync(rustdocDir, resolve(siteDir, "rust"), { recursive: true });
writeFileSync(resolve(siteDir, ".nojekyll"), "");

const rustSections = [
  {
    title: "API Stack",
    items: [
      ["vizij-api-core", "rust/vizij_api_core/"],
      ["vizij-api-wasm", "rust/vizij_api_wasm/"],
      ["bevy_vizij_api", "rust/bevy_vizij_api/"]
    ]
  },
  {
    title: "Animation Stack",
    items: [
      ["vizij-animation-core", "rust/vizij_animation_core/"],
      ["vizij-animation-wasm", "rust/vizij_animation_wasm/"],
      ["bevy_vizij_animation", "rust/bevy_vizij_animation/"]
    ]
  },
  {
    title: "Node Graph Stack",
    items: [
      ["vizij-graph-core", "rust/vizij_graph_core/"],
      ["vizij-graph-wasm", "rust/vizij_graph_wasm/"],
      ["bevy_vizij_graph", "rust/bevy_vizij_graph/"]
    ]
  },
  {
    title: "Orchestrator & Fixtures",
    items: [
      ["vizij-orchestrator-core", "rust/vizij_orchestrator/"],
      ["vizij-orchestrator-wasm", "rust/vizij_orchestrator_wasm/"],
      ["vizij-test-fixtures", "rust/vizij_test_fixtures/"]
    ]
  }
];

const npmSections = [
  {
    title: "WASM Wrappers",
    items: [
      ["@vizij/animation-wasm", "npm/modules/animation-wasm_src.html"],
      ["@vizij/node-graph-wasm", "npm/modules/node-graph-wasm_src.html"],
      ["@vizij/orchestrator-wasm", "npm/modules/orchestrator-wasm_src.html"]
    ]
  },
  {
    title: "Support Packages",
    items: [
      ["@vizij/value-json", "npm/modules/value-json_src.html"],
      ["@vizij/test-fixtures", "npm/modules/test-fixtures_src.html"],
      ["@vizij/wasm-loader", "npm/modules/wasm-loader_src.html"],
      ["@vizij/wasm-loader/browser", "npm/modules/wasm-loader_src_browser.html"],
      ["@vizij/wasm-loader/shared", "npm/modules/wasm-loader_src_shared.html"]
    ]
  }
];

function renderSection(section) {
  const links = section.items
    .map(
      ([label, href]) =>
        `<li><a href="${href}">${label}</a></li>`
    )
    .join("");
  return `<section><h2>${section.title}</h2><ul>${links}</ul></section>`;
}

function renderMiniSection(section) {
  const links = section.items
    .map(([label, href]) => `<li><a href="../${href}">${label}</a></li>`)
    .join("");
  return `<section><h2>${section.title}</h2><ul>${links}</ul></section>`;
}

const html = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Vizij RS API Docs</title>
    <style>
      :root {
        color-scheme: light;
        --bg: #f4f1eb;
        --panel: rgba(255, 255, 255, 0.82);
        --text: #17211d;
        --muted: #4e6158;
        --line: rgba(23, 33, 29, 0.12);
        --accent: #16634b;
        --accent-soft: #d7efe5;
        --shadow: 0 18px 50px rgba(23, 33, 29, 0.08);
      }
      * { box-sizing: border-box; }
      body {
        margin: 0;
        font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
        color: var(--text);
        background:
          radial-gradient(circle at top left, rgba(22, 99, 75, 0.12), transparent 32rem),
          linear-gradient(180deg, #fbf8f2 0%, var(--bg) 100%);
      }
      main {
        max-width: 1100px;
        margin: 0 auto;
        padding: 72px 24px 96px;
      }
      .hero {
        background: var(--panel);
        border: 1px solid var(--line);
        border-radius: 28px;
        padding: 32px;
        box-shadow: var(--shadow);
        backdrop-filter: blur(12px);
      }
      h1 {
        margin: 0 0 12px;
        font-family: "IBM Plex Serif", Georgia, serif;
        font-size: clamp(2.2rem, 4vw, 3.8rem);
        line-height: 1.05;
      }
      p {
        margin: 0;
        max-width: 70ch;
        color: var(--muted);
        line-height: 1.6;
      }
      .hero-links,
      .grid {
        display: grid;
        gap: 20px;
      }
      .hero-links {
        margin-top: 28px;
        grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
      }
      .hero-links a,
      .grid a {
        color: var(--accent);
        text-decoration: none;
      }
      .hero-links a:hover,
      .grid a:hover {
        text-decoration: underline;
      }
      .hero-links article,
      .grid section {
        background: rgba(255, 255, 255, 0.7);
        border: 1px solid var(--line);
        border-radius: 22px;
        padding: 20px 22px;
      }
      .hero-links h2,
      .grid h2 {
        margin: 0 0 10px;
        font-size: 1.05rem;
      }
      .hero-links p {
        font-size: 0.96rem;
      }
      .grid {
        margin-top: 28px;
        grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
      }
      ul {
        margin: 0;
        padding-left: 18px;
        color: var(--muted);
      }
      li + li {
        margin-top: 8px;
      }
      .eyebrow {
        display: inline-block;
        margin-bottom: 16px;
        padding: 6px 10px;
        border-radius: 999px;
        background: var(--accent-soft);
        color: var(--accent);
        font-size: 0.84rem;
        font-weight: 600;
        letter-spacing: 0.02em;
      }
    </style>
  </head>
  <body>
    <main>
      <section class="hero">
        <span class="eyebrow">Vizij Runtime Docs</span>
        <h1>Rust crates and npm wrappers, published together.</h1>
        <p>
          This site bundles native rustdoc output for the runtime crates and TypeDoc output for
          the public npm packages. Start with the top-level API indexes, then drill into the stack
          pages below for direct entry points.
        </p>
        <div class="hero-links">
          <article>
            <h2><a href="rust/">Rust API Index</a></h2>
            <p>Generated with cargo doc from the public Vizij crates, including unpublished workspace crates.</p>
          </article>
          <article>
            <h2><a href="npm/">npm API Index</a></h2>
            <p>Generated with TypeDoc from the published wrapper and support package entrypoints.</p>
          </article>
        </div>
      </section>
      <div class="grid">
        ${rustSections.map(renderSection).join("")}
        ${npmSections.map(renderSection).join("")}
      </div>
    </main>
  </body>
</html>
`;

writeFileSync(resolve(siteDir, "index.html"), html);

const rustIndexHtml = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Vizij Rust API Docs</title>
    <style>
      :root {
        color-scheme: light;
        --bg: #f7f4ee;
        --panel: #ffffff;
        --text: #17211d;
        --muted: #4e6158;
        --line: rgba(23, 33, 29, 0.12);
        --accent: #16634b;
      }
      * { box-sizing: border-box; }
      body {
        margin: 0;
        font-family: "IBM Plex Sans", "Segoe UI", sans-serif;
        color: var(--text);
        background: linear-gradient(180deg, #fbf8f2 0%, var(--bg) 100%);
      }
      main {
        max-width: 980px;
        margin: 0 auto;
        padding: 56px 24px 80px;
      }
      header {
        margin-bottom: 24px;
      }
      h1 {
        margin: 0 0 12px;
        font-family: "IBM Plex Serif", Georgia, serif;
        font-size: clamp(2rem, 4vw, 3rem);
      }
      p {
        margin: 0;
        color: var(--muted);
        line-height: 1.6;
        max-width: 68ch;
      }
      .grid {
        display: grid;
        gap: 18px;
        grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
        margin-top: 24px;
      }
      section {
        background: var(--panel);
        border: 1px solid var(--line);
        border-radius: 20px;
        padding: 20px;
      }
      h2 {
        margin: 0 0 10px;
        font-size: 1.02rem;
      }
      ul {
        margin: 0;
        padding-left: 18px;
      }
      li + li {
        margin-top: 8px;
      }
      a {
        color: var(--accent);
        text-decoration: none;
      }
      a:hover {
        text-decoration: underline;
      }
      .back {
        display: inline-block;
        margin-top: 24px;
        color: var(--muted);
      }
    </style>
  </head>
  <body>
    <main>
      <header>
        <h1>Rust API Docs</h1>
        <p>
          Native rustdoc output for the public Vizij crates. Start here if you want trait impls,
          re-exports, and crate-level module structure rather than the npm wrapper surface.
        </p>
      </header>
      <div class="grid">
        ${rustSections.map(renderMiniSection).join("")}
      </div>
      <a class="back" href="../index.html">Back to docs home</a>
    </main>
  </body>
</html>
`;

writeFileSync(resolve(siteDir, "rust/index.html"), rustIndexHtml);

console.log(`Docs site assembled at ${siteDir}`);
