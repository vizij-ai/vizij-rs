import type {
  GraphSpec,
  LinkSpec,
  SelectorSegmentJSON,
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
    },
    {
      id: "ik_out",
      type: "output",
      params: { path: "samples/urdf.angles" },
    },
  ],
  links: [
    link("target_pos", "ik", "target_pos"),
    link("seed", "ik", "seed"),
    link("ik", "ik_out", "in"),
  ],
};

export const urdfGraphSamples: Record<string, GraphSpec> = {
  "urdf-ik-position": urdfIkPosition,
};
