import type { AnimationSetup, GraphRegistrationConfig } from "./types.js";

type OrchestrationsModule = typeof import("@vizij/test-fixtures")["orchestrations"];

let loader: Promise<OrchestrationsModule> | null = null;

function fixturesModule(): Promise<OrchestrationsModule> {
  if (!loader) {
    loader = import("@vizij/test-fixtures")
      .then((mod) => mod.orchestrations)
      .catch((err) => {
        throw new Error(
          `Failed to load @vizij/test-fixtures. Install the workspace package alongside @vizij/orchestrator-wasm to access shared samples. Original error: ${err instanceof Error ? err.message : String(err)}`,
        );
      });
  }
  return loader;
}

/** List orchestration fixture keys available via the shared manifest. */
export async function listOrchestrationFixtures(): Promise<string[]> {
  const module = await fixturesModule();
  return module.orchestrationNames();
}

/** Load the raw descriptor JSON value for the given orchestration fixture key. */
export async function loadOrchestrationDescriptor<T = unknown>(name: string): Promise<T> {
  const module = await fixturesModule();
  return module.orchestrationDescriptor<T>(name);
}

/** Load the orchestration descriptor as a JSON string. */
export async function loadOrchestrationJson(name: string): Promise<string> {
  const module = await fixturesModule();
  return module.orchestrationJson(name);
}

/**
 * Load a complete orchestration bundle containing the descriptor, resolved animation,
 * graph spec, and optional staged inputs. Useful for bootstrapping integration tests.
 */
type PipelineDescriptor = {
  animation: string;
  graph: string;
  initial_inputs?: Array<{ path: string; value: unknown }>;
  steps?: Array<{ delta: number; expect: Record<string, unknown> }>;
  [key: string]: unknown;
};

export async function loadOrchestrationBundle(
  name: string,
): Promise<{
  descriptor: PipelineDescriptor;
  animation: AnimationSetup["animation"];
  graphSpec: GraphRegistrationConfig["spec"];
}> {
  const module = await fixturesModule();
  const bundle = module.loadOrchestrationBundle(name) as {
    descriptor: PipelineDescriptor;
    animation: AnimationSetup["animation"];
    graphSpec: unknown;
  };
  return {
    descriptor: bundle.descriptor,
    animation: bundle.animation,
    graphSpec: bundle.graphSpec as GraphRegistrationConfig["spec"],
  };
}
