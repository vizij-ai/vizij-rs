# Implementation Plan

[Overview]
Add two new IK nodes that use the Rust “k” crate to solve inverse kinematics from a URDF provided as a string, outputting joint states as a record mapping joint names to angles. Nodes will support both position-only IK and full 6-DoF (position + orientation), and will be feature-gated to keep WASM builds opt-in.

The vizij-graph-core evaluator will be extended with new NodeType variants and schema signatures for both IK nodes. The IK runtime will parse the URDF string, construct a kinematics chain (base → tip), run an iterative Jacobian-based solver, and produce joint solutions. To avoid excessive per-frame parsing, a per-node cached runtime state will store the constructed kinematics chain and metadata, invalidating when inputs/params change (URDF content or chain selection). Output will be a Value::Record of joint_name → Value::Float(angle_radians). The feature gate “urdf_ik” will make this optional for consumers (including WASM), with clear pass-through enabling in dependent crates.

[Types]  
Introduce new NodeType variants and extend NodeParams and runtime state.

Detailed type definitions, interfaces, enums, or data structures with complete specifications. Include field names, types, validation rules, and relationships.

1) NodeType additions (vizij-rs/crates/node-graph/vizij-graph-core/src/types.rs)
- enum NodeType:
  - UrdfIkPosition
  - UrdfIkPose

2) NodeParams additions (vizij-rs/crates/node-graph/vizij-graph-core/src/types.rs)
- Add new serde-compatible fields (all optional, with #[serde(default)] on the struct still effective):
  - urdf_xml: Option<String>
    - The complete URDF XML string.
  - root_link: Option<String>
    - Defaults to "base_link" if None.
  - tip_link: Option<String>
    - Defaults to the last link or "tool0" if None.
  - seed: Option<Vec<f32>>
    - Initial joint state seed; length matches the chain DoFs. If None, defaults to zeros.
  - weights: Option<Vec<f32>>
    - Optional per-joint weights; if provided, length must equal DoFs; otherwise uniform weights = 1.0.
  - max_iters: Option<u32>
    - Iteration cap; default 100.
  - tol_pos: Option<f32>
    - Position tolerance (meters); default 1e-3.
  - tol_rot: Option<f32>
    - Orientation tolerance (radians); default 1e-3. Ignored for position-only node.

Validation:
- If weights is provided, len(weights) must equal DoFs; otherwise, fallback to 1.0 weights or error (see Functions).
- If seed is provided but len(seed) != DoFs, either resize with clamp or error (see Functions).
- If urdf_xml is None or empty, evaluation errors.

3) Runtime state (vizij-rs/crates/node-graph/vizij-graph-core/src/eval.rs)
- Extend NodeRuntimeState:
  - UrdfIk(UrdfIkState)

- Define struct UrdfIkState:
  - hash: u64                         // hash of (urdf_xml, root_link, tip_link) to detect rebuild
  - dofs: usize
  - joint_names: Vec<String>          // in solver order
  - chain: k::Chain<f32>              // kinematic chain (cfg(feature = "urdf_ik"))
  - solver: k::JacobianIKSolver<f32>  // configured with tolerances/weights (cfg(feature = "urdf_ik"))

[Files]
Add feature-gated IK support, new NodeType variants, schema registry entries, and evaluation logic.

Detailed breakdown:
- New files to be created (with full paths and purpose)
  - None required; all changes are within existing modules.

- Existing files to be modified (with specific changes)
  - vizij-rs/crates/node-graph/vizij-graph-core/Cargo.toml
    - Add optional dependency:
      - k = { version = "0.28", optional = true, default-features = false }
      - urdf-rs = { version = "0.7", optional = true }
    - Add feature:
      - [features]
        urdf_ik = ["k", "urdf-rs"]
      - Ensure no default features change unless desired. Keep the feature disabled by default.
  - vizij-rs/crates/node-graph/vizij-graph-core/src/types.rs
    - Add NodeType variants: UrdfIkPosition, UrdfIkPose.
    - Extend NodeParams with: urdf_xml, root_link, tip_link, seed, weights, max_iters, tol_pos, tol_rot (all Option<...>).
  - vizij-rs/crates/node-graph/vizij-graph-core/src/schema.rs
    - Optionally extend PortType with an “Any” variant to document record-typed outputs. If not extending, use Vector with doc note; preferred: add Any.
      - pub enum PortType { Float, Bool, Vec3, Vector, Any }
    - Add two NodeSignature entries (category "Robotics"):
      - UrdfIkPosition:
        - Inputs:
          - target_pos: Vec3 (required)
          - seed: Vector (optional; default [])
        - Outputs:
          - out: Any (doc: "Record: joint_name → angle_radians")
        - Params:
          - urdf_xml: Any (default_json: {"text": ""})
          - root_link: Any (default_json: {"text": "base_link"})
          - tip_link: Any (default_json: {"text": "tool0"})
          - weights: Vector (default_json: {"vector": []})
          - max_iters: Float (default_json: {"float": 100.0})
          - tol_pos: Float (default_json: {"float": 0.001})
      - UrdfIkPose:
        - Inputs:
          - target_pos: Vec3 (required)
          - target_rot: Vector (required; document as quaternion [x,y,z,w])
          - seed: Vector (optional; default [])
        - Outputs:
          - out: Any (doc: "Record: joint_name → angle_radians")
        - Params:
          - urdf_xml: Any (default_json: {"text": ""})
          - root_link: Any (default_json: {"text": "base_link"})
          - tip_link: Any (default_json: {"text": "tool0"})
          - weights: Vector (default_json: {"vector": []})
          - max_iters: Float (default_json: {"float": 100.0})
          - tol_pos: Float (default_json: {"float": 0.001})
          - tol_rot: Float (default_json: {"float": 0.001})
    - Registry: push these nodes only when cfg(feature = "urdf_ik") to avoid exposing nodes when feature disabled.
  - vizij-rs/crates/node-graph/vizij-graph-core/src/eval.rs
    - Extend NodeRuntimeState with UrdfIk(UrdfIkState).
    - Add GraphRuntime::ik_state_mut(...) helper like spring/damp/slew to manage cached IK state per node (cfg(feature = "urdf_ik")).
    - In eval_node match:
      - Add arms for NodeType::UrdfIkPosition and NodeType::UrdfIkPose under cfg(feature = "urdf_ik"):
        - Read params: urdf_xml, root_link, tip_link, weights, max_iters, tol_pos, tol_rot (if applicable).
        - Read inputs: target_pos (Vec3), target_rot (Quat stored in Vector/Quat), seed (Vector).
        - Validate and/or normalize inputs (lengths, defaults).
        - Build or reuse IK state: recompute if (hash(urdf_xml, root, tip)) changes.
        - Solve IK with k::JacobianIKSolver; for pose, include orientation objective; for position-only, position objective only.
        - Collect joint_name → angle and produce Value::Record(HashMap<String, Value::Float>).
      - Under cfg(not(feature = "urdf_ik")), include stub arms that return Err("URDF IK feature not enabled") if these NodeTypes ever appear.
  - vizij-rs/crates/node-graph/vizij-graph-wasm/Cargo.toml
    - Add a pass-through optional feature to allow enabling IK in WASM builds, but keep it enabled by default as it is commonly used:
      - [features]
        urdf_ik = ["vizij-graph-core/urdf_ik"]
    - Note: If “k” and “urdf-rs” compile to wasm32-unknown-unknown in your toolchain, this can be enabled by consumers.

- Files to be deleted or moved
  - None.

- Configuration file updates
  - None beyond Cargo.toml feature/deps additions.

[Functions]
Add two evaluation paths and internal helpers for IK.

Detailed breakdown:
- New functions (name, signature, file path, purpose)
  - ik_state_mut(&mut self, node_id: &NodeId, key: &IkKey) -> &mut UrdfIkState
    - Path: vizij-graph-core/src/eval.rs (impl GraphRuntime)
    - Purpose: Get or initialize cached IK state for a node. IkKey includes (hash, base, tip).
  - build_chain_from_urdf(urdf_xml: &str, root: &str, tip: &str) -> Result<(k::Chain<f32>, Vec<String>), String>
    - Path: eval.rs (cfg(feature = "urdf_ik"))
    - Purpose: Parse URDF string and construct a kinematics chain with ordered joint names.

  - solve_position(chain: &mut k::Chain<f32>, solver: &mut k::JacobianIKSolver<f32>, target_pos: [f32; 3], seed: &[f32], weights: Option<&[f32]>, max_iters: u32, tol_pos: f32) -> Result<Vec<f32>, String>
    - Path: eval.rs (cfg(feature = "urdf_ik"))
    - Purpose: Run position-only IK to generate joint angles.

  - solve_pose(chain: &mut k::Chain<f32>, solver: &mut k::JacobianIKSolver<f32>, target_pos: [f32; 3], target_rot: [f32; 4], seed: &[f32], weights: Option<&[f32]>, max_iters: u32, tol_pos: f32, tol_rot: f32) -> Result<Vec<f32>, String>
    - Path: eval.rs (cfg(feature = "urdf_ik"))
    - Purpose: Run full 6-DoF IK.

- Modified functions (exact name, current file path, required changes)
  - eval_node(...) in eval.rs
    - Add match arms for UrdfIkPosition and UrdfIkPose under cfg(feature = "urdf_ik"); add cfg-not-enabled arms that return Err.
  - GraphRuntime struct and impl in eval.rs
    - Extend NodeRuntimeState enum + add ik_state_mut accessor.

- Removed functions (name, file path, reason, migration strategy)
  - None.

[Classes]
No new classes (Rust structs/enums added above).

Detailed breakdown:
- New classes (name, file path, key methods, inheritance)
  - UrdfIkState (eval.rs)
    - Fields: hash, dofs, joint_names, chain, solver.
- Modified classes (exact name, file path, specific modifications)
  - NodeRuntimeState (eval.rs): add UrdfIk variant.
- Removed classes
  - None.

[Dependencies]
Introduce optional dependencies and a feature gate to keep IK opt-in.

Details of new packages, version changes, and integration requirements.
- vizij-graph-core/Cargo.toml:
  - k = { version = "0.32", optional = true, default-features = false }
  - urdf-rs = { version = "0.9", optional = true }
  - [features] urdf_ik = ["k", "urdf-rs"]
- vizij-graph-wasm/Cargo.toml:
  - [features] urdf_ik = ["vizij-graph-core/urdf_ik"]
Notes:
- If “k” and “urdf-rs” do not compile to wasm32-unknown-unknown in your environment, disable the feature in the wasm crate as needed. Keeping it optional prevents unexpected wasm size increases or incompatibilities but it will be enabled by default as it is pure rust and should compile easily, and it is a core function for many graphs.
- k depends on nalgebra; no direct addition required.

[Testing]
Add feature-gated unit tests using a minimal URDF.

Test file requirements, existing test modifications, and validation strategies.
- New tests (cfg(feature = "urdf_ik")) in vizij-graph-core/src/eval.rs tests module:
  - Position-only IK sanity:
    - Provide a tiny 2–3 joint planar URDF string inline.
    - Build a graph with one UrdfIkPosition node (urdf_xml param; root_link; tip_link).
    - Target a reachable position. Assert:
      - eval returns Ok(...)
      - Output is Value::Record with expected joint keys.
      - Forward kinematics from solution positions reaches within tol_pos.
  - Pose IK sanity:
    - Same URDF; add target_rot quaternion (identity or small rotation).
    - Assert both pos and orientation errors below tol_pos / tol_rot (approx).
- Negative tests:
  - Malformed URDF produces Err(...) with message.
  - Wrong seed length: ensure graceful handling (truncate/pad or error; choose one policy and test it).
  - weights length mismatch: fallback to uniform weights or error; test the chosen behavior.

[Implementation Order]
Implement feature gating and type changes first, then schema and eval, and finally tests.

Numbered steps showing the logical order of changes to minimize conflicts and ensure successful integration.
1) Add feature and dependencies in vizij-graph-core/Cargo.toml (“urdf_ik” with optional k, urdf-rs). Add wasm crate feature pass-through (optional).
2) Update NodeType (add UrdfIkPosition, UrdfIkPose) and extend NodeParams with new fields.
3) Update schema.rs:
   - Optionally add PortType::Any.
   - Add NodeSignature entries for both nodes under cfg(feature = "urdf_ik").
4) Implement runtime support in eval.rs:
   - Extend NodeRuntimeState and add UrdfIkState and ik_state_mut.
   - Under cfg(feature = "urdf_ik"), add eval_node arms:
     - Build/reuse chain state from URDF string, root_link, tip_link.
     - Solve IK using Jacobian solver (position-only and pose).
     - Output Value::Record of joint_name → Value::Float(angle).
   - Under cfg(not(feature = "urdf_ik")), add arms returning Err with a clear message if invoked.
5) Add unit tests (cfg(feature = "urdf_ik")) with a minimal inline URDF covering both nodes and common edge cases.
6) Verify workspace builds:
   - cargo build -p vizij-graph-core
   - cargo test -p vizij-graph-core --features urdf_ik
   - test wasm build with feature on
