import type { GraphSpec, ValueJSON } from "./types";

const textValue = (text: string): ValueJSON =>
  ({ type: "text", data: text } as unknown as ValueJSON);

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
      id: "vsub",
      type: "vectorsubtract",
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
      id: "out_sub",
      type: "output",
      params: { path: "samples/vector.sub" },
      inputs: { in: { node_id: "vsub" } },
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


/**
 * Nested Telemetry Aggregation
 *
 * Demonstrates recursive record/tuple/array inputs and selector-based projections:
 * - A deeply nested Input node with default values covering vectors, tuples, arrays, bools and text.
 * - Vector arithmetic mixes and joins different projections.
 * - Scalar extraction via VectorIndex feeds into numeric math nodes.
 * - Multiple Output sinks publish heterogenous values (vector, float, bool, text).
 */
export const nestedTelemetry: GraphSpec = {
  nodes: [
    {
      id: "payload",
      type: "input",
      params: {
        path: "samples/telemetry.payload",
        value: {
          record: {
            sensors: {
              record: {
                gyro: { vector: [0.1, -0.2, 0.05] },
                accel: { vector: [0.0, 9.8, 0.2] },
                temperature: { float: 36.5 },
              },
            },
            calibration: {
              record: {
                offsets: {
                  tuple: [
                    { vector: [0.5, 0.5, 0.5] },
                    { vector: [-0.5, -0.25, 0.75] },
                  ],
                },
                gains: {
                  array: [
                    { vector: [1.0, 0.5, 0.25] },
                    { vector: [0.2, 0.4, 0.8] },
                  ],
                },
              },
            },
            metadata: {
              record: {
                label: textValue("imu"),
                active: { bool: true },
              },
            },
          },
        },
      },
    },
    { id: "zero", type: "constant", params: { value: 0 } },
    { id: "two", type: "constant", params: { value: 2 } },
    {
      id: "accel_corrected",
      type: "vectorsubtract",
      inputs: {
        a: {
          node_id: "payload",
          selector: [
            { field: "sensors" },
            { field: "accel" },
          ],
        },
        b: {
          node_id: "payload",
          selector: [
            { field: "calibration" },
            { field: "offsets" },
            { index: 0 },
          ],
        },
      },
    },
    {
      id: "gyro_blended",
      type: "vectoradd",
      inputs: {
        a: {
          node_id: "payload",
          selector: [
            { field: "sensors" },
            { field: "gyro" },
          ],
        },
        b: {
          node_id: "payload",
          selector: [
            { field: "calibration" },
            { field: "offsets" },
            { index: 1 },
          ],
        },
      },
    },
    {
      id: "telemetry_join",
      type: "join",
      inputs: {
        segment_1: { node_id: "accel_corrected" },
        segment_2: { node_id: "gyro_blended" },
      },
    },
    {
      id: "calibration_pack",
      type: "join",
      inputs: {
        segment_1: {
          node_id: "payload",
          selector: [
            { field: "calibration" },
            { field: "offsets" },
            { index: 0 },
          ],
        },
        segment_2: {
          node_id: "payload",
          selector: [
            { field: "calibration" },
            { field: "offsets" },
            { index: 1 },
          ],
        },
      },
    },
    {
      id: "gain0_x",
      type: "vectorindex",
      inputs: {
        v: {
          node_id: "payload",
          selector: [
            { field: "calibration" },
            { field: "gains" },
            { index: 0 },
          ],
        },
        index: { node_id: "zero" },
      },
    },
    {
      id: "gain1_x",
      type: "vectorindex",
      inputs: {
        v: {
          node_id: "payload",
          selector: [
            { field: "calibration" },
            { field: "gains" },
            { index: 1 },
          ],
        },
        index: { node_id: "zero" },
      },
    },
    {
      id: "gain_sum",
      type: "add",
      inputs: {
        lhs: { node_id: "gain0_x" },
        rhs: { node_id: "gain1_x" },
      },
    },
    {
      id: "gain_avg",
      type: "divide",
      inputs: {
        lhs: { node_id: "gain_sum" },
        rhs: { node_id: "two" },
      },
    },
    {
      id: "telemetry_vector_out",
      type: "output",
      params: { path: "samples/telemetry.corrected" },
      inputs: { in: { node_id: "telemetry_join" } },
    },
    {
      id: "telemetry_gain_out",
      type: "output",
      params: { path: "samples/telemetry.gain" },
      inputs: { in: { node_id: "gain_avg" } },
    },
    {
      id: "telemetry_offsets_out",
      type: "output",
      params: { path: "samples/telemetry.offsets" },
      inputs: { in: { node_id: "calibration_pack" } },
    },
    {
      id: "telemetry_label_out",
      type: "output",
      params: { path: "samples/telemetry.label" },
      inputs: {
        in: {
          node_id: "payload",
          selector: [
            { field: "metadata" },
            { field: "label" },
          ],
        },
      },
    },
    {
      id: "telemetry_active_out",
      type: "output",
      params: { path: "samples/telemetry.active" },
      inputs: {
        in: {
          node_id: "payload",
          selector: [
            { field: "metadata" },
            { field: "active" },
          ],
        },
      },
    },
  ],
};

/**
 * Nested Rig Weighted Pose sample
 *
 * Demonstrates deeply nested selectors across records, arrays, tuples and lists.
 * A single Input node provides a hierarchical rig description. Downstream nodes
 * index into the structure to build a weighted pose, accumulate harmonic timing
 * data, and blend against a local target pose.
 */
export const nestedRigWeightedPose: GraphSpec = {
  nodes: [
    {
      id: "config",
      type: "input",
      params: {
        path: "samples/nested.rig",
        value: {
          record: {
            rig: {
              record: {
                root: { vector: [0.5, -0.25, 2.0] },
                limbs: {
                  array: [
                    {
                      record: {
                        offset: { vector: [0.25, 0.0, 0.5] },
                        weight: { float: 0.75 },
                      },
                    },
                    {
                      record: {
                        offset: { vector: [-0.1, 0.4, -0.2] },
                        weight: { float: 0.4 },
                      },
                    },
                  ],
                },
                controls: {
                  record: {
                    phase: { float: 0.35 },
                    harmonics: {
                      list: [
                        {
                          record: {
                            amplitude: { float: 0.5 },
                            frequency: { float: 2.0 },
                          },
                        },
                        {
                          record: {
                            amplitude: { float: 0.25 },
                            frequency: { float: 4.0 },
                          },
                        },
                      ],
                    },
                    localTarget: {
                      tuple: [
                        { vector: [0.1, -0.3, 0.5] },
                        { vector: [0.0, 0.75, -0.25] },
                      ],
                    },
                  },
                },
              },
            },
          },
        },
      },
    },
    {
      id: "limb0",
      type: "vectorscale",
      inputs: {
        scalar: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "limbs" },
            { index: 0 },
            { field: "weight" },
          ],
        },
        v: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "limbs" },
            { index: 0 },
            { field: "offset" },
          ],
        },
      },
    },
    {
      id: "limb1",
      type: "vectorscale",
      inputs: {
        scalar: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "limbs" },
            { index: 1 },
            { field: "weight" },
          ],
        },
        v: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "limbs" },
            { index: 1 },
            { field: "offset" },
          ],
        },
      },
    },
    {
      id: "limb_sum",
      type: "vectoradd",
      inputs: {
        a: { node_id: "limb0" },
        b: { node_id: "limb1" },
      },
    },
    {
      id: "pose_sum",
      type: "vectoradd",
      inputs: {
        a: {
          node_id: "config",
          selector: [{ field: "rig" }, { field: "root" }],
        },
        b: { node_id: "limb_sum" },
      },
    },
    {
      id: "harmonic0",
      type: "multiply",
      inputs: {
        operands_1: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "controls" },
            { field: "harmonics" },
            { index: 0 },
            { field: "amplitude" },
          ],
        },
        operands_2: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "controls" },
            { field: "harmonics" },
            { index: 0 },
            { field: "frequency" },
          ],
        },
      },
    },
    {
      id: "harmonic1",
      type: "multiply",
      inputs: {
        operands_1: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "controls" },
            { field: "harmonics" },
            { index: 1 },
            { field: "amplitude" },
          ],
        },
        operands_2: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "controls" },
            { field: "harmonics" },
            { index: 1 },
            { field: "frequency" },
          ],
        },
      },
    },
    {
      id: "phase_sum",
      type: "add",
      inputs: {
        operands_1: {
          node_id: "config",
          selector: [{ field: "rig" }, { field: "controls" }, { field: "phase" }],
        },
        operands_2: { node_id: "harmonic0" },
        operands_3: { node_id: "harmonic1" },
      },
    },
    {
      id: "target_scaled",
      type: "vectorscale",
      inputs: {
        scalar: { node_id: "phase_sum" },
        v: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "controls" },
            { field: "localTarget" },
            { index: 1 },
          ],
        },
      },
    },
    {
      id: "target_combined",
      type: "vectoradd",
      inputs: {
        a: {
          node_id: "config",
          selector: [
            { field: "rig" },
            { field: "controls" },
            { field: "localTarget" },
            { index: 0 },
          ],
        },
        b: { node_id: "target_scaled" },
      },
    },
    {
      id: "pose_mix",
      type: "vectoradd",
      inputs: {
        a: { node_id: "pose_sum" },
        b: { node_id: "target_combined" },
      },
    },
    {
      id: "out_pose",
      type: "output",
      params: { path: "samples/nested.pose" },
      inputs: { in: { node_id: "pose_sum" } },
    },
    {
      id: "out_phase",
      type: "output",
      params: { path: "samples/nested.phase" },
      inputs: { in: { node_id: "phase_sum" } },
    },
    {
      id: "out_target",
      type: "output",
      params: { path: "samples/nested.target" },
      inputs: { in: { node_id: "target_combined" } },
    },
    {
      id: "out_pose_mix",
      type: "output",
      params: { path: "samples/nested.pose_mix" },
      inputs: { in: { node_id: "pose_mix" } },
    },
  ],
};

/**
 * Selector Cascade sample
 *
 * Builds a scalar score from a complex payload that mixes arrays, lists and tuples.
 * Demonstrates selector chains, scalar math, vector indexing and conditional gating.
 */
export const selectorCascade: GraphSpec = {
  nodes: [
    {
      id: "payload",
      type: "input",
      params: {
        path: "samples/selector.payload",
        value: {
          record: {
            metrics: {
              record: {
                nested: {
                  tuple: [
                    {
                      record: {
                        values: {
                          array: [
                            { float: 2.0 },
                            { float: 3.0 },
                            { float: 5.0 },
                          ],
                        },
                        weight: { float: 0.75 },
                      },
                    },
                    {
                      record: {
                        values: {
                          list: [{ float: -1.0 }, { float: 4.0 }],
                        },
                        weight: { float: 0.25 },
                      },
                    },
                  ],
                },
              },
            },
            offsets: { vector: [1.0, 2.0, 3.0, 4.0] },
            toggle: { bool: true },
          },
        },
      },
    },
    { id: "two", type: "constant", params: { value: 2.0 } },
    { id: "zero", type: "constant", params: { value: 0.0 } },
    {
      id: "primary_sum",
      type: "add",
      inputs: {
        operands_1: {
          node_id: "payload",
          selector: [
            { field: "metrics" },
            { field: "nested" },
            { index: 0 },
            { field: "values" },
            { index: 0 },
          ],
        },
        operands_2: {
          node_id: "payload",
          selector: [
            { field: "metrics" },
            { field: "nested" },
            { index: 0 },
            { field: "values" },
            { index: 2 },
          ],
        },
      },
    },
    {
      id: "primary_weighted",
      type: "multiply",
      inputs: {
        operands_1: { node_id: "primary_sum" },
        operands_2: {
          node_id: "payload",
          selector: [
            { field: "metrics" },
            { field: "nested" },
            { index: 0 },
            { field: "weight" },
          ],
        },
      },
    },
    {
      id: "secondary_sum",
      type: "add",
      inputs: {
        operands_1: {
          node_id: "payload",
          selector: [
            { field: "metrics" },
            { field: "nested" },
            { index: 1 },
            { field: "values" },
            { index: 0 },
          ],
        },
        operands_2: {
          node_id: "payload",
          selector: [
            { field: "metrics" },
            { field: "nested" },
            { index: 1 },
            { field: "values" },
            { index: 1 },
          ],
        },
      },
    },
    {
      id: "secondary_mean",
      type: "divide",
      inputs: {
        lhs: { node_id: "secondary_sum" },
        rhs: { node_id: "two" },
      },
    },
    {
      id: "secondary_weighted",
      type: "multiply",
      inputs: {
        operands_1: { node_id: "secondary_mean" },
        operands_2: {
          node_id: "payload",
          selector: [
            { field: "metrics" },
            { field: "nested" },
            { index: 1 },
            { field: "weight" },
          ],
        },
      },
    },
    {
      id: "offset_component",
      type: "vectorindex",
      inputs: {
        v: {
          node_id: "payload",
          selector: [{ field: "offsets" }],
        },
        index: { node_id: "two" },
      },
    },
    {
      id: "gated_bias",
      type: "if",
      inputs: {
        cond: {
          node_id: "payload",
          selector: [{ field: "toggle" }],
        },
        then: { node_id: "offset_component" },
        else: { node_id: "zero" },
      },
    },
    {
      id: "final_score",
      type: "add",
      inputs: {
        operands_1: { node_id: "primary_weighted" },
        operands_2: { node_id: "secondary_weighted" },
        operands_3: { node_id: "gated_bias" },
      },
    },
    {
      id: "out_score",
      type: "output",
      params: { path: "samples/selector.score" },
      inputs: { in: { node_id: "final_score" } },
    },
    {
      id: "out_secondary",
      type: "output",
      params: { path: "samples/selector.secondary_mean" },
      inputs: { in: { node_id: "secondary_mean" } },
    },
    {
      id: "out_primary",
      type: "output",
      params: { path: "samples/selector.primary_weighted" },
      inputs: { in: { node_id: "primary_weighted" } },
    },
  ],
};


/**
 * Layered Rig Blend
 *
 * Demonstrates deeply nested structured values and selector projections:
 * - An Input node produces a record containing lists, tuples and child records.
 * - VectorScale nodes combine list indices with record fields to build weighted poses.
 * - Join gathers scalar weights into a typed vector output.
 * - Output nodes publish nested list/tuple data directly from the structured source.
 */
export const layeredRigBlend: GraphSpec = {
  nodes: [
    {
      id: "rig_config",
      type: "input",
      params: {
        path: "samples/rig.config",
        value: {
          record: {
            base_pose: { vec3: [0.1, 0.25, -0.05] },
            layers: {
              list: [
                {
                  record: {
                    offset: { vec3: [0.5, -0.25, 0.0] },
                    weight: { float: 0.6 },
                  },
                },
                {
                  record: {
                    offset: { vec3: [-0.2, 0.1, 0.4] },
                    weight: { float: 0.25 },
                  },
                },
              ],
            },
            gain: { float: 1.5 },
            info: {
              record: {
                tags: { list: [{ text: "arm" }, { text: "blend" }] },
                counters: { tuple: [{ float: 2 }, { float: 3 }] },
              },
            },
          },
        },
      },
    },

    {
      id: "layer0_scaled",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 0 }, { field: "offset" }],
        },
        scalar: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
        },
      },
    },
    {
      id: "layer1_scaled",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 1 }, { field: "offset" }],
        },
        scalar: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
        },
      },
    },
    {
      id: "layer_sum",
      type: "vectoradd",
      inputs: {
        a: { node_id: "layer0_scaled" },
        b: { node_id: "layer1_scaled" },
      },
    },
    {
      id: "gain_scale",
      type: "vectorscale",
      inputs: {
        v: { node_id: "layer_sum" },
        scalar: { node_id: "rig_config", selector: [{ field: "gain" }] },
      },
    },
    {
      id: "pose_result",
      type: "vectoradd",
      inputs: {
        a: { node_id: "gain_scale" },
        b: { node_id: "rig_config", selector: [{ field: "base_pose" }] },
      },
    },

    {
      id: "weight_sum",
      type: "add",
      inputs: {
        a: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
        },
        b: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
        },
      },
    },
    {
      id: "weights_vector",
      type: "join",
      inputs: {
        first: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
        },
        second: {
          node_id: "rig_config",
          selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
        },
        total: { node_id: "weight_sum" },
      },
    },

    {
      id: "out_pose",
      type: "output",
      params: { path: "samples/rig.pose" },
      inputs: { in: { node_id: "pose_result" } },
    },
    {
      id: "out_weights",
      type: "output",
      params: { path: "samples/rig.weights" },
      inputs: { in: { node_id: "weights_vector" } },
    },
    {
      id: "out_tags",
      type: "output",
      params: { path: "samples/rig.tags" },
      inputs: {
        in: {
          node_id: "rig_config",
          selector: [{ field: "info" }, { field: "tags" }],
        },
      },
    },
    {
      id: "out_counters",
      type: "output",
      params: { path: "samples/rig.counterTuple" },
      inputs: {
        in: {
          node_id: "rig_config",
          selector: [{ field: "info" }, { field: "counters" }],
        },
      },
    },
  ],
};


/**
 * Hierarchical blend example with nested record/array inputs.
 * - Demonstrates selectors that traverse field/index/field chains.
 * - Uses Split/Join/VectorIndex plus scalar/vector math.
 */
export const hierarchicalBlend: GraphSpec = {
  nodes: [
    {
      id: "rig",
      type: "input",
      params: {
        path: "samples/nested.rig",
        value: {
          record: {
            controls: {
              array: [
                {
                  record: {
                    weight: { float: 0.25 },
                    offset: { vec3: [0.2, 0.0, -0.1] },
                  },
                },
                {
                  record: {
                    weight: { float: 0.75 },
                    offset: { vec3: [-0.1, 0.5, 0.2] },
                  },
                },
              ],
            },
            aim: {
              tuple: [{ vec3: [0, 0, 0] }, { vec3: [1, 2, 2] }],
            },
            bias: { vec3: [0.05, -0.05, 0.1] },
            weights: { vector: [0.25, 0.5, 0.75] },
          },
        },
      },
    },
    {
      id: "ctrl0",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "rig",
          selector: [
            { field: "controls" },
            { index: 0 },
            { field: "offset" },
          ],
        },
        scalar: {
          node_id: "rig",
          selector: [
            { field: "controls" },
            { index: 0 },
            { field: "weight" },
          ],
        },
      },
    },
    {
      id: "ctrl1",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "rig",
          selector: [
            { field: "controls" },
            { index: 1 },
            { field: "offset" },
          ],
        },
        scalar: {
          node_id: "rig",
          selector: [
            { field: "controls" },
            { index: 1 },
            { field: "weight" },
          ],
        },
      },
    },
    {
      id: "combined",
      type: "vectoradd",
      inputs: { a: { node_id: "ctrl0" }, b: { node_id: "ctrl1" } },
    },
    {
      id: "biased",
      type: "vectoradd",
      inputs: {
        a: { node_id: "combined" },
        b: { node_id: "rig", selector: [{ field: "bias" }] },
      },
    },
    {
      id: "offset_split",
      type: "split",
      params: { sizes: [2, 1] },
      inputs: { in: { node_id: "biased" } },
    },
    { id: "component_index", type: "constant", params: { value: 2 } },
    {
      id: "weight_component",
      type: "vectorindex",
      inputs: {
        v: { node_id: "rig", selector: [{ field: "weights" }] },
        index: { node_id: "component_index" },
      },
    },
    {
      id: "aim_diff",
      type: "vectorsubtract",
      inputs: {
        a: {
          node_id: "rig",
          selector: [{ field: "aim" }, { index: 1 }],
        },
        b: {
          node_id: "rig",
          selector: [{ field: "aim" }, { index: 0 }],
        },
      },
    },
    { id: "aim_distance", type: "vectorlength", inputs: { in: { node_id: "aim_diff" } } },
    {
      id: "pose_join",
      type: "join",
      inputs: { a: { node_id: "biased" }, b: { node_id: "aim_diff" } },
    },
    {
      id: "pose_out",
      type: "output",
      params: { path: "samples/nested.pose" },
      inputs: { in: { node_id: "pose_join" } },
    },
    {
      id: "offset_xy_out",
      type: "output",
      params: { path: "samples/nested.offset_xy" },
      inputs: { in: { node_id: "offset_split", output_key: "part1" } },
    },
    {
      id: "offset_z_out",
      type: "output",
      params: { path: "samples/nested.offset_z" },
      inputs: {
        in: {
          node_id: "offset_split",
          output_key: "part2",
          selector: [{ index: 0 }],
        },
      },
    },
    {
      id: "aim_distance_out",
      type: "output",
      params: { path: "samples/nested.aim_distance" },
      inputs: { in: { node_id: "aim_distance" } },
    },
    {
      id: "weight_component_out",
      type: "output",
      params: { path: "samples/nested.weight_2" },
      inputs: { in: { node_id: "weight_component" } },
    },
  ],
};

/**
 * Weighted average across a tuple of records. Exercises tuple/field selectors,
 * scalar aggregation and vector scaling.
 */
export const weightedAverage: GraphSpec = {
  nodes: [
    {
      id: "targets",
      type: "input",
      params: {
        path: "samples/weighted.targets",
        value: {
          tuple: [
            {
              record: {
                weight: { float: 0.5 },
                value: { vec3: [1, 0, 0.8] },
              },
            },
            {
              record: {
                weight: { float: 0.3 },
                value: { vec3: [0.2, 1, 0] },
              },
            },
            {
              record: {
                weight: { float: 0.4 },
                value: { vec3: [0, 0, 1] },
              },
            },
          ],
        },
      },
    },
    {
      id: "weighted_0",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "targets",
          selector: [{ index: 0 }, { field: "value" }],
        },
        scalar: {
          node_id: "targets",
          selector: [{ index: 0 }, { field: "weight" }],
        },
      },
    },
    {
      id: "weighted_1",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "targets",
          selector: [{ index: 1 }, { field: "value" }],
        },
        scalar: {
          node_id: "targets",
          selector: [{ index: 1 }, { field: "weight" }],
        },
      },
    },
    {
      id: "weighted_2",
      type: "vectorscale",
      inputs: {
        v: {
          node_id: "targets",
          selector: [{ index: 2 }, { field: "value" }],
        },
        scalar: {
          node_id: "targets",
          selector: [{ index: 2 }, { field: "weight" }],
        },
      },
    },
    {
      id: "weighted_sum_ab",
      type: "vectoradd",
      inputs: { a: { node_id: "weighted_0" }, b: { node_id: "weighted_1" } },
    },
    {
      id: "weighted_sum",
      type: "vectoradd",
      inputs: { a: { node_id: "weighted_sum_ab" }, b: { node_id: "weighted_2" } },
    },
    {
      id: "weight_sum",
      type: "add",
      inputs: {
        weight_0: {
          node_id: "targets",
          selector: [{ index: 0 }, { field: "weight" }],
        },
        weight_1: {
          node_id: "targets",
          selector: [{ index: 1 }, { field: "weight" }],
        },
        weight_2: {
          node_id: "targets",
          selector: [{ index: 2 }, { field: "weight" }],
        },
      },
    },
    { id: "one", type: "constant", params: { value: 1 } },
    {
      id: "inv_weight",
      type: "divide",
      inputs: { lhs: { node_id: "one" }, rhs: { node_id: "weight_sum" } },
    },
    {
      id: "average",
      type: "vectorscale",
      inputs: { v: { node_id: "weighted_sum" }, scalar: { node_id: "inv_weight" } },
    },
    {
      id: "sum_out",
      type: "output",
      params: { path: "samples/weighted.sum" },
      inputs: { in: { node_id: "weighted_sum" } },
    },
    {
      id: "avg_out",
      type: "output",
      params: { path: "samples/weighted.average" },
      inputs: { in: { node_id: "average" } },
    },
    {
      id: "total_out",
      type: "output",
      params: { path: "samples/weighted.total" },
      inputs: { in: { node_id: "weight_sum" } },
    },
  ],
};


export const graphSamples: Record<string, GraphSpec> = {
  "oscillator-basics": oscillatorBasics,
  "vector-playground": vectorPlayground,
  "logic-gate": logicGate,
  "tuple-spring-damp-slew": tupleSpringDampSlew,
  "nested-telemetry": nestedTelemetry,
  "nested-rig-weighted-pose": nestedRigWeightedPose,
  "selector-cascade": selectorCascade,
  "layered-rig-blend": layeredRigBlend,
  "hierarchical-blend": hierarchicalBlend,
  "weighted-average": weightedAverage,
};
