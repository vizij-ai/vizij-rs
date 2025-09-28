use crate::types::NodeType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortType {
    Float,
    Bool,
    Vec3,
    Quat,
    Transform,
    Vector,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    Float,
    Bool,
    Vec3,
    Vector,
    Any, // union (Value)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortSpec {
    pub id: &'static str,
    pub ty: PortType,
    pub label: &'static str,
    #[serde(default)]
    pub doc: &'static str,
    #[serde(default)]
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariadicSpec {
    pub id: &'static str,
    pub ty: PortType,
    pub label: &'static str,
    #[serde(default)]
    pub doc: &'static str,
    pub min: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSpec {
    pub id: &'static str,
    pub ty: ParamType,
    pub label: &'static str,
    #[serde(default)]
    pub doc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_json: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSignature {
    pub type_id: NodeType,
    pub name: &'static str,
    pub category: &'static str,
    pub inputs: Vec<PortSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variadic_inputs: Option<VariadicSpec>,
    pub outputs: Vec<PortSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variadic_outputs: Option<VariadicSpec>,
    pub params: Vec<ParamSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: &'static str,
    pub nodes: Vec<NodeSignature>,
}

// Helpers
fn p_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Float,
        label: "In",
        doc: "",
        optional: false,
    }
}
fn p_bool_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Bool,
        label: "In",
        doc: "",
        optional: false,
    }
}
fn p_vector_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Vector,
        label: "In",
        doc: "",
        optional: false,
    }
}
fn p_out_float() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Float,
        label: "Out",
        doc: "",
        optional: false,
    }
}
fn p_out_bool() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Bool,
        label: "Out",
        doc: "",
        optional: false,
    }
}
fn p_out_vec3() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Vec3,
        label: "Out",
        doc: "",
        optional: false,
    }
}
fn p_out_vector() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Vector,
        label: "Out",
        doc: "",
        optional: false,
    }
}

pub fn registry() -> Registry {
    use NodeType::*;
    let mut nodes: Vec<NodeSignature> = Vec::new();

    // Scalars / arithmetic (float-based legacy kept for convenience)
    nodes.push(NodeSignature {
        type_id: Constant,
        name: "Constant",
        category: "Math",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "value",
            ty: ParamType::Any,
            label: "Value",
            doc: "",
            default_json: Some(serde_json::json!({ "float": 0.0 })),
            min: None,
            max: None,
        }],
    });

    nodes.push(NodeSignature {
        type_id: Slider,
        name: "Slider",
        category: "Math",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![
            ParamSpec {
                id: "value",
                ty: ParamType::Float,
                label: "Value",
                doc: "",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "min",
                ty: ParamType::Float,
                label: "Min",
                doc: "",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "max",
                ty: ParamType::Float,
                label: "Max",
                doc: "",
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
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![
            PortSpec {
                id: "x",
                ty: PortType::Float,
                label: "X",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "y",
                ty: PortType::Float,
                label: "Y",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "z",
                ty: PortType::Float,
                label: "Z",
                doc: "",
                optional: false,
            },
        ],
        variadic_outputs: None,
        params: vec![
            ParamSpec {
                id: "x",
                ty: ParamType::Float,
                label: "X",
                doc: "",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "y",
                ty: ParamType::Float,
                label: "Y",
                doc: "",
                default_json: Some(serde_json::json!({ "float": 0.0 })),
                min: None,
                max: None,
            },
            ParamSpec {
                id: "z",
                ty: ParamType::Float,
                label: "Z",
                doc: "",
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
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operands",
            ty: PortType::Float,
            label: "Operand",
            doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "",
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
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operands",
            ty: PortType::Float,
            label: "Operand",
            doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "exp",
                ty: PortType::Float,
                label: "Exponent",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "value",
                ty: PortType::Float,
                label: "Value",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "base",
                ty: PortType::Float,
                label: "Base",
                doc: "",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    for (nt, name) in [(Sin, "Sin"), (Cos, "Cos"), (Tan, "Tan")] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
            category: "Math",
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
        inputs: vec![
            PortSpec {
                id: "frequency",
                ty: PortType::Float,
                label: "Frequency",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "phase",
                ty: PortType::Float,
                label: "Phase",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Bool,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Bool,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Bool,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Bool,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Bool,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Bool,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "lhs",
                ty: PortType::Float,
                label: "LHS",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "rhs",
                ty: PortType::Float,
                label: "RHS",
                doc: "",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        variadic_outputs: None,
        params: vec![],
    });

    // If (union in core; schema uses Vector as generic placeholder)
    nodes.push(NodeSignature {
        type_id: If,
        name: "If",
        category: "Logic",
        inputs: vec![
            PortSpec {
                id: "cond",
                ty: PortType::Bool,
                label: "Condition",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "then",
                ty: PortType::Vector,
                label: "Then",
                doc: "Value",
                optional: true,
            },
            PortSpec {
                id: "else",
                ty: PortType::Vector,
                label: "Else",
                doc: "Value",
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
        inputs: vec![
            PortSpec {
                id: "in",
                ty: PortType::Float,
                label: "In",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "min",
                ty: PortType::Float,
                label: "Min",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "max",
                ty: PortType::Float,
                label: "Max",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "in",
                ty: PortType::Float,
                label: "In",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "in_min",
                ty: PortType::Float,
                label: "In Min",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "in_max",
                ty: PortType::Float,
                label: "In Max",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "out_min",
                ty: PortType::Float,
                label: "Out Min",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "out_max",
                ty: PortType::Float,
                label: "Out Max",
                doc: "",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        variadic_outputs: None,
        params: vec![],
    });

    // 3D-specific utility kept
    nodes.push(NodeSignature {
        type_id: Vec3Cross,
        name: "Vec3 Cross",
        category: "Vectors",
        inputs: vec![
            PortSpec {
                id: "a",
                ty: PortType::Vec3,
                label: "A",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "b",
                ty: PortType::Vec3,
                label: "B",
                doc: "",
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
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_vector()],
        variadic_outputs: None,
        params: vec![ParamSpec {
            id: "value",
            ty: ParamType::Any,
            label: "Value",
            doc: "",
            default_json: Some(serde_json::json!({ "vector": [] })),
            min: None,
            max: None,
        }],
    });

    for (nt, name) in [
        (VectorAdd, "Vector Add"),
        (VectorSubtract, "Vector Subtract"),
        (VectorMultiply, "Vector Multiply"),
    ] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
            category: "Vectors",
            inputs: vec![
                PortSpec {
                    id: "a",
                    ty: PortType::Vector,
                    label: "A",
                    doc: "",
                    optional: false,
                },
                PortSpec {
                    id: "b",
                    ty: PortType::Vector,
                    label: "B",
                    doc: "",
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
        inputs: vec![
            PortSpec {
                id: "scalar",
                ty: PortType::Float,
                label: "Scalar",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "v",
                ty: PortType::Vector,
                label: "Vector",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "a",
                ty: PortType::Vector,
                label: "A",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "b",
                ty: PortType::Vector,
                label: "B",
                doc: "",
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
        inputs: vec![
            PortSpec {
                id: "v",
                ty: PortType::Vector,
                label: "Vector",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "index",
                ty: PortType::Float,
                label: "Index",
                doc: "0-based index; non-integer values are floored",
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
        inputs: vec![],
        variadic_inputs: Some(VariadicSpec {
            id: "operands",
            ty: PortType::Vector,
            label: "Operand",
            doc: "",
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
        inputs: vec![p_vector_in()],
        variadic_inputs: None,
        outputs: vec![],
        variadic_outputs: Some(VariadicSpec {
            id: "parts",
            ty: PortType::Vector,
            label: "Part",
            doc: "",
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
    for (nt, name) in [
        (VectorMin, "Vector Min"),
        (VectorMax, "Vector Max"),
        (VectorMean, "Vector Mean"),
        (VectorMedian, "Vector Median"),
        (VectorMode, "Vector Mode"),
    ] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
            category: "Vectors",
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
        type_id: BlendWeightedAverage,
        name: "Blend - Weighted Average",
        category: "Blend",
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
            id: "cases",
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
            doc: "Array of string labels; entry i maps to variadic input cases_i.",
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
        inputs: vec![
            PortSpec {
                id: "bone1",
                ty: PortType::Float,
                label: "Bone1",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "bone2",
                ty: PortType::Float,
                label: "Bone2",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "bone3",
                ty: PortType::Float,
                label: "Bone3",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "theta",
                ty: PortType::Float,
                label: "Theta",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "x",
                ty: PortType::Float,
                label: "Target X",
                doc: "",
                optional: false,
            },
            PortSpec {
                id: "y",
                ty: PortType::Float,
                label: "Target Y",
                doc: "",
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
