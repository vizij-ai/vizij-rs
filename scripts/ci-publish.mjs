#!/usr/bin/env node

// Publish every publishable @vizij/* package whose version is not yet on npm.
//
// The decision is per package, against the registry alone: a version already
// on npm is skipped, a version absent from an existing package is built and
// published, and a package npm has never seen is reported as needing its
// one-time manual first publish (npm's trusted publishing cannot create a
// package). One package failing does not stop the others; the run fails if a
// package that could have been published was not. Re-runs are safe.
//
// Only the packages being published are built, a wasm wrapper from its crates
// first (`build:wasm <target>`). A prerelease version (3.0.0-alpha.0)
// publishes under its own dist-tag, so `latest` keeps pointing at the last
// release. Internal `workspace:` ranges are materialised to real versions for
// the duration of the publish and restored afterwards.
//
//   node scripts/ci-publish.mjs             publish what npm is missing
//   node scripts/ci-publish.mjs --dry-run   list it, publish nothing
//   PUBLISH_PACKAGE=runtime node …          consider @vizij/runtime only
//
// Under GitHub Actions, --dry-run also writes `pending=true|false` to
// $GITHUB_OUTPUT, so the workflow installs the toolchains only when something
// will be published.

import { spawnSync } from "node:child_process";
import { promises as fs } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { applyWorkspaceManifestUpdates, restoreWorkspaceManifests } from "./prepare-publish-manifests.mjs";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const PACKAGES_ROOT = path.resolve(REPO_ROOT, "npm/@vizij");
const DRY_RUN = process.argv.includes("--dry-run");

/**
 * Wasm build target for each wasm wrapper package (`build:wasm <target>`).
 * The other packages need no wasm build step.
 */
const WASM_TARGETS = {
  animation: "animation",
  "node-graph": "graph",
  runtime: "runtime",
};

function run(cmd, args, opts = {}) {
  return spawnSync(cmd, args, { encoding: "utf8", cwd: REPO_ROOT, ...opts });
}

/** The publishable packages, each after the internal packages it depends on. */
async function publishablePackages() {
  const pkgs = [];
  for (const entry of await fs.readdir(PACKAGES_ROOT, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const dir = path.resolve(PACKAGES_ROOT, entry.name);
    let json;
    try {
      json = JSON.parse(await fs.readFile(path.resolve(dir, "package.json"), "utf8"));
    } catch {
      continue;
    }
    if (json.private === true || !json.name || !json.version) continue;
    const deps = Object.keys({ ...json.dependencies, ...json.peerDependencies });
    pkgs.push({ name: json.name, short: entry.name, version: json.version, dir, deps });
  }
  const byName = new Map(pkgs.map((pkg) => [pkg.name, pkg]));
  const ordered = [];
  const visit = (pkg) => {
    if (ordered.includes(pkg)) return;
    for (const dep of pkg.deps) {
      if (byName.has(dep)) visit(byName.get(dep));
    }
    ordered.push(pkg);
  };
  for (const pkg of pkgs.sort((a, b) => a.name.localeCompare(b.name))) visit(pkg);
  return ordered;
}

/** "published" | "unpublished" | "absent": the version, the package, or neither. */
function registryStatus(name, version) {
  const atVersion = run("npm", ["view", `${name}@${version}`, "version"]);
  if (atVersion.status === 0 && atVersion.stdout.trim()) return "published";
  const anyVersion = run("npm", ["view", name, "version"]);
  return anyVersion.status === 0 ? "unpublished" : "absent";
}

/** `alpha` for 3.0.0-alpha.0; none for a release. */
function prereleaseTag(version) {
  return /^\d+\.\d+\.\d+-([a-z]+)/i.exec(version)?.[1];
}

function step(cmd, args) {
  console.log(`$ ${cmd} ${args.join(" ")}`);
  return run(cmd, args, { stdio: "inherit" }).status === 0;
}

async function main() {
  const only = process.env.PUBLISH_PACKAGE?.trim();
  const pkgs = (await publishablePackages()).filter((pkg) => !only || pkg.short === only);
  if (only && pkgs.length === 0) {
    throw new Error(`PUBLISH_PACKAGE=${only} names no publishable package under npm/@vizij/`);
  }

  const results = pkgs.map((pkg) => {
    const status = registryStatus(pkg.name, pkg.version);
    if (status === "published") return { pkg, outcome: "skip", note: "already on npm" };
    if (status === "absent") {
      return { pkg, outcome: "bootstrap", note: "not on npm; needs a one-time manual first publish" };
    }
    return { pkg, outcome: DRY_RUN ? "would-publish" : "pending" };
  });
  const glyph = { skip: "=", bootstrap: "!", "would-publish": "+", pending: "+" };
  for (const r of results) {
    const tag = prereleaseTag(r.pkg.version);
    console.log(
      `${glyph[r.outcome]} ${r.pkg.name}@${r.pkg.version} — ${r.outcome}` +
        (r.note ? ` (${r.note})` : "") +
        (tag && r.outcome !== "skip" ? ` [dist-tag ${tag}]` : ""),
    );
  }

  const pending = results.filter((r) => r.outcome === "pending" || r.outcome === "would-publish");
  if (DRY_RUN && process.env.GITHUB_OUTPUT) {
    await fs.appendFile(process.env.GITHUB_OUTPUT, `pending=${pending.length > 0}\n`);
  }

  if (!DRY_RUN && pending.length > 0) {
    if (!step("pnpm", ["run", "build:shared"])) throw new Error("build:shared failed; not publishing.");
    for (const r of pending) {
      const target = WASM_TARGETS[r.pkg.short];
      const built =
        (!target || step("pnpm", ["run", "build:wasm", "--", target])) &&
        step("pnpm", ["--filter", r.pkg.name, "run", "build"]);
      if (!built) {
        r.outcome = "failed";
        console.error(`Failed to build ${r.pkg.name}@${r.pkg.version} (continuing).`);
      }
    }
    await applyWorkspaceManifestUpdates();
    try {
      for (const r of pending.filter((r) => r.outcome === "pending")) {
        const tag = prereleaseTag(r.pkg.version);
        console.log(`Publishing ${r.pkg.name}@${r.pkg.version}${tag ? ` under ${tag}` : ""}…`);
        const published = run(
          "npm",
          ["publish", "--access", "public", "--provenance", "--ignore-scripts", ...(tag ? ["--tag", tag] : [])],
          { cwd: r.pkg.dir, stdio: "inherit" },
        );
        r.outcome = published.status === 0 ? "published" : "failed";
        if (published.status !== 0) console.error(`Failed to publish ${r.pkg.name}@${r.pkg.version} (continuing).`);
      }
    } finally {
      await restoreWorkspaceManifests();
    }
  }

  const by = (outcome) => results.filter((r) => r.outcome === outcome);
  console.log(
    `\npublished ${by("published").length} · skipped ${by("skip").length} · ` +
      `bootstrap ${by("bootstrap").length} · failed ${by("failed").length}` +
      (DRY_RUN ? ` · would-publish ${by("would-publish").length}` : ""),
  );
  if (by("bootstrap").length) {
    console.log(
      "\nNeed a one-time manual first publish (trusted publishing cannot create a package):\n" +
        by("bootstrap")
          .map((r) => `  - ${r.pkg.name}@${r.pkg.version}`)
          .join("\n"),
    );
  }
  if (by("failed").length) {
    console.error(`\n${by("failed").length} package(s) failed.`);
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
