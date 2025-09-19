import type { GraphSpec } from "./types";

/**
 * Samples compatible with the updated core:
 * - Explicit Output nodes with typed paths so writes are produced
 * - Demonstrates selector projections and the Input node (using defaults via params.value)
 * - Designed to run standalone without host staging
 */

/**
 * Oscillator Basics
 * Time/Slider → Oscillator → Clamp/Remap → Output
 */
export const oscillatorBasics: GraphSpec = {
  nodes: [
    { id: "time", type: "time" },
    { id: "freq", type: "slider", params: { value: 0.5, min: 0, max: 2 } },
    {
      id: "osc",
      type: "oscillator",
      inputs: {
        frequency: { node_id: "freq" },
        phase: { node_id: "time" },
      },
    },
    { id: "offset", type: "constant", params: { value: 0.3 } },
    {
      id: "add1",
      type: "add",
      inputs: { a: { node_id: "osc" }, b: { node_id: "offset" } },
    },
    { id: "const0", type: "constant", params: { value: 0 } },
    { id: "clamp_max", type: "constant", params: { value: 1 } },
    {
      id: "clamp1",
      type: "clamp",
      inputs: {
        in: { node_id: "add1" },
        min: { node_id: "const0" },
        max: { node_id: "clamp_max" },
      },
    },
    { id: "remap_in_min", type: "constant", params: { value: 0 } },
    { id: "remap_in_max", type: "constant", params: { value: 1 } },
    { id: "remap_out_min", type: "constant", params: { value: -1 } },
    { id: "remap_out_max", type: "constant", params: { value: 1 } },
    {
      id: "remap1",
      type: "remap",
      inputs: {
        in: { node_id: "clamp1" },
        in_min: { node_id: "remap_in_min" },
        in_max: { node_id: "remap_in_max" },
        out_min: { node_id: "remap_out_min" },
        out_max: { node_id: "remap_out_max" },
      },
    },
    {
      id: "out",
      type: "output",
      params: { path: "samples/oscillator.signal" },
      inputs: { in: { node_id: "remap1" } },
    },
  ],
};

/**
 * Vector Playground (with Input nodes for v1 and v2)
 * - Two Input nodes provide default vectors.
 * - Demonstrates vector add/normalize/dot/length.
 * - Three Output sinks publish the results.
 */
export const vectorPlayground: GraphSpec = {
  nodes: [
    {
      id: "v1_in",
      type: "input",
      params: {
        path: "samples/vector.v1",
        value: { vec3: [1, 2, 3] },
      },
    },
    {
      id: "v2_in",
      type: "input",
      params: {
        path: "samples/vector.v2",
        value: { vec3: [0, 1, 0] },
      },
    },
    {
      id: "vadd",
      type: "vectoradd",
      inputs: { a: { node_id: "v1_in" }, b: { node_id: "v2_in" } },
    },
    {
      id: "vnorm",
      type: "vectornormalize",
      inputs: { in: { node_id: "v2_in" } },
    },
    {
      id: "vdot",
      type: "vectordot",
      inputs: { a: { node_id: "vadd" }, b: { node_id: "vnorm" } },
    },
    {
      id: "vlen",
      type: "vectorlength",
      inputs: { in: { node_id: "vadd" } },
    },
    // Publish each result via separate Output nodes so writes are explicit
    {
      id: "out_sum",
      type: "output",
      params: { path: "samples/vector.sum" },
      inputs: { in: { node_id: "vadd" } },
    },
    {
      id: "out_dot",
      type: "output",
      params: { path: "samples/vector.dot" },
      inputs: { in: { node_id: "vdot" } },
    },
    {
      id: "out_len",
      type: "output",
      params: { path: "samples/vector.len" },
      inputs: { in: { node_id: "vlen" } },
    },
  ],
};

/**
 * Logic Gate
 * Time → Sin → GreaterThan → If → Output
 */
export const logicGate: GraphSpec = {
  nodes: [
    { id: "time", type: "time" },
    { id: "sin", type: "sin", inputs: { in: { node_id: "time" } } },
    { id: "threshold", type: "constant", params: { value: 0 } },
    {
      id: "greater",
      type: "greaterthan",
      inputs: { lhs: { node_id: "sin" }, rhs: { node_id: "threshold" } },
    },
    { id: "then", type: "constant", params: { value: 1 } },
    { id: "else", type: "constant", params: { value: -1 } },
    {
      id: "gate",
      type: "if",
      inputs: {
        cond: { node_id: "greater" },
        then: { node_id: "then" },
        else: { node_id: "else" },
      },
    },
    {
      id: "out",
      type: "output",
      params: { path: "samples/logic.gated" },
      inputs: { in: { node_id: "gate" } },
    },
  ],
};

/**
 * Tuple Spring/Damp/Slew Sample
 *
 * An Input node provides a tuple [pos: Vec3, rot: Vec3] via params.value.
 * We project pos/rot via selectors into Spring/Damp/Slew nodes independently, then publish
 * three outputs by concatenating the processed vectors:
 *   [pos.x,pos.y,pos.z, rot.x,rot.y,rot.z]
 */
export const tupleSpringDampSlew: GraphSpec = {
  nodes: [
    {
      id: "pair",
      type: "input",
      params: {
        path: "samples/pair",
        value: {
          tuple: [{ vec3: [0.2, 0.1, 0.0] }, { vec3: [0.0, 0.0, 1.0] }],
        },
      },
    },

    // Spring over pos (index 0) and rot (index 1)
    {
      id: "spring_pos",
      type: "spring",
      inputs: { in: { node_id: "pair", selector: [{ index: 0 }] } },
      params: { stiffness: 120, damping: 20, mass: 1 },
    },
    {
      id: "spring_rot",
      type: "spring",
      inputs: { in: { node_id: "pair", selector: [{ index: 1 }] } },
      params: { stiffness: 120, damping: 20, mass: 1 },
    },
    // Damp
    {
      id: "damp_pos",
      type: "damp",
      inputs: { in: { node_id: "pair", selector: [{ index: 0 }] } },
      params: { half_life: 0.1 },
    },
    {
      id: "damp_rot",
      type: "damp",
      inputs: { in: { node_id: "pair", selector: [{ index: 1 }] } },
      params: { half_life: 0.1 },
    },
    // Slew
    {
      id: "slew_pos",
      type: "slew",
      inputs: { in: { node_id: "pair", selector: [{ index: 0 }] } },
      params: { max_rate: 1.0 },
    },
    {
      id: "slew_rot",
      type: "slew",
      inputs: { in: { node_id: "pair", selector: [{ index: 1 }] } },
      params: { max_rate: 1.0 },
    },

    // Re-assemble each result as a concatenated Vector [pos..., rot...]
    {
      id: "join_spring",
      type: "join",
      inputs: { a: { node_id: "spring_pos" }, b: { node_id: "spring_rot" } },
    },
    {
      id: "join_damp",
      type: "join",
      inputs: { a: { node_id: "damp_pos" }, b: { node_id: "damp_rot" } },
    },
    {
      id: "join_slew",
      type: "join",
      inputs: { a: { node_id: "slew_pos" }, b: { node_id: "slew_rot" } },
    },

    // Outputs
    {
      id: "out_spring",
      type: "output",
      params: { path: "samples/tuple.spring" },
      inputs: { in: { node_id: "join_spring" } },
    },
    {
      id: "out_damp",
      type: "output",
      params: { path: "samples/tuple.damp" },
      inputs: { in: { node_id: "join_damp" } },
    },
    {
      id: "out_slew",
      type: "output",
      params: { path: "samples/tuple.slew" },
      inputs: { in: { node_id: "join_slew" } },
    },
  ],
};

export const graphSamples: Record<string, GraphSpec> = {
  "oscillator-basics": oscillatorBasics,
  "vector-playground": vectorPlayground,
  "logic-gate": logicGate,
  "tuple-spring-damp-slew": tupleSpringDampSlew,
};
