import type { GraphSpec } from "./types.js";

type NodeGraphModule = typeof import("@vizij/test-fixtures")["nodeGraphs"];

let loader: Promise<NodeGraphModule> | null = null;

function fixturesModule(): Promise<NodeGraphModule> {
  if (!loader) {
    loader = import("@vizij/test-fixtures")
      .then((mod) => mod.nodeGraphs)
      .catch((err) => {
        throw new Error(
          `Failed to load @vizij/test-fixtures. Install the workspace package alongside @vizij/node-graph-wasm to access shared samples. Original error: ${err instanceof Error ? err.message : String(err)}`,
        );
      });
  }
  return loader;
}

/** List the node-graph fixture keys available via the shared manifest. */
export async function listNodeGraphFixtures(): Promise<string[]> {
  const module = await fixturesModule();
  return module.nodeGraphNames();
}

/** Load the GraphSpec for the given shared fixture key. */
export async function loadNodeGraphSpec(name: string): Promise<GraphSpec> {
  const module = await fixturesModule();
  const raw = module.nodeGraphSpec<unknown>(name);
  if (raw && typeof raw === "object") {
    if ("nodes" in (raw as Record<string, unknown>)) {
      return raw as GraphSpec;
    }
    if ("spec" in (raw as Record<string, unknown>)) {
      const spec = (raw as Record<string, unknown>).spec;
      if (spec && typeof spec === "object") {
        return spec as GraphSpec;
      }
    }
  }
  throw new Error(`Fixture '${name}' did not contain a GraphSpec-compatible payload`);
}

/** Load the GraphSpec JSON string for the given shared fixture key. */
export async function loadNodeGraphSpecJson(name: string): Promise<string> {
  const module = await fixturesModule();
  return module.nodeGraphSpecJson(name);
}

/** Load any staged input bundle associated with the node-graph fixture. */
export async function loadNodeGraphStage<T = unknown>(name: string): Promise<T | null> {
  const module = await fixturesModule();
  return module.nodeGraphStage<T>(name);
}

/**
 * Load both the GraphSpec and optional stage inputs for a shared node-graph fixture.
 * Returns `{ spec }`; stage data is no longer bundled with fixtures and should be provided per usage site.
 */
export async function loadNodeGraphBundle(name: string): Promise<{ spec: GraphSpec }> {
  const spec = await loadNodeGraphSpec(name);
  return { spec };
}
