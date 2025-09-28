import type { GraphSpec } from "./types";

/**
 * Minimal URDF IK Position sample.
 * Default URDF/XML fields are left empty so the host UI can populate them at runtime.
 */
export const urdfIkPosition: GraphSpec = {
  nodes: [
    {
      id: "target_pos",
      type: "input",
      params: {
        path: "samples/urdf.target_pos",
        value: { vec3: [0, 0.6, 0] },
      },
    },
    {
      id: "seed",
      type: "vectorconstant",
      params: { value: { vector: [0, 0, 0, 0, 0, 0] } },
    },
    {
      id: "ik",
      type: "urdfikposition",
      params: {
        urdf_xml: "",
        root_link: "",
        tip_link: "",
        max_iters: 128,
        tol_pos: 0.005,
      },
      inputs: {
        target_pos: { node_id: "target_pos" },
        seed: { node_id: "seed" },
      },
    },
    {
      id: "ik_out",
      type: "output",
      params: { path: "samples/urdf.angles" },
      inputs: { in: { node_id: "ik" } },
    },
  ],
};

export const urdfGraphSamples: Record<string, GraphSpec> = {
  "urdf-ik-position": urdfIkPosition,
};
