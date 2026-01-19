use crate::types::NodeType;
use serde::{Deserialize, Serialize};

/// Supported port value categories for the node registry.
///
/// These categories are a UI/schema abstraction; runtime evaluation still uses
/// `vizij_api_core::Value`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortType {
    /// Scalar float inputs/outputs.
    Float,
    /// Boolean inputs/outputs.
    Bool,
    /// 3D vector inputs/outputs.
    Vec3,
    /// Quaternion inputs/outputs.
    Quat,
    /// Transform inputs/outputs.
    Transform,
    /// Numeric vector inputs/outputs.
    Vector,
    /// Arbitrary value inputs/outputs.
    Any,
}

/// Supported parameter value categories for the node registry.
///
/// Parameter categories are used for tooling and JSON schema generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    /// Scalar float parameters.
    Float,
    /// Boolean parameters.
    Bool,
    /// Vec3 parameters.
    Vec3,
    /// Numeric vector parameters.
    Vector,
    /// Arbitrary value parameters.
    Any, // union (Value)
}

/// Static port metadata used by the schema registry.
///
/// This metadata is consumed by editor tooling and wasm bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    /// Stable port identifier used in JSON and bindings.
    pub id: &'static str,
    /// Expected port value category.
    pub ty: PortType,
    /// Human-readable label for UI surfaces.
    pub label: &'static str,
    /// Short documentation string displayed in tooling.
    #[serde(default)]
    pub doc: &'static str,
    /// Whether the port can be left unconnected.
    #[serde(default)]
    pub optional: bool,
}

/// Definition for variadic input or output groups.
///
/// Variadic groups describe slots such as `operand_0`, `operand_1`, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariadicSpec {
    /// Variadic group identifier (prefix for ports).
    pub id: &'static str,
    /// Expected value category for ports in the group.
    pub ty: PortType,
    /// Human-readable label for UI surfaces.
    pub label: &'static str,
    /// Short documentation string displayed in tooling.
    #[serde(default)]
    pub doc: &'static str,
    /// Minimum number of ports required.
    pub min: usize,
    /// Optional maximum number of ports allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
}

/// Static parameter metadata used by the schema registry.
///
/// Use `default_json` to match the Value JSON encoding used in wasm bindings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Stable parameter identifier used in JSON and bindings.
    pub id: &'static str,
    /// Expected parameter value category.
    pub ty: ParamType,
    /// Human-readable label for UI surfaces.
    pub label: &'static str,
    /// Short documentation string displayed in tooling.
    #[serde(default)]
    pub doc: &'static str,
    /// Default JSON value (Value JSON encoding).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_json: Option<serde_json::Value>,
    /// Optional minimum (for numeric params).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Optional maximum (for numeric params).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// Signature describing a node's ports, parameters, and documentation.
///
/// Signatures form the node registry that UI tooling consumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSignature {
    /// Unique node type id for the signature.
    pub type_id: NodeType,
    /// Display name for the node.
    pub name: &'static str,
    /// Category used by UI tooling.
    pub category: &'static str,
    /// Short documentation string displayed in tooling.
    #[serde(default)]
    pub doc: &'static str,
    /// Fixed input port definitions.
    pub inputs: Vec<PortSpec>,
    /// Optional variadic input group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variadic_inputs: Option<VariadicSpec>,
    /// Fixed output port definitions.
    pub outputs: Vec<PortSpec>,
    /// Optional variadic output group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variadic_outputs: Option<VariadicSpec>,
    /// Parameter definitions for this node.
    pub params: Vec<ParamSpec>,
}

/// Registry of known node signatures for tooling and validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    /// Registry schema version.
    pub version: &'static str,
    /// Registered node signatures.
    pub nodes: Vec<NodeSignature>,
}

// Helpers
fn p_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Float,
        label: "In",
        doc: "Input scalar value.",
        optional: false,
    }
}
fn p_bool_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Bool,
        label: "In",
        doc: "Input boolean value.",
        optional: false,
    }
}
fn p_vector_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Vector,
        label: "In",
        doc: "Input numeric vector; accepts scalars and arrays.",
        optional: false,
    }
}
fn p_out_float() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Float,
        label: "Out",
        doc: "Computed scalar result.",
        optional: false,
    }
}
fn p_out_bool() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Bool,
        label: "Out",
        doc: "Computed boolean result.",
        optional: false,
    }
}
fn p_out_vec3() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Vec3,
        label: "Out",
        doc: "Computed 3D vector.",
        optional: false,
    }
}
fn p_out_vector() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Vector,
        label: "Out",
        doc: "Computed numeric vector result.",
        optional: false,
    }
}

/// Return the built-in node registry for tooling and validation.
///
/// The registry mirrors the built-in node types and is consumed by UI tooling,
/// wasm bindings, and schema-aware validators.
pub fn registry() -> Registry {
    use NodeType::*;
    let mut nodes: Vec<NodeSignature> = Vec::new();

    // Scalars / arithmetic (float-based legacy kept for convenience)
    nodes.push(NodeSignature {
        type_id: Constant,
        name: "Constant",
        category: "Math",
        doc: "Outputs the configured value every frame; defaults to 0.0 when unspecified.",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "value",
            ty: ParamType::Any,
            label: "Value",
            doc: "Value to emit on the output port each tick.",
            default_json: Some(serde_json::json!({ "float": 0.0 })),
            min: None,
            max: None,
        }],
    });

    nodes.push(NodeSignature {
        type_id: Slider,
        name: "Slider",
        category: "Math",
        doc: "Provides a tunable scalar value constrained to the configured min/max range.",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![
            ParamSpec {
                id: "value",
                ty: ParamType::Float,
                label: "Value",
                doc: "Initial slider position.",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "min",
                ty: ParamType::Float,
                label: "Min",
                doc: "Lower bound for the slider value.",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "max",
                ty: ParamType::Float,
                label: "Max",
                doc: "Upper bound for the slider value.",
                default_json: Some(serde_json::json!({ "float": 1.0 })),
                min: None,
                max: None,
            },
        ],
    });

    nodes.push(NodeSignature {
        type_id: MultiSlider,
        name: "Multi Slider",
        category: "Math",
        doc: "Provides three independent slider-controlled scalar outputs for X, Y, and Z.",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![
            PortSpec {
                id: "x",
                ty: PortType::Float,
                label: "X",
                doc: "Current X slider value.",
                optional: false,
            },
            PortSpec {
                id: "y",
                ty: PortType::Float,
                label: "Y",
                doc: "Current Y slider value.",
                optional: false,
            },
            PortSpec {
                id: "z",
                ty: PortType::Float,
                label: "Z",
                doc: "Current Z slider value.",
                optional: false,
            },
        ],
        variadic_outputs: None,
        params: vec![
            ParamSpec {
                id: "x",
                ty: ParamType::Float,
                label: "X",
                doc: "Initial X slider value.",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "y",
                ty: ParamType::Float,
                label: "Y",
                doc: "Initial Y slider value.",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "z",
                ty: ParamType::Float,
                label: "Z",
                doc: "Initial Z slider value.",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
        ],
    });

    nodes.push(NodeSignature {
        type_id: Add,
        name: "Add",
        category: "Math",
        doc: "Sums all incoming operands, treating missing inputs as 0.",
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Float,
            label: "Operand",
            doc: "Each scalar to include in the sum.",
            min: 2,
            max: None,
        }),
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Subtract,
        name: "Subtract",
        category: "Math",
        doc: "Subtracts RHS from LHS; missing inputs default to 0.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Minuend value.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Subtrahend value.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Multiply,
        name: "Multiply",
        category: "Math",
        doc: "Multiplies all incoming operands; missing inputs act as 1.",
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Float,
            label: "Operand",
            doc: "Each scalar to include in the product.",
            min: 2,
            max: None,
        }),
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Divide,
        name: "Divide",
        category: "Math",
        doc: "Divides LHS by RHS; division by zero yields NaN.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Dividend value.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Divisor value; zero produces NaN.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Modulo,
        name: "Modulo",
        category: "Math",
        doc: "Computes the remainder of LHS ÷ RHS; divisor 0 yields NaN.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Dividend value.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Divisor value; zero produces NaN.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Power,
        name: "Power",
        category: "Math",
        doc: "Raises Base to the given Exponent using f32 powf semantics.",
        inputs: vec![
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "Base value.",
                optional: false,
            },
            PortSpec {
                id: "exp",
                ty: PortType::Float,
                label: "Exponent",
                doc: "Exponent value.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Log,
        name: "Log",
        category: "Math",
        doc: "Computes logarithm of Value in the provided Base; invalid bases yield NaN.",
        inputs: vec![
            PortSpec {
                id: "value",
                ty: PortType::Float,
                label: "Value",
                doc: "Argument whose logarithm is evaluated.",
                optional: false,
            },
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "Logarithm base; non-positive or 1.0 results produce NaN.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Abs,
        name: "Abs",
        category: "Math",
        doc: "Outputs the absolute value of In component-wise.",
        inputs: vec![p_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Sqrt,
        name: "Sqrt",
        category: "Math",
        doc: "Outputs the square root of In; negative inputs yield NaN components.",
        inputs: vec![p_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Sign,
        name: "Sign",
        category: "Math",
        doc: "Outputs -1 for negative inputs, 0 for zero, and +1 for positive values.",
        inputs: vec![p_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Min,
        name: "Min",
        category: "Math",
        doc: "Returns the smallest scalar among all operands; missing inputs yield NaN.",
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Float,
            label: "Operand",
            doc: "Scalar operand to consider.",
            min: 2,
            max: None,
        }),
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Max,
        name: "Max",
        category: "Math",
        doc: "Returns the largest scalar among all operands; missing inputs yield NaN.",
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Float,
            label: "Operand",
            doc: "Scalar operand to consider.",
            min: 2,
            max: None,
        }),
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Round,
        name: "Round",
        category: "Math",
        doc: "Rounds In according to the selected Mode (floor, ceil, or trunc).",
        inputs: vec![p_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "round_mode",
            ty: ParamType::Any,
            label: "Mode",
            doc: "Rounding mode: \"floor\", \"ceil\", or \"trunc\".",
            default_json: Some(serde_json::json!({ "text": "floor" })),
            min: None,
            max: None,
        }],
    });

    for (nt, name, doc) in [
        (
            Sin,
            "Sin",
            "Computes the sine of the input angle (radians).",
        ),
        (
            Cos,
            "Cos",
            "Computes the cosine of the input angle (radians).",
        ),
        (
            Tan,
            "Tan",
            "Computes the tangent of the input angle (radians); results blow up near π/2 + kπ.",
        ),
    ] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
            category: "Math",
            doc,
            inputs: vec![p_in()],
            variadic_inputs: None,
            outputs: vec![p_out_float()],
            variadic_outputs: None,
            params: vec![],
        });
    }

    // Time & generators
    nodes.push(NodeSignature {
        type_id: Time,
        name: "Time",
        category: "Time",
        doc: "Outputs the graph runtime's elapsed seconds.",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Oscillator,
        name: "Oscillator",
        category: "Time",
        doc: "Generates a sine wave using the provided frequency and phase inputs.",
        inputs: vec![
            PortSpec {
                id: "frequency",
                ty: PortType::Float,
                label: "Frequency",
                doc: "Oscillation rate in Hz; accepts scalars or vectors.",
                optional: false,
            },
            PortSpec {
                id: "phase",
                ty: PortType::Float,
                label: "Phase",
                doc: "Phase offset in radians; broadcast across vector frequency inputs.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    // Transitions & smoothing
    nodes.push(NodeSignature {
        type_id: Spring,
        name: "Spring",
        category: "Transitions",
        doc: "Integrates a critically damped spring toward the Target; zero or non-finite dt snaps to Target.",
        inputs: vec![PortSpec {
            id: "in",
            ty: PortType::Vector,
            label: "Target",
            doc: "Target value to spring toward (scalar or vector).",
            optional: false,
        }],
        variadic_inputs: None,
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Vector,
            label: "Value",
            doc: "Spring-integrated value.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![
            ParamSpec {
                id: "stiffness",
                ty: ParamType::Float,
                label: "Stiffness",
                doc: "Hooke's spring constant controlling acceleration toward the target.",
                default_json: Some(serde_json::json!({ "float": 120.0 })),
                min: Some(0.0),
                max: None,
            },
            ParamSpec {
                id: "damping",
                ty: ParamType::Float,
                label: "Damping",
                doc: "Velocity damping applied each step.",
                default_json: Some(serde_json::json!({ "float": 20.0 })),
                min: Some(0.0),
                max: None,
            },
            ParamSpec {
                id: "mass",
                ty: ParamType::Float,
                label: "Mass",
                doc: "Effective mass of the spring system.",
                default_json: Some(serde_json::json!({ "float": 1.0 })),
                min: Some(0.0),
                max: None,
            },
        ],
    });

    nodes.push(NodeSignature {
        type_id: Damp,
        name: "Damp",
        category: "Transitions",
        doc: "Exponentially decays toward the Target using a configurable half-life; zero dt or half-life snaps to Target.",
        inputs: vec![PortSpec {
            id: "in",
            ty: PortType::Vector,
            label: "Target",
            doc: "Target value to smooth toward.",
            optional: false,
        }],
        variadic_inputs: None,
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Vector,
            label: "Value",
            doc: "Exponentially smoothed output.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "half_life",
            ty: ParamType::Float,
            label: "Half-Life",
            doc: "Seconds for the remaining error to halve.",
            default_json: Some(serde_json::json!({ "float": 0.2 })),
            min: Some(0.0),
            max: None,
        }],
    });

    nodes.push(NodeSignature {
        type_id: Slew,
        name: "Slew",
        category: "Transitions",
        doc: "Limits the rate of change toward Target using max_rate units per second; zero dt or max_rate snaps to Target.",
        inputs: vec![PortSpec {
            id: "in",
            ty: PortType::Vector,
            label: "Target",
            doc: "Target value to chase.",
            optional: false,
        }],
        variadic_inputs: None,
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Vector,
            label: "Value",
            doc: "Rate-limited output value.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "max_rate",
            ty: ParamType::Float,
            label: "Max Rate",
            doc: "Maximum units per second the value may change.",
            default_json: Some(serde_json::json!({ "float": 1.0 })),
            min: Some(0.0),
            max: None,
        }],
    });

    // Logic (Bool semantics)
    nodes.push(NodeSignature {
        type_id: And,
        name: "And",
        category: "Logic",
        doc: "Outputs true when both inputs are true; missing inputs default to false.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Bool,
                label: "LHS",
                doc: "Left-hand boolean operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Bool,
                label: "RHS",
                doc: "Right-hand boolean operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: Or,
        name: "Or",
        category: "Logic",
        doc: "Outputs true when either input is true; missing inputs default to false.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Bool,
                label: "LHS",
                doc: "Left-hand boolean operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Bool,
                label: "RHS",
                doc: "Right-hand boolean operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: Xor,
        name: "Xor",
        category: "Logic",
        doc: "Outputs true when exactly one input is true; missing inputs default to false.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Bool,
                label: "LHS",
                doc: "Left-hand boolean operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Bool,
                label: "RHS",
                doc: "Right-hand boolean operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: Not,
        name: "Not",
        category: "Logic",
        doc: "Outputs the logical negation of the input; missing input defaults to false.",
        inputs: vec![p_bool_in()],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });

    // Conditional (float comparisons)
    nodes.push(NodeSignature {
        type_id: GreaterThan,
        name: "Greater Than",
        category: "Logic",
        doc: "Outputs true when LHS is strictly greater than RHS.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Left-hand scalar operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Right-hand scalar operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: LessThan,
        name: "Less Than",
        category: "Logic",
        doc: "Outputs true when LHS is strictly less than RHS.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Left-hand scalar operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Right-hand scalar operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: Equal,
        name: "Equal",
        category: "Logic",
        doc: "Outputs true when LHS and RHS differ by less than 1e-6.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Left-hand scalar operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Right-hand scalar operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: NotEqual,
        name: "Not Equal",
        category: "Logic",
        doc: "Outputs true when LHS and RHS differ by more than 1e-6.",
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "Left-hand scalar operand.",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "Right-hand scalar operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });

    // If: schema uses Vector as a generic stand-in for the core union value type.
    nodes.push(NodeSignature {
        type_id: If,
        name: "If",
        category: "Logic",
        doc: "Routes Then when Condition is true, otherwise Else; missing branches default to 0.",
        inputs: vec![
            PortSpec {
                id: "cond",
                ty: PortType::Bool,
                label: "Condition",
                doc: "Boolean condition to evaluate.",
                optional: false,
            },
            PortSpec {
                id: "then",
                ty: PortType::Vector,
                label: "Then",
                doc: "Value emitted when Condition is true.",
                optional: true,
            },
            PortSpec {
                id: "else",
                ty: PortType::Vector,
                label: "Else",
                doc: "Value emitted when Condition is false.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_vector()],
        variadic_outputs: None,
        params: vec![],
    });

    // Ranges
    nodes.push(NodeSignature {
        type_id: Clamp,
        name: "Clamp",
        category: "Math",
        doc: "Constrains In between Min and Max; expects Min ≤ Max.",
        inputs: vec![
            PortSpec {
                id: "in",
                ty: PortType::Float,
                label: "In",
                doc: "Value to clamp.",
                optional: false,
            },
            PortSpec {
                id: "min",
                ty: PortType::Float,
                label: "Min",
                doc: "Lower bound.",
                optional: false,
            },
            PortSpec {
                id: "max",
                ty: PortType::Float,
                label: "Max",
                doc: "Upper bound.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Remap,
        name: "Remap",
        category: "Math",
        doc: "Normalizes In from the [In Min, In Max] range into [Out Min, Out Max]; input is clamped to the source range and divide-by-zero yields NaN.",
        inputs: vec![
            PortSpec {
                id: "in",
                ty: PortType::Float,
                label: "In",
                doc: "Value to remap.",
                optional: false,
            },
            PortSpec {
                id: "in_min",
                ty: PortType::Float,
                label: "In Min",
                doc: "Lower bound of the input range.",
                optional: false,
            },
            PortSpec {
                id: "in_max",
                ty: PortType::Float,
                label: "In Max",
                doc: "Upper bound of the input range.",
                optional: false,
            },
            PortSpec {
                id: "out_min",
                ty: PortType::Float,
                label: "Out Min",
                doc: "Lower bound of the output range.",
                optional: false,
            },
            PortSpec {
                id: "out_max",
                ty: PortType::Float,
                label: "Out Max",
                doc: "Upper bound of the output range.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: CenteredRemap,
        name: "Centered Remap",
        category: "Math",
        doc: "Linearly remaps In around an anchor without clamping: values ≤ Anchor use the [In Low, Anchor] span while values ≥ Anchor use [Anchor, In High]; spans may collapse to anchor to pin one side.",
        inputs: vec![
            PortSpec {
                id: "in",
                ty: PortType::Float,
                label: "In",
                doc: "Value to remap.",
                optional: false,
            },
            PortSpec {
                id: "in_low",
                ty: PortType::Float,
                label: "In Low",
                doc: "Reference point for values below the anchor.",
                optional: false,
            },
            PortSpec {
                id: "in_anchor",
                ty: PortType::Float,
                label: "In Anchor",
                doc: "Anchor point separating the low/high spans.",
                optional: false,
            },
            PortSpec {
                id: "in_high",
                ty: PortType::Float,
                label: "In High",
                doc: "Reference point for values above the anchor.",
                optional: false,
            },
            PortSpec {
                id: "out_low",
                ty: PortType::Float,
                label: "Out Low",
                doc: "Output mapped to In Low.",
                optional: false,
            },
            PortSpec {
                id: "out_anchor",
                ty: PortType::Float,
                label: "Out Anchor",
                doc: "Output corresponding to the anchor.",
                optional: false,
            },
            PortSpec {
                id: "out_high",
                ty: PortType::Float,
                label: "Out High",
                doc: "Output mapped to In High.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: PiecewiseRemap,
        name: "Piecewise Remap",
        category: "Math",
        doc: "Remaps In across ordered breakpoint pairs; supports n segments, optional clamping, and extrapolation when Clamp is disabled.",
        inputs: vec![
            PortSpec {
                id: "in",
                ty: PortType::Float,
                label: "In",
                doc: "Value to remap.",
                optional: false,
            },
            PortSpec {
                id: "input_breakpoints",
                ty: PortType::Vector,
                label: "Input Breakpoints",
                doc: "Ordered numeric domain breakpoints (length ≥ 2).",
                optional: false,
            },
            PortSpec {
                id: "output_breakpoints",
                ty: PortType::Vector,
                label: "Output Breakpoints",
                doc: "Output values paired element-wise with Input Breakpoints.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "clamp",
            ty: ParamType::Bool,
            label: "Clamp",
            doc: "Clamp to the first/last segment when enabled; otherwise extrapolate beyond the breakpoint range.",
            default_json: Some(serde_json::json!({ "bool": false })),
            min: None,
            max: None,
        }],
    });

    // 3D-specific utility kept
    nodes.push(NodeSignature {
        type_id: Vec3Cross,
        name: "Vec3 Cross",
        category: "Vectors",
        doc: "Computes the cross product A × B; mismatched shapes yield NaN components.",
        inputs: vec![
            PortSpec {
                id: "a",
                ty: PortType::Vec3,
                label: "A",
                doc: "First 3D vector operand.",
                optional: false,
            },
            PortSpec {
                id: "b",
                ty: PortType::Vec3,
                label: "B",
                doc: "Second 3D vector operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_vec3()],
        variadic_outputs: None,
        params: vec![],
    });

    // Generic vector utilities
    nodes.push(NodeSignature {
        type_id: VectorConstant,
        name: "Vector Constant",
        category: "Vectors",
        doc: "Outputs the configured vector value each frame.",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_vector()],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "value",
            ty: ParamType::Any,
            label: "Value",
            doc: "Vector or numeric value to emit.",
            default_json: Some(serde_json::json!({ "vector": [] })),
            min: None,
            max: None,
        }],
    });

    for (nt, name, doc) in [
        (
            VectorAdd,
            "Vector Add",
            "Element-wise sum of A and B; mismatched shapes produce NaN.",
        ),
        (
            VectorSubtract,
            "Vector Subtract",
            "Element-wise subtraction A - B; mismatched shapes produce NaN.",
        ),
        (
            VectorMultiply,
            "Vector Multiply",
            "Element-wise product of A and B; mismatched shapes produce NaN.",
        ),
    ] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
            category: "Vectors",
            doc,
            inputs: vec![
                PortSpec {
                    id: "a",
                    ty: PortType::Vector,
                    label: "A",
                    doc: "First vector operand.",
                    optional: false,
                },
                PortSpec {
                    id: "b",
                    ty: PortType::Vector,
                    label: "B",
                    doc: "Second vector operand.",
                    optional: false,
                },
            ],
            variadic_inputs: None,
            outputs: vec![p_out_vector()],
            variadic_outputs: None,
            params: vec![],
        });
    }

    nodes.push(NodeSignature {
        type_id: VectorScale,
        name: "Vector Scale",
        category: "Vectors",
        doc: "Multiplies Vector by Scalar; scalar broadcasts across components.",
        inputs: vec![
            PortSpec {
                id: "scalar",
                ty: PortType::Float,
                label: "Scalar",
                doc: "Scalar multiplier.",
                optional: false,
            },
            PortSpec {
                id: "v",
                ty: PortType::Vector,
                label: "Vector",
                doc: "Vector to scale.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_vector()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: VectorNormalize,
        name: "Vector Normalize",
        category: "Vectors",
        doc: "Normalizes the input vector to unit length; zero-length inputs yield NaN components.",
        inputs: vec![p_vector_in()],
        variadic_inputs: None,
        outputs: vec![p_out_vector()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: VectorDot,
        name: "Vector Dot",
        category: "Vectors",
        doc: "Computes the dot product of A and B; mismatched shapes yield NaN.",
        inputs: vec![
            PortSpec {
                id: "a",
                ty: PortType::Vector,
                label: "A",
                doc: "First vector operand.",
                optional: false,
            },
            PortSpec {
                id: "b",
                ty: PortType::Vector,
                label: "B",
                doc: "Second vector operand.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: VectorLength,
        name: "Vector Length",
        category: "Vectors",
        doc: "Computes the Euclidean length of the input vector; non-numeric inputs yield NaN.",
        inputs: vec![p_vector_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: VectorIndex,
        name: "Vector Index",
        category: "Vectors",
        doc: "Extracts the element at floor(Index); out-of-range indices yield NaN.",
        inputs: vec![
            PortSpec {
                id: "v",
                ty: PortType::Vector,
                label: "Vector",
                doc: "Vector to sample from.",
                optional: false,
            },
            PortSpec {
                id: "index",
                ty: PortType::Float,
                label: "Index",
                doc: "0-based index; non-integer values are floored; out-of-range produces NaN.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    // Join (variadic inputs -> single vector)
    nodes.push(NodeSignature {
        type_id: Join,
        name: "Join",
        category: "Vectors",
        doc: "Concatenates all Operand inputs into a single numeric vector, skipping non-numeric entries.",
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Vector,
            label: "Operand",
            doc: "Each vector or scalar slice to append in order.",
            min: 1,
            max: None,
        }),
        outputs: vec![p_out_vector()],
        variadic_outputs: None,
        params: vec![],
    });

    // Split (vector in, sizes param, variadic vector outputs)
    nodes.push(NodeSignature {
        type_id: Split,
        name: "Split",
        category: "Vectors",
        doc: "Splits In into Parts sized by the Sizes param; mismatched totals return NaN-filled segments.",
        inputs: vec![p_vector_in()],
        variadic_inputs: None,
        outputs: vec![],
        variadic_outputs: Some(VariadicSpec {
            id: "parts",
            ty: PortType::Vector,
            label: "Part",
            doc: "Returned segment corresponding to each requested size.",
            min: 1,
            max: None,
        }),
        params: vec![
            ParamSpec {
                id: "sizes",
                ty: ParamType::Vector,
                label: "Sizes",
                doc: "Vector of sizes (floored to integers). Sum must equal input length; otherwise each part is NaNs of requested size.",
                default_json: Some(serde_json::json!({ "vector": [] })),
                min: None,
                max: None,
            }
        ],
    });

    // Reducers: vector -> float
    for (nt, name, doc) in [
        (
            VectorMin,
            "Vector Min",
            "Returns the minimum element of In; empty vectors yield NaN.",
        ),
        (
            VectorMax,
            "Vector Max",
            "Returns the maximum element of In; empty vectors yield NaN.",
        ),
        (
            VectorMean,
            "Vector Mean",
            "Returns the arithmetic mean of In; empty vectors yield NaN.",
        ),
        (
            VectorMedian,
            "Vector Median",
            "Returns the median of In (average of middle pair for even counts); empty vectors yield NaN.",
        ),
        (
            VectorMode,
            "Vector Mode",
            "Returns the most frequent non-NaN value in In; ties choose the smallest value; empty vectors yield NaN.",
        ),
    ] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
            category: "Vectors",
            doc,
            inputs: vec![p_vector_in()],
            variadic_inputs: None,
            outputs: vec![p_out_float()],
            variadic_outputs: None,
            params: vec![],
        });
    }

    // Blend helpers
    nodes.push(NodeSignature {
        type_id: WeightedSumVector,
        name: "Weighted Sum Vector",
        category: "Blend",
        doc: "Pre-computes aggregate blend statistics from Values, optionally applying Weights and Masks; mismatched lengths return NaNs.",
        inputs: vec![
            PortSpec {
                id: "values",
                ty: PortType::Vector,
                label: "Values",
                doc: "Per-input channel values used when computing blend sums.",
                optional: false,
            },
            PortSpec {
                id: "weights",
                ty: PortType::Vector,
                label: "Weights",
                doc: "Per-input weights (single scalar broadcasts to all values).",
                optional: true,
            },
            PortSpec {
                id: "masks",
                ty: PortType::Vector,
                label: "Masks",
                doc: "Optional 0/1 mask enabling contributions; single scalar broadcasts to all values.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![
            PortSpec {
                id: "total_weighted_sum",
                ty: PortType::Float,
                label: "Total Weighted Sum",
                doc: "Σ(value_i × weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "total_weight",
                ty: PortType::Float,
                label: "Total Weight",
                doc: "Σ(weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "max_effective_weight",
                ty: PortType::Float,
                label: "Max Effective Weight",
                doc: "max(weight_i × mask_i); 0.0 when no inputs provided.",
                optional: false,
            },
            PortSpec {
                id: "input_count",
                ty: PortType::Float,
                label: "Input Count",
                doc: "Number of values considered (as Float).",
                optional: false,
            },
        ],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: DefaultBlend,
        name: "Default Blend",
        category: "Blend",
        doc: "Produces a weighted sum of operand inputs plus Baseline and Offset; handles weight broadcasting and falls back to neutral/null when counts mismatch.",
        inputs: vec![
            PortSpec {
                id: "baseline",
                ty: PortType::Any,
                label: "Baseline",
                doc: "Base value scaled by the remaining weight (clamped at zero).",
                optional: true,
            },
            PortSpec {
                id: "offset",
                ty: PortType::Any,
                label: "Offset",
                doc: "Optional offset added after blending.",
                optional: true,
            },
            PortSpec {
                id: "weights",
                ty: PortType::Vector,
                label: "Weights",
                doc: "Optional per-target weights; single value broadcasts to all targets.",
                optional: true,
            },
        ],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Any,
            label: "Operand",
            doc: "Operand values to blend before adding baseline and offset.",
            min: 0,
            max: None,
        }),
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Any,
            label: "Out",
            doc: "Blended value.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: BlendWeightedAverage,
        name: "Blend - Weighted Average",
        category: "Blend",
        doc: "Normalises Total Weighted Sum by Total Weight / Max Effective Weight; falls back when the divisor is invalid.",
        inputs: vec![
            PortSpec {
                id: "total_weighted_sum",
                ty: PortType::Float,
                label: "Total Weighted Sum",
                doc: "Σ(value_i × weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "total_weight",
                ty: PortType::Float,
                label: "Total Weight",
                doc: "Σ(weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "max_effective_weight",
                ty: PortType::Float,
                label: "Max Effective Weight",
                doc: "max(weight_i × mask_i); used to normalise the divisor.",
                optional: false,
            },
            PortSpec {
                id: "fallback",
                ty: PortType::Float,
                label: "Fallback",
                doc: "Used when total_weight is zero or the average cannot be computed.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: BlendAdditive,
        name: "Blend - Additive",
        category: "Blend",
        doc: "Outputs Total Weighted Sum when any inputs contribute; otherwise emits Fallback or NaN.",
        inputs: vec![
            PortSpec {
                id: "total_weighted_sum",
                ty: PortType::Float,
                label: "Total Weighted Sum",
                doc: "Σ(value_i × weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "total_weight",
                ty: PortType::Float,
                label: "Total Weight",
                doc: "Σ(weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "fallback",
                ty: PortType::Float,
                label: "Fallback",
                doc: "Fallback value when no inputs contribute.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: BlendMultiply,
        name: "Blend - Multiply",
        category: "Blend",
        doc: "Multiplies contributions using (1 - weight) + value × weight × mask for each entry; mismatched lengths yield NaN, empty input returns 1.",
        inputs: vec![
            PortSpec {
                id: "values",
                ty: PortType::Vector,
                label: "Values",
                doc: "Per-input channel values.",
                optional: false,
            },
            PortSpec {
                id: "weights",
                ty: PortType::Vector,
                label: "Weights",
                doc: "Per-input weights (single scalar broadcasts to all values).",
                optional: true,
            },
            PortSpec {
                id: "masks",
                ty: PortType::Vector,
                label: "Masks",
                doc: "Optional 0/1 mask enabling contributions; single scalar broadcasts to all values.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Float,
            label: "Out",
            doc: "Product over inputs of (1 - weight_i) + value_i × weight_i × mask_i.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: BlendWeightedOverlay,
        name: "Blend - Weighted Overlay",
        category: "Blend",
        doc: "Interpolates between Base and Total Weighted Sum using Max Effective Weight as the blend factor; invalid weights yield NaN.",
        inputs: vec![
            PortSpec {
                id: "total_weighted_sum",
                ty: PortType::Float,
                label: "Total Weighted Sum",
                doc: "Σ(value_i × weight_i × mask_i).",
                optional: false,
            },
            PortSpec {
                id: "max_effective_weight",
                ty: PortType::Float,
                label: "Max Effective Weight",
                doc: "max(weight_i × mask_i) used as interpolation factor.",
                optional: false,
            },
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "Optional base value to blend from.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: BlendWeightedAverageOverlay,
        name: "Blend - Weighted Average Overlay",
        category: "Blend",
        doc: "Computes an averaged offset and adds it to Base; falls back to Base when averaging fails.",
        inputs: vec![
            PortSpec {
                id: "total_weighted_sum",
                ty: PortType::Float,
                label: "Total Weighted Sum",
                doc: "Σ(delta_i × weight_i × mask_i) where delta is (value - base).",
                optional: false,
            },
            PortSpec {
                id: "total_weight",
                ty: PortType::Float,
                label: "Total Weight",
                doc: "Σ(weight_i × mask_i) for the deltas.",
                optional: false,
            },
            PortSpec {
                id: "max_effective_weight",
                ty: PortType::Float,
                label: "Max Effective Weight",
                doc: "max(weight_i × mask_i) used to normalise the divisor.",
                optional: false,
            },
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "Optional base value the averaged delta is applied to.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: BlendMax,
        name: "Blend - Max",
        category: "Blend",
        doc: "Selects the value whose weight × mask is largest; scales it by that effective weight or falls back to Base when none contribute.",
        inputs: vec![
            PortSpec {
                id: "values",
                ty: PortType::Vector,
                label: "Values",
                doc: "Per-input channel values.",
                optional: false,
            },
            PortSpec {
                id: "weights",
                ty: PortType::Vector,
                label: "Weights",
                doc: "Per-input weights (single scalar broadcasts to all values).",
                optional: true,
            },
            PortSpec {
                id: "masks",
                ty: PortType::Vector,
                label: "Masks",
                doc: "Optional 0/1 mask enabling contributions; single scalar broadcasts to all values.",
                optional: true,
            },
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "Fallback value returned when no effective weight is positive.",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    // Case routing (select by string labels)
    nodes.push(NodeSignature {
        type_id: Case, // reuse If's union output typing; runtime uses NodeType::If/Case mapping. Keep name as 'Case' in types.
        name: "Case",
        category: "Logic",
        doc: "Selects the case value whose label matches Selector; returns Default or NaN when no match is found.",
        inputs: vec![
            PortSpec {
                id: "selector",
                ty: PortType::Any,
                label: "Selector",
                doc: "Value compared against the configured case_labels (exact match).",
                optional: false,
            },
            PortSpec {
                id: "default",
                ty: PortType::Any,
                label: "Default",
                doc: "Value returned when no case label matches the selector.",
                optional: true,
            },
        ],
        variadic_inputs: Some(VariadicSpec {
            id: "operand",
            ty: PortType::Any,
            label: "Case Value",
            doc: "Values routed when their corresponding case_labels entry equals the selector.",
            min: 0,
            max: None,
        }),
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Any,
            label: "Out",
            doc: "Clone of the matched case value or the default when no match is found.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "case_labels",
            ty: ParamType::Any,
            label: "Case Labels",
            doc: "Array of string labels; entry i maps to variadic input operand_i.",
            default_json: Some(serde_json::json!([])),
            min: None,
            max: None,
        }],
    });

    // Robotics
    nodes.push(NodeSignature {
        type_id: InverseKinematics,
        name: "Inverse Kinematics",
        category: "Robotics",
        doc: "Analytic planar 3-bone IK solver; returns joint angles or NaNs when the target is unreachable.",
        inputs: vec![
            PortSpec {
                id: "bone1",
                ty: PortType::Float,
                label: "Bone1",
                doc: "Length of the first bone segment.",
                optional: false,
            },
            PortSpec {
                id: "bone2",
                ty: PortType::Float,
                label: "Bone2",
                doc: "Length of the second bone segment.",
                optional: false,
            },
            PortSpec {
                id: "bone3",
                ty: PortType::Float,
                label: "Bone3",
                doc: "Length of the end-effector segment.",
                optional: false,
            },
            PortSpec {
                id: "theta",
                ty: PortType::Float,
                label: "Theta",
                doc: "Desired end-effector orientation in radians.",
                optional: false,
            },
            PortSpec {
                id: "x",
                ty: PortType::Float,
                label: "Target X",
                doc: "Target X coordinate.",
                optional: false,
            },
            PortSpec {
                id: "y",
                ty: PortType::Float,
                label: "Target Y",
                doc: "Target Y coordinate.",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_vec3()],
        variadic_outputs: None,
        params: vec![],
    });

    #[cfg(feature = "urdf_ik")]
    {
        nodes.push(NodeSignature {
            type_id: UrdfIkPosition,
            name: "URDF IK (Position)",
            category: "Robotics",
            doc: "Solves for joint angles that reach Target Position using the configured URDF chain; errors when URDF data is missing or the target is unreachable.",
            inputs: vec![
                PortSpec {
                    id: "target_pos",
                    ty: PortType::Vec3,
                    label: "Target Position",
                    doc: "World-space XYZ target (meters).",
                    optional: false,
                },
                PortSpec {
                    id: "seed",
                    ty: PortType::Vector,
                    label: "Seed",
                    doc: "Optional joint seed vector.",
                    optional: true,
                },
            ],
            variadic_inputs: None,
            outputs: vec![PortSpec {
                id: "out",
                ty: PortType::Any,
                label: "Joint Angles",
                doc: "Record mapping joint_name → angle radians.",
                optional: false,
            }],
            variadic_outputs: None,
            params: vec![
                ParamSpec {
                    id: "urdf_xml",
                    ty: ParamType::Any,
                    label: "URDF XML",
                    doc: "Robot URDF definition (string).",
                    default_json: Some(serde_json::json!({ "text": "" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "root_link",
                    ty: ParamType::Any,
                    label: "Root Link",
                    doc: "Chain root link name.",
                    default_json: Some(serde_json::json!({ "text": "base_link" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "tip_link",
                    ty: ParamType::Any,
                    label: "Tip Link",
                    doc: "Chain tip link name.",
                    default_json: Some(serde_json::json!({ "text": "tool0" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "weights",
                    ty: ParamType::Vector,
                    label: "Joint Weights",
                    doc: "Optional per-joint weights.",
                    default_json: Some(serde_json::json!({ "vector": [] })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "max_iters",
                    ty: ParamType::Float,
                    label: "Max Iterations",
                    doc: "Solver iteration cap.",
                    default_json: Some(serde_json::json!({ "float": 100.0 })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "tol_pos",
                    ty: ParamType::Float,
                    label: "Position Tolerance",
                    doc: "Solver position tolerance (m).",
                    default_json: Some(serde_json::json!({ "float": 0.001 })),
                    min: None,
                    max: None,
                },
            ],
        });

        nodes.push(NodeSignature {
            type_id: UrdfIkPose,
            name: "URDF IK (Pose)",
            category: "Robotics",
            doc: "Solves for joint angles matching both Target Position and Target Rotation; errors when the pose is unreachable or input shapes are invalid.",
            inputs: vec![
                PortSpec {
                    id: "target_pos",
                    ty: PortType::Vec3,
                    label: "Target Position",
                    doc: "World-space XYZ target (meters).",
                    optional: false,
                },
                PortSpec {
                    id: "target_rot",
                    ty: PortType::Vector,
                    label: "Target Rotation",
                    doc: "Target quaternion (x, y, z, w).",
                    optional: false,
                },
                PortSpec {
                    id: "seed",
                    ty: PortType::Vector,
                    label: "Seed",
                    doc: "Optional joint seed vector.",
                    optional: true,
                },
            ],
            variadic_inputs: None,
            outputs: vec![PortSpec {
                id: "out",
                ty: PortType::Any,
                label: "Joint Angles",
                doc: "Record mapping joint_name → angle radians.",
                optional: false,
            }],
            variadic_outputs: None,
            params: vec![
                ParamSpec {
                    id: "urdf_xml",
                    ty: ParamType::Any,
                    label: "URDF XML",
                    doc: "Robot URDF definition (string).",
                    default_json: Some(serde_json::json!({ "text": "" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "root_link",
                    ty: ParamType::Any,
                    label: "Root Link",
                    doc: "Chain root link name.",
                    default_json: Some(serde_json::json!({ "text": "base_link" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "tip_link",
                    ty: ParamType::Any,
                    label: "Tip Link",
                    doc: "Chain tip link name.",
                    default_json: Some(serde_json::json!({ "text": "tool0" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "weights",
                    ty: ParamType::Vector,
                    label: "Joint Weights",
                    doc: "Optional per-joint weights.",
                    default_json: Some(serde_json::json!({ "vector": [] })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "max_iters",
                    ty: ParamType::Float,
                    label: "Max Iterations",
                    doc: "Solver iteration cap.",
                    default_json: Some(serde_json::json!({ "float": 100.0 })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "tol_pos",
                    ty: ParamType::Float,
                    label: "Position Tolerance",
                    doc: "Solver position tolerance (m).",
                    default_json: Some(serde_json::json!({ "float": 0.001 })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "tol_rot",
                    ty: ParamType::Float,
                    label: "Rotation Tolerance",
                    doc: "Solver rotation tolerance (rad).",
                    default_json: Some(serde_json::json!({ "float": 0.001 })),
                    min: None,
                    max: None,
                },
            ],
        });

        nodes.push(NodeSignature {
            type_id: UrdfFk,
            name: "URDF FK",
            category: "Robotics",
            doc: "Applies forward kinematics for the configured URDF chain using provided joint values or defaults.",
            inputs: vec![PortSpec {
                id: "joints",
                ty: PortType::Any,
                label: "Joint Values",
                doc: "Record or array of joint angles (radians).",
                optional: false,
            }],
            variadic_inputs: None,
            outputs: vec![
                PortSpec {
                    id: "position",
                    ty: PortType::Vec3,
                    label: "Position",
                    doc: "Tip position in root frame (meters).",
                    optional: false,
                },
                PortSpec {
                    id: "rotation",
                    ty: PortType::Quat,
                    label: "Rotation",
                    doc: "Tip orientation as quaternion (x, y, z, w).",
                    optional: false,
                },
                PortSpec {
                    id: "transform",
                    ty: PortType::Transform,
                    label: "Transform",
                    doc: "Full pose convenience output (pos + rot + unit scale).",
                    optional: true,
                },
            ],
            variadic_outputs: None,
            params: vec![
                ParamSpec {
                    id: "urdf_xml",
                    ty: ParamType::Any,
                    label: "URDF XML",
                    doc: "Robot URDF definition (string).",
                    default_json: Some(serde_json::json!({ "text": "" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "root_link",
                    ty: ParamType::Any,
                    label: "Root Link",
                    doc: "Chain root link name.",
                    default_json: Some(serde_json::json!({ "text": "base_link" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "tip_link",
                    ty: ParamType::Any,
                    label: "Tip Link",
                    doc: "Chain tip link name.",
                    default_json: Some(serde_json::json!({ "text": "tool0" })),
                    min: None,
                    max: None,
                },
                ParamSpec {
                    id: "joint_defaults",
                    ty: ParamType::Any,
                    label: "Joint Defaults",
                    doc: "Fallback joint list [[name, angle], ...] when input misses entries.",
                    default_json: Some(serde_json::json!({ "list": [] })),
                    min: None,
                    max: None,
                },
            ],
        });
    }

    // IO nodes
    nodes.push(NodeSignature {
        type_id: Input,
        name: "Input",
        category: "IO",
        doc: "Reads a staged value from the host path or emits the configured Default; enforces declared output shape when provided.",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Any,
            label: "Out",
            doc: "Staged input value forwarded into the graph.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![
            ParamSpec {
                id: "path",
                ty: ParamType::Any,
                label: "Path",
                doc: "TypedPath string used to stage host inputs (e.g. robot1/Joint.angle).",
                default_json: None,
                min: None,
                max: None,
            },
            ParamSpec {
                id: "value",
                ty: ParamType::Any,
                label: "Default",
                doc: "Optional fallback emitted when no staged input is present.",
                default_json: None,
                min: None,
                max: None,
            },
        ],
    });

    nodes.push(NodeSignature {
        type_id: Output,
        name: "Output",
        category: "IO",
        doc: "Publishes In to the host path while passing the value through for downstream nodes.",
        inputs: vec![PortSpec {
            id: "in",
            ty: PortType::Any,
            label: "In",
            doc: "Value to publish via TypedPath write.",
            optional: false,
        }],
        variadic_inputs: None,
        outputs: vec![PortSpec {
            id: "out",
            ty: PortType::Any,
            label: "Out",
            doc: "Passthrough copy of the input value for chaining.",
            optional: false,
        }],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "path",
            ty: ParamType::Any,
            label: "Path",
            doc: "TypedPath string used when queuing external writes.",
            default_json: None,
            min: None,
            max: None,
        }],
    });

    Registry {
        version: "1.0.0",
        nodes,
    }
}
