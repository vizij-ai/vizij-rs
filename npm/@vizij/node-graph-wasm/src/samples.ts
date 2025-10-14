import type {
  GraphSpec,
  LinkSpec,
  SelectorSegmentJSON,
  ValueJSON,
} from "./types";

const link = (
  from: string,
  to: string,
  input: string,
  options: { output?: string; selector?: SelectorSegmentJSON[] } = {},
): LinkSpec => ({
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
  links: [
    link("freq", "osc", "frequency"),
    link("time", "osc", "phase"),
    link("osc", "add1", "a"),
    link("offset", "add1", "b"),
    link("add1", "clamp1", "in"),
    link("const0", "clamp1", "min"),
    link("clamp_max", "clamp1", "max"),
    link("clamp1", "remap1", "in"),
    link("remap_in_min", "remap1", "in_min"),
    link("remap_in_max", "remap1", "in_max"),
    link("remap_out_min", "remap1", "out_min"),
    link("remap_out_max", "remap1", "out_max"),
    link("remap1", "out", "in"),
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
  links: [
    link("v1_in", "vadd", "a"),
    link("v2_in", "vadd", "b"),
    link("v1_in", "vsub", "a"),
    link("v2_in", "vsub", "b"),
    link("v2_in", "vnorm", "in"),
    link("vadd", "vdot", "a"),
    link("vnorm", "vdot", "b"),
    link("vadd", "vlen", "in"),
    link("vadd", "out_sum", "in"),
    link("vsub", "out_sub", "in"),
    link("vdot", "out_dot", "in"),
    link("vlen", "out_len", "in"),
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
  links: [
    link("time", "sin", "in"),
    link("sin", "greater", "lhs"),
    link("threshold", "greater", "rhs"),
    link("greater", "gate", "cond"),
    link("then", "gate", "then"),
    link("else", "gate", "else"),
    link("gate", "out", "in"),
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
  links: [
    link("pair", "spring_pos", "in", { selector: [{ index: 0 }] }),
    link("pair", "spring_rot", "in", { selector: [{ index: 1 }] }),
    link("pair", "damp_pos", "in", { selector: [{ index: 0 }] }),
    link("pair", "damp_rot", "in", { selector: [{ index: 1 }] }),
    link("pair", "slew_pos", "in", { selector: [{ index: 0 }] }),
    link("pair", "slew_rot", "in", { selector: [{ index: 1 }] }),
    link("spring_pos", "join_spring", "a"),
    link("spring_rot", "join_spring", "b"),
    link("damp_pos", "join_damp", "a"),
    link("damp_rot", "join_damp", "b"),
    link("slew_pos", "join_slew", "a"),
    link("slew_rot", "join_slew", "b"),
    link("join_spring", "out_spring", "in"),
    link("join_damp", "out_damp", "in"),
    link("join_slew", "out_slew", "in"),
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
  links: [
    link("payload", "accel_corrected", "a", {
      selector: [{ field: "sensors" }, { field: "accel" }],
    }),
    link("payload", "accel_corrected", "b", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 0 },
      ],
    }),
    link("payload", "gyro_blended", "a", {
      selector: [{ field: "sensors" }, { field: "gyro" }],
    }),
    link("payload", "gyro_blended", "b", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 1 },
      ],
    }),
    link("accel_corrected", "telemetry_join", "segment_1"),
    link("gyro_blended", "telemetry_join", "segment_2"),
    link("payload", "calibration_pack", "segment_1", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 0 },
      ],
    }),
    link("payload", "calibration_pack", "segment_2", {
      selector: [
        { field: "calibration" },
        { field: "offsets" },
        { index: 1 },
      ],
    }),
    link("payload", "gain0_x", "v", {
      selector: [
        { field: "calibration" },
        { field: "gains" },
        { index: 0 },
      ],
    }),
    link("zero", "gain0_x", "index"),
    link("payload", "gain1_x", "v", {
      selector: [
        { field: "calibration" },
        { field: "gains" },
        { index: 1 },
      ],
    }),
    link("zero", "gain1_x", "index"),
    link("gain0_x", "gain_sum", "lhs"),
    link("gain1_x", "gain_sum", "rhs"),
    link("gain_sum", "gain_avg", "lhs"),
    link("two", "gain_avg", "rhs"),
    link("telemetry_join", "telemetry_vector_out", "in"),
    link("gain_avg", "telemetry_gain_out", "in"),
    link("calibration_pack", "telemetry_offsets_out", "in"),
    link("payload", "telemetry_label_out", "in", {
      selector: [{ field: "metadata" }, { field: "label" }],
    }),
    link("payload", "telemetry_active_out", "in", {
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
  links: [
    link("config", "limb0", "scalar", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 0 },
        { field: "weight" },
      ],
    }),
    link("config", "limb0", "v", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 0 },
        { field: "offset" },
      ],
    }),
    link("config", "limb1", "scalar", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 1 },
        { field: "weight" },
      ],
    }),
    link("config", "limb1", "v", {
      selector: [
        { field: "rig" },
        { field: "limbs" },
        { index: 1 },
        { field: "offset" },
      ],
    }),
    link("limb0", "limb_sum", "a"),
    link("limb1", "limb_sum", "b"),
    link("config", "pose_sum", "a", {
      selector: [{ field: "rig" }, { field: "root" }],
    }),
    link("limb_sum", "pose_sum", "b"),
    link("config", "harmonic0", "operands_1", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 0 },
        { field: "amplitude" },
      ],
    }),
    link("config", "harmonic0", "operands_2", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 0 },
        { field: "frequency" },
      ],
    }),
    link("config", "harmonic1", "operands_1", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 1 },
        { field: "amplitude" },
      ],
    }),
    link("config", "harmonic1", "operands_2", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "harmonics" },
        { index: 1 },
        { field: "frequency" },
      ],
    }),
    link("config", "phase_sum", "operands_1", {
      selector: [{ field: "rig" }, { field: "controls" }, { field: "phase" }],
    }),
    link("harmonic0", "phase_sum", "operands_2"),
    link("harmonic1", "phase_sum", "operands_3"),
    link("phase_sum", "target_scaled", "scalar"),
    link("config", "target_scaled", "v", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "localTarget" },
        { index: 1 },
      ],
    }),
    link("config", "target_combined", "a", {
      selector: [
        { field: "rig" },
        { field: "controls" },
        { field: "localTarget" },
        { index: 0 },
      ],
    }),
    link("target_scaled", "target_combined", "b"),
    link("pose_sum", "pose_mix", "a"),
    link("target_combined", "pose_mix", "b"),
    link("pose_sum", "out_pose", "in"),
    link("phase_sum", "out_phase", "in"),
    link("target_combined", "out_target", "in"),
    link("pose_mix", "out_pose_mix", "in"),
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
  links: [
    link("payload", "primary_sum", "operands_1", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 0 },
        { field: "values" },
        { index: 0 },
      ],
    }),
    link("payload", "primary_sum", "operands_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 0 },
        { field: "values" },
        { index: 2 },
      ],
    }),
    link("primary_sum", "primary_weighted", "operands_1"),
    link("payload", "primary_weighted", "operands_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 0 },
        { field: "weight" },
      ],
    }),
    link("payload", "secondary_sum", "operands_1", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 1 },
        { field: "values" },
        { index: 0 },
      ],
    }),
    link("payload", "secondary_sum", "operands_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 1 },
        { field: "values" },
        { index: 1 },
      ],
    }),
    link("secondary_sum", "secondary_mean", "lhs"),
    link("two", "secondary_mean", "rhs"),
    link("secondary_mean", "secondary_weighted", "operands_1"),
    link("payload", "secondary_weighted", "operands_2", {
      selector: [
        { field: "metrics" },
        { field: "nested" },
        { index: 1 },
        { field: "weight" },
      ],
    }),
    link("payload", "offset_component", "v", {
      selector: [{ field: "offsets" }],
    }),
    link("two", "offset_component", "index"),
    link("payload", "gated_bias", "cond", {
      selector: [{ field: "toggle" }],
    }),
    link("offset_component", "gated_bias", "then"),
    link("zero", "gated_bias", "else"),
    link("primary_weighted", "final_score", "operands_1"),
    link("secondary_weighted", "final_score", "operands_2"),
    link("gated_bias", "final_score", "operands_3"),
    link("final_score", "out_score", "in"),
    link("secondary_mean", "out_secondary", "in"),
    link("primary_weighted", "out_primary", "in"),
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
  links: [
    link("rig_config", "layer0_scaled", "v", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "offset" }],
    }),
    link("rig_config", "layer0_scaled", "scalar", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
    }),
    link("rig_config", "layer1_scaled", "v", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "offset" }],
    }),
    link("rig_config", "layer1_scaled", "scalar", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
    }),
    link("layer0_scaled", "layer_sum", "a"),
    link("layer1_scaled", "layer_sum", "b"),
    link("layer_sum", "gain_scale", "v"),
    link("rig_config", "gain_scale", "scalar", {
      selector: [{ field: "gain" }],
    }),
    link("gain_scale", "pose_result", "a"),
    link("rig_config", "pose_result", "b", {
      selector: [{ field: "base_pose" }],
    }),
    link("rig_config", "weight_sum", "a", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
    }),
    link("rig_config", "weight_sum", "b", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
    }),
    link("rig_config", "weights_vector", "first", {
      selector: [{ field: "layers" }, { index: 0 }, { field: "weight" }],
    }),
    link("rig_config", "weights_vector", "second", {
      selector: [{ field: "layers" }, { index: 1 }, { field: "weight" }],
    }),
    link("weight_sum", "weights_vector", "total"),
    link("pose_result", "out_pose", "in"),
    link("weights_vector", "out_weights", "in"),
    link("rig_config", "out_tags", "in", {
      selector: [{ field: "info" }, { field: "tags" }],
    }),
    link("rig_config", "out_counters", "in", {
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
  links: [
    link("rig", "ctrl0", "v", {
      selector: [
        { field: "controls" },
        { index: 0 },
        { field: "offset" },
      ],
    }),
    link("rig", "ctrl0", "scalar", {
      selector: [
        { field: "controls" },
        { index: 0 },
        { field: "weight" },
      ],
    }),
    link("rig", "ctrl1", "v", {
      selector: [
        { field: "controls" },
        { index: 1 },
        { field: "offset" },
      ],
    }),
    link("rig", "ctrl1", "scalar", {
      selector: [
        { field: "controls" },
        { index: 1 },
        { field: "weight" },
      ],
    }),
    link("ctrl0", "combined", "a"),
    link("ctrl1", "combined", "b"),
    link("combined", "biased", "a"),
    link("rig", "biased", "b", { selector: [{ field: "bias" }] }),
    link("biased", "offset_split", "in"),
    link("rig", "weight_component", "v", {
      selector: [{ field: "weights" }],
    }),
    link("component_index", "weight_component", "index"),
    link("rig", "aim_diff", "a", {
      selector: [{ field: "aim" }, { index: 1 }],
    }),
    link("rig", "aim_diff", "b", {
      selector: [{ field: "aim" }, { index: 0 }],
    }),
    link("aim_diff", "aim_distance", "in"),
    link("biased", "pose_join", "a"),
    link("aim_diff", "pose_join", "b"),
    link("pose_join", "pose_out", "in"),
    link("offset_split", "offset_xy_out", "in", { output: "part1" }),
    link("offset_split", "offset_z_out", "in", {
      output: "part2",
      selector: [{ index: 0 }],
    }),
    link("aim_distance", "aim_distance_out", "in"),
    link("weight_component", "weight_component_out", "in"),
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
  links: [
    link("targets", "weighted_0", "v", {
      selector: [{ index: 0 }, { field: "value" }],
    }),
    link("targets", "weighted_0", "scalar", {
      selector: [{ index: 0 }, { field: "weight" }],
    }),
    link("targets", "weighted_1", "v", {
      selector: [{ index: 1 }, { field: "value" }],
    }),
    link("targets", "weighted_1", "scalar", {
      selector: [{ index: 1 }, { field: "weight" }],
    }),
    link("targets", "weighted_2", "v", {
      selector: [{ index: 2 }, { field: "value" }],
    }),
    link("targets", "weighted_2", "scalar", {
      selector: [{ index: 2 }, { field: "weight" }],
    }),
    link("weighted_0", "weighted_sum_ab", "a"),
    link("weighted_1", "weighted_sum_ab", "b"),
    link("weighted_sum_ab", "weighted_sum", "a"),
    link("weighted_2", "weighted_sum", "b"),
    link("targets", "weight_sum", "weight_0", {
      selector: [{ index: 0 }, { field: "weight" }],
    }),
    link("targets", "weight_sum", "weight_1", {
      selector: [{ index: 1 }, { field: "weight" }],
    }),
    link("targets", "weight_sum", "weight_2", {
      selector: [{ index: 2 }, { field: "weight" }],
    }),
    link("one", "inv_weight", "lhs"),
    link("weight_sum", "inv_weight", "rhs"),
    link("weighted_sum", "average", "v"),
    link("inv_weight", "average", "scalar"),
    link("weighted_sum", "sum_out", "in"),
    link("average", "avg_out", "in"),
    link("weight_sum", "total_out", "in"),
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
