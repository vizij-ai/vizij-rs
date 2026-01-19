import type {
  GraphSpec,
  EdgeSpec,
  SelectorSegmentJSON,
  ValueJSON,
} from "./types";

const edge = (
  from: string,
  to: string,
  input: string,
  options: { output?: string; selector?: SelectorSegmentJSON[] } = {},
): EdgeSpec => ({
  from: {
    node_id: from,
    ...(options.output ? { output: options.output } : {}),
  },
  to: { node_id: to, input },
  ...(options.selector ? { selector: options.selector } : {}),
});

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
    { id: "osc", type: "oscillator" },
    { id: "offset", type: "constant", params: { value: 0.3 } },
    { id: "add1", type: "add" },
    { id: "const0", type: "constant", params: { value: 0 } },
    { id: "clamp_max", type: "constant", params: { value: 1 } },
    { id: "clamp1", type: "clamp" },
    { id: "remap_in_min", type: "constant", params: { value: 0 } },
    { id: "remap_in_max", type: "constant", params: { value: 1 } },
    { id: "remap_out_min", type: "constant", params: { value: -1 } },
    { id: "remap_out_max", type: "constant", params: { value: 1 } },
    { id: "remap1", type: "remap" },
    {
      id: "out",
      type: "output",
      params: { path: "samples/oscillator.signal" },
    },
  ],
  edges: [
    edge("freq", "osc", "frequency"),
    edge("time", "osc", "phase"),
    edge("osc", "add1", "a"),
    edge("offset", "add1", "b"),
    edge("add1", "clamp1", "in"),
    edge("const0", "clamp1", "min"),
    edge("clamp_max", "clamp1", "max"),
    edge("clamp1", "remap1", "in"),
    edge("remap_in_min", "remap1", "in_min"),
    edge("remap_in_max", "remap1", "in_max"),
    edge("remap_out_min", "remap1", "out_min"),
    edge("remap_out_max", "remap1", "out_max"),
    edge("remap1", "out", "in"),
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
    { id: "vadd", type: "vectoradd" },
    { id: "vsub", type: "vectorsubtract" },
    { id: "vnorm", type: "vectornormalize" },
    { id: "vdot", type: "vectordot" },
    { id: "vlen", type: "vectorlength" },
    {
      id: "out_sum",
      type: "output",
      params: { path: "samples/vector.sum" },
    },
    {
      id: "out_sub",
      type: "output",
      params: { path: "samples/vector.sub" },
    },
    {
      id: "out_dot",
      type: "output",
      params: { path: "samples/vector.dot" },
    },
    {
      id: "out_len",
      type: "output",
      params: { path: "samples/vector.len" },
    },
  ],
  edges: [
    edge("v1_in", "vadd", "a"),
    edge("v2_in", "vadd", "b"),
    edge("v1_in", "vsub", "a"),
    edge("v2_in", "vsub", "b"),
    edge("v2_in", "vnorm", "in"),
    edge("vadd", "vdot", "a"),
    edge("vnorm", "vdot", "b"),
    edge("vadd", "vlen", "in"),
    edge("vadd", "out_sum", "in"),
    edge("vsub", "out_sub", "in"),
    edge("vdot", "out_dot", "in"),
    edge("vlen", "out_len", "in"),
  ],
};

/**
 * Logic Gate
 * Time → Sin → GreaterThan → If → Output
 */
export const logicGate: GraphSpec = {
  nodes: [
    { id: "time", type: "time" },
    { id: "sin", type: "sin" },
    { id: "threshold", type: "constant", params: { value: 0 } },
    { id: "greater", type: "greaterthan" },
    { id: "then", type: "constant", params: { value: 1 } },
    { id: "else", type: "constant", params: { value: -1 } },
    { id: "gate", type: "if" },
    {
      id: "out",
      type: "output",
      params: { path: "samples/logic.gated" },
    },
  ],
  edges: [
    edge("time", "sin", "in"),
    edge("sin", "greater", "lhs"),
    edge("threshold", "greater", "rhs"),
    edge("greater", "gate", "cond"),
    edge("then", "gate", "then"),
    edge("else", "gate", "else"),
    edge("gate", "out", "in"),
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
    {
      id: "spring_pos",
      type: "spring",
      params: { stiffness: 120, damping: 20, mass: 1 },
    },
    {
      id: "spring_rot",
      type: "spring",
      params: { stiffness: 120, damping: 20, mass: 1 },
    },
    {
      id: "damp_pos",
      type: "damp",
      params: { half_life: 0.1 },
    },
    {
      id: "damp_rot",
      type: "damp",
      params: { half_life: 0.1 },
    },
    {
      id: "slew_pos",
      type: "slew",
      params: { max_rate: 1.0 },
    },
    {
      id: "slew_rot",
      type: "slew",
      params: { max_rate: 1.0 },
    },
    { id: "join_spring", type: "join" },
    { id: "join_damp", type: "join" },
    { id: "join_slew", type: "join" },
    {
      id: "out_spring",
      type: "output",
      params: { path: "samples/tuple.spring" },
    },
    {
      id: "out_damp",
      type: "output",
      params: { path: "samples/tuple.damp" },
    },
    {
      id: "out_slew",
      type: "output",
      params: { path: "samples/tuple.slew" },
    },
  ],
  edges: [
    edge("pair", "spring_pos", "in", { selector: [{ index: 0 }] }),
    edge("pair", "spring_rot", "in", { selector: [{ index: 1 }] }),
    edge("pair", "damp_pos", "in", { selector: [{ index: 0 }] }),
    edge("pair", "damp_rot", "in", { selector: [{ index: 1 }] }),
    edge("pair", "slew_pos", "in", { selector: [{ index: 0 }] }),
    edge("pair", "slew_rot", "in", { selector: [{ index: 1 }] }),
    edge("spring_pos", "join_spring", "a"),
    edge("spring_rot", "join_spring", "b"),
    edge("damp_pos", "join_damp", "a"),
    edge("damp_rot", "join_damp", "b"),
    edge("slew_pos", "join_slew", "a"),
    edge("slew_rot", "join_slew", "b"),
    edge("join_spring", "out_spring", "in"),
    edge("join_damp", "out_damp", "in"),
    edge("join_slew", "out_slew", "in"),
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
    { id: "accel_corrected", type: "vectorsubtract" },
    { id: "gyro_blended", type: "vectoradd" },
    { id: "telemetry_join", type: "join" },
    { id: "calibration_pack", type: "join" },
    { id: "gain0_x", type: "vectorindex" },
    { id: "gain1_x", type: "vectorindex" },
    { id: "gain_sum", type: "add" },
    { id: "gain_avg", type: "divide" },
    {
      id: "telemetry_vector_out",
      type: "output",
      params: { path: "samples/telemetry.corrected" },
    },
    {
      id: "telemetry_gain_out",
      type: "output",
      params: { path: "samples/telemetry.gain" },
    },
    {
      id: "telemetry_offsets_out",
      type: "output",
      params: { path: "samples/telemetry.offsets" },
    },
    {
      id: "telemetry_label_out",
      type: "output",
      params: { path: "samples/telemetry.label" },
    },
    {
      id: "telemetry_active_out",
      type: "output",
      params: { path: "samples/telemetry.active" },
    },
  ],
  edges: [
    edge("payload", "accel_corrected", "a", {
      selector: [{ field: "sensors" }, { field: "accel" }],
    }),
    edge("payload", "accel_corrected", "b", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 0 },
      ],
    }),
    edge("payload", "gyro_blended", "a", {
      selector: [{ field: "sensors" }, { field: "gyro" }],
    }),
    edge("payload", "gyro_blended", "b", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 1 },
      ],
    }),
    edge("accel_corrected", "telemetry_join", "segment_1"),
    edge("gyro_blended", "telemetry_join", "segment_2"),
    edge("payload", "calibration_pack", "segment_1", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 0 },
      ],
    }),
    edge("payload", "calibration_pack", "segment_2", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 1 },
      ],
    }),
    edge("payload", "gain0_x", "v", {
      selector: [
        { field: "calibration" },
        { field: "gains" },
        { index: 0 },
      ],
    }),
    edge("zero", "gain0_x", "index"),
    edge("payload", "gain1_x", "v", {
      selector: [
        { field: "calibration" },
        { field: "gains" },
        { index: 1 },
      ],
    }),
    edge("zero", "gain1_x", "index"),
    edge("gain0_x", "gain_sum", "lhs"),
    edge("gain1_x", "gain_sum", "rhs"),
    edge("gain_sum", "gain_avg", "lhs"),
    edge("two", "gain_avg", "rhs"),
    edge("telemetry_join", "telemetry_vector_out", "in"),
    edge("gain_avg", "telemetry_gain_out", "in"),
    edge("calibration_pack", "telemetry_offsets_out", "in"),
    edge("payload", "telemetry_label_out", "in", {
      selector: [{ field: "metadata" }, { field: "label" }],
    }),
    edge("payload", "telemetry_active_out", "in", {
      selector: [{ field: "metadata" }, { field: "active" }],
    }),
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
    { id: "limb0", type: "vectorscale" },
    { id: "limb1", type: "vectorscale" },
    { id: "limb_sum", type: "vectoradd" },
    { id: "pose_sum", type: "vectoradd" },
    { id: "harmonic0", type: "multiply" },
    { id: "harmonic1", type: "multiply" },
    { id: "phase_sum", type: "add" },
    { id: "target_scaled", type: "vectorscale" },
    { id: "target_combined", type: "vectoradd" },
    { id: "pose_mix", type: "vectoradd" },
    {
      id: "out_pose",
      type: "output",
      params: { path: "samples/nested.pose" },
    },
    {
      id: "out_phase",
      type: "output",
      params: { path: "samples/nested.phase" },
    },
    {
      id: "out_target",
      type: "output",
      params: { path: "samples/nested.target" },
    },
    {
      id: "out_pose_mix",
      type: "output",
      params: { path: "samples/nested.pose_mix" },
    },
  ],
  edges: [
    edge("config", "limb0", "scalar", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 0 },
        { field: "weight" },
      ],
    }),
    edge("config", "limb0", "v", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 0 },
        { field: "offset" },
      ],
    }),
    edge("config", "limb1", "scalar", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 1 },
        { field: "weight" },
      ],
    }),
    edge("config", "limb1", "v", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 1 },
        { field: "offset" },
      ],
    }),
    edge("limb0", "limb_sum", "a"),
    edge("limb1", "limb_sum", "b"),
    edge("config", "pose_sum", "a", {
      selector: [{ field: "rig" }, { field: "root" }],
    }),
    edge("limb_sum", "pose_sum", "b"),
    edge("config", "harmonic0", "operand_1", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 0 },
        { field: "amplitude" },
      ],
    }),
    edge("config", "harmonic0", "operand_2", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 0 },
        { field: "frequency" },
      ],
    }),
    edge("config", "harmonic1", "operand_1", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 1 },
        { field: "amplitude" },
      ],
    }),
    edge("config", "harmonic1", "operand_2", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 1 },
        { field: "frequency" },
      ],
    }),
    edge("config", "phase_sum", "operand_1", {
      selector: [{ field: "rig" }, { field: "controls" }, { field: "phase" }],
    }),
    edge("harmonic0", "phase_sum", "operand_2"),
    edge("harmonic1", "phase_sum", "operand_3"),
    edge("phase_sum", "target_scaled", "scalar"),
    edge("config", "target_scaled", "v", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "localTarget" },
        { index: 1 },
      ],
    }),
    edge("config", "target_combined", "a", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "localTarget" },
        { index: 0 },
      ],
    }),
    edge("target_scaled", "target_combined", "b"),
    edge("pose_sum", "pose_mix", "a"),
    edge("target_combined", "pose_mix", "b"),
    edge("pose_sum", "out_pose", "in"),
    edge("phase_sum", "out_phase", "in"),
    edge("target_combined", "out_target", "in"),
    edge("pose_mix", "out_pose_mix", "in"),
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
    { id: "primary_sum", type: "add" },
    { id: "primary_weighted", type: "multiply" },
    { id: "secondary_sum", type: "add" },
    { id: "secondary_mean", type: "divide" },
    { id: "secondary_weighted", type: "multiply" },
    { id: "offset_component", type: "vectorindex" },
    { id: "gated_bias", type: "if" },
    { id: "final_score", type: "add" },
    {
      id: "out_score",
      type: "output",
      params: { path: "samples/selector.score" },
    },
    {
      id: "out_secondary",
      type: "output",
      params: { path: "samples/selector.secondary_mean" },
    },
    {
      id: "out_primary",
      type: "output",
      params: { path: "samples/selector.primary_weighted" },
    },
  ],
  edges: [
    edge("payload", "primary_sum", "operand_1", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 0 },
        { field: "values" },
        { index: 0 },
      ],
    }),
    edge("payload", "primary_sum", "operand_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 0 },
        { field: "values" },
        { index: 2 },
      ],
    }),
    edge("primary_sum", "primary_weighted", "operand_1"),
    edge("payload", "primary_weighted", "operand_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 0 },
        { field: "weight" },
      ],
    }),
    edge("payload", "secondary_sum", "operand_1", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 1 },
        { field: "values" },
        { index: 0 },
      ],
    }),
    edge("payload", "secondary_sum", "operand_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 1 },
        { field: "values" },
        { index: 1 },
      ],
    }),
    edge("secondary_sum", "secondary_mean", "lhs"),
    edge("two", "secondary_mean", "rhs"),
    edge("secondary_mean", "secondary_weighted", "operand_1"),
    edge("payload", "secondary_weighted", "operand_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 1 },
        { field: "weight" },
      ],
    }),
    edge("payload", "offset_component", "v", {
      selector: [{ field: "offsets" }],
    }),
    edge("two", "offset_component", "index"),
    edge("payload", "gated_bias", "cond", {
      selector: [{ field: "toggle" }],
    }),
    edge("offset_component", "gated_bias", "then"),
    edge("zero", "gated_bias", "else"),
    edge("primary_weighted", "final_score", "operand_1"),
    edge("secondary_weighted", "final_score", "operand_2"),
    edge("gated_bias", "final_score", "operand_3"),
    edge("final_score", "out_score", "in"),
    edge("secondary_mean", "out_secondary", "in"),
    edge("primary_weighted", "out_primary", "in"),
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
    { id: "layer0_scaled", type: "vectorscale" },
    { id: "layer1_scaled", type: "vectorscale" },
    { id: "layer_sum", type: "vectoradd" },
    { id: "gain_scale", type: "vectorscale" },
    { id: "pose_result", type: "vectoradd" },
    { id: "weight_sum", type: "add" },
    { id: "weights_vector", type: "join" },
    {
      id: "out_pose",
      type: "output",
      params: { path: "samples/rig.pose" },
    },
    {
      id: "out_weights",
      type: "output",
      params: { path: "samples/rig.weights" },
    },
    {
      id: "out_tags",
      type: "output",
      params: { path: "samples/rig.tags" },
    },
    {
      id: "out_counters",
      type: "output",
      params: { path: "samples/rig.counterTuple" },
    },
  ],
  edges: [
    edge("rig_config", "layer0_scaled", "v", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "offset" }],
    }),
    edge("rig_config", "layer0_scaled", "scalar", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
    }),
    edge("rig_config", "layer1_scaled", "v", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "offset" }],
    }),
    edge("rig_config", "layer1_scaled", "scalar", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
    }),
    edge("layer0_scaled", "layer_sum", "a"),
    edge("layer1_scaled", "layer_sum", "b"),
    edge("layer_sum", "gain_scale", "v"),
    edge("rig_config", "gain_scale", "scalar", {
      selector: [{ field: "gain" }],
    }),
    edge("gain_scale", "pose_result", "a"),
    edge("rig_config", "pose_result", "b", {
      selector: [{ field: "base_pose" }],
    }),
    edge("rig_config", "weight_sum", "a", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
    }),
    edge("rig_config", "weight_sum", "b", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
    }),
    edge("rig_config", "weights_vector", "first", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
    }),
    edge("rig_config", "weights_vector", "second", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
    }),
    edge("weight_sum", "weights_vector", "total"),
    edge("pose_result", "out_pose", "in"),
    edge("weights_vector", "out_weights", "in"),
    edge("rig_config", "out_tags", "in", {
      selector: [{ field: "info" }, { field: "tags" }],
    }),
    edge("rig_config", "out_counters", "in", {
      selector: [{ field: "info" }, { field: "counters" }],
    }),
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
    { id: "ctrl0", type: "vectorscale" },
    { id: "ctrl1", type: "vectorscale" },
    { id: "combined", type: "vectoradd" },
    { id: "biased", type: "vectoradd" },
    { id: "offset_split", type: "split", params: { sizes: [2, 1] } },
    { id: "component_index", type: "constant", params: { value: 2 } },
    { id: "weight_component", type: "vectorindex" },
    { id: "aim_diff", type: "vectorsubtract" },
    { id: "aim_distance", type: "vectorlength" },
    { id: "pose_join", type: "join" },
    {
      id: "pose_out",
      type: "output",
      params: { path: "samples/nested.pose" },
    },
    {
      id: "offset_xy_out",
      type: "output",
      params: { path: "samples/nested.offset_xy" },
    },
    {
      id: "offset_z_out",
      type: "output",
      params: { path: "samples/nested.offset_z" },
    },
    {
      id: "aim_distance_out",
      type: "output",
      params: { path: "samples/nested.aim_distance" },
    },
    {
      id: "weight_component_out",
      type: "output",
      params: { path: "samples/nested.weight_2" },
    },
  ],
  edges: [
    edge("rig", "ctrl0", "v", {
      selector: [
        { field: "controls" },
        { index: 0 },
        { field: "offset" },
      ],
    }),
    edge("rig", "ctrl0", "scalar", {
      selector: [
        { field: "controls" },
        { index: 0 },
        { field: "weight" },
      ],
    }),
    edge("rig", "ctrl1", "v", {
      selector: [
        { field: "controls" },
        { index: 1 },
        { field: "offset" },
      ],
    }),
    edge("rig", "ctrl1", "scalar", {
      selector: [
        { field: "controls" },
        { index: 1 },
        { field: "weight" },
      ],
    }),
    edge("ctrl0", "combined", "a"),
    edge("ctrl1", "combined", "b"),
    edge("combined", "biased", "a"),
    edge("rig", "biased", "b", { selector: [{ field: "bias" }] }),
    edge("biased", "offset_split", "in"),
    edge("rig", "weight_component", "v", {
      selector: [{ field: "weights" }],
    }),
    edge("component_index", "weight_component", "index"),
    edge("rig", "aim_diff", "a", {
      selector: [{ field: "aim" }, { index: 1 }],
    }),
    edge("rig", "aim_diff", "b", {
      selector: [{ field: "aim" }, { index: 0 }],
    }),
    edge("aim_diff", "aim_distance", "in"),
    edge("biased", "pose_join", "a"),
    edge("aim_diff", "pose_join", "b"),
    edge("pose_join", "pose_out", "in"),
    edge("offset_split", "offset_xy_out", "in", { output: "part1" }),
    edge("offset_split", "offset_z_out", "in", {
      output: "part2",
      selector: [{ index: 0 }],
    }),
    edge("aim_distance", "aim_distance_out", "in"),
    edge("weight_component", "weight_component_out", "in"),
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
    { id: "weighted_0", type: "vectorscale" },
    { id: "weighted_1", type: "vectorscale" },
    { id: "weighted_2", type: "vectorscale" },
    { id: "weighted_sum_ab", type: "vectoradd" },
    { id: "weighted_sum", type: "vectoradd" },
    { id: "weight_sum", type: "add" },
    { id: "one", type: "constant", params: { value: 1 } },
    { id: "inv_weight", type: "divide" },
    { id: "average", type: "vectorscale" },
    {
      id: "sum_out",
      type: "output",
      params: { path: "samples/weighted.sum" },
    },
    {
      id: "avg_out",
      type: "output",
      params: { path: "samples/weighted.average" },
    },
    {
      id: "total_out",
      type: "output",
      params: { path: "samples/weighted.total" },
    },
  ],
  edges: [
    edge("targets", "weighted_0", "v", {
      selector: [{ index: 0 }, { field: "value" }],
    }),
    edge("targets", "weighted_0", "scalar", {
      selector: [{ index: 0 }, { field: "weight" }],
    }),
    edge("targets", "weighted_1", "v", {
      selector: [{ index: 1 }, { field: "value" }],
    }),
    edge("targets", "weighted_1", "scalar", {
      selector: [{ index: 1 }, { field: "weight" }],
    }),
    edge("targets", "weighted_2", "v", {
      selector: [{ index: 2 }, { field: "value" }],
    }),
    edge("targets", "weighted_2", "scalar", {
      selector: [{ index: 2 }, { field: "weight" }],
    }),
    edge("weighted_0", "weighted_sum_ab", "a"),
    edge("weighted_1", "weighted_sum_ab", "b"),
    edge("weighted_sum_ab", "weighted_sum", "a"),
    edge("weighted_2", "weighted_sum", "b"),
    edge("targets", "weight_sum", "weight_0", {
      selector: [{ index: 0 }, { field: "weight" }],
    }),
    edge("targets", "weight_sum", "weight_1", {
      selector: [{ index: 1 }, { field: "weight" }],
    }),
    edge("targets", "weight_sum", "weight_2", {
      selector: [{ index: 2 }, { field: "weight" }],
    }),
    edge("one", "inv_weight", "lhs"),
    edge("weight_sum", "inv_weight", "rhs"),
    edge("weighted_sum", "average", "v"),
    edge("inv_weight", "average", "scalar"),
    edge("weighted_sum", "sum_out", "in"),
    edge("average", "avg_out", "in"),
    edge("weight_sum", "total_out", "in"),
  ],
};


/**
 * All bundled graph samples keyed by short name.
 *
 * Use this map to enumerate available sample graphs or fetch a spec by name.
 */
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
