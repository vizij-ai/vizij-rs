use crate::types::NodeType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PortType {
    Float,
    Bool,
    Vec3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamType {
    Float,
    Bool,
    Vec3,
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
    pub params: Vec<ParamSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Registry {
    pub version: &'static str,
    pub nodes: Vec<NodeSignature>,
}

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
fn p_vec_in() -> PortSpec {
    PortSpec {
        id: "in",
        ty: PortType::Vec3,
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
fn p_out_vec() -> PortSpec {
    PortSpec {
        id: "out",
        ty: PortType::Vec3,
        label: "Out",
        doc: "",
        optional: false,
    }
}

pub fn registry() -> Registry {
    use NodeType::*;
    let mut nodes: Vec<NodeSignature> = Vec::new();

    // Scalars / arithmetic
    nodes.push(NodeSignature {
        type_id: Constant,
        name: "Constant",
        category: "Math",
        inputs: vec![],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
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
        params: vec![],
    });

    // Logic
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
        params: vec![],
    });
    nodes.push(NodeSignature {
        type_id: Not,
        name: "Not",
        category: "Logic",
        inputs: vec![p_bool_in()],
        variadic_inputs: None,
        outputs: vec![p_out_bool()],
        params: vec![],
    });

    // Conditional
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
        params: vec![],
    });
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
            // then/else allow union; document as Any
            PortSpec {
                id: "then",
                ty: PortType::Vec3,
                label: "Then",
                doc: "Value (float/bool/vec3)",
                optional: true,
            },
            PortSpec {
                id: "else",
                ty: PortType::Vec3,
                label: "Else",
                doc: "Value (float/bool/vec3)",
                optional: true,
            },
        ],
        variadic_inputs: None,
        outputs: vec![
            // Union in core; encode as vec3 in schema for display; UI should treat as Any value
            PortSpec {
                id: "out",
                ty: PortType::Vec3,
                label: "Out",
                doc: "Value (float/bool/vec3)",
                optional: false,
            },
        ],
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
        params: vec![],
    });

    // Vec3 utilities
    nodes.push(NodeSignature {
        type_id: Vec3,
        name: "Vec3",
        category: "Vectors",
        inputs: vec![
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
        variadic_inputs: None,
        outputs: vec![p_out_vec()],
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Vec3Split,
        name: "Vec3 Split",
        category: "Vectors",
        inputs: vec![p_vec_in()],
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
        params: vec![],
    });

    for (nt, name) in [
        (Vec3Add, "Vec3 Add"),
        (Vec3Subtract, "Vec3 Subtract"),
        (Vec3Multiply, "Vec3 Multiply"),
    ] {
        nodes.push(NodeSignature {
            type_id: nt,
            name,
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
            outputs: vec![p_out_vec()],
            params: vec![],
        });
    }

    nodes.push(NodeSignature {
        type_id: Vec3Scale,
        name: "Vec3 Scale",
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
                ty: PortType::Vec3,
                label: "Vector",
                doc: "",
                optional: false,
            },
        ],
        variadic_inputs: None,
        outputs: vec![p_out_vec()],
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Vec3Normalize,
        name: "Vec3 Normalize",
        category: "Vectors",
        inputs: vec![p_vec_in()],
        variadic_inputs: None,
        outputs: vec![p_out_vec()],
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Vec3Dot,
        name: "Vec3 Dot",
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
        outputs: vec![p_out_float()],
        params: vec![],
    });

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
        outputs: vec![p_out_vec()],
        params: vec![],
    });

    nodes.push(NodeSignature {
        type_id: Vec3Length,
        name: "Vec3 Length",
        category: "Vectors",
        inputs: vec![p_vec_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        params: vec![],
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
        outputs: vec![p_out_vec()],
        params: vec![],
    });

    // Sinks
    nodes.push(NodeSignature {
        type_id: Output,
        name: "Output",
        category: "Sinks",
        inputs: vec![p_in()],
        variadic_inputs: None,
        outputs: vec![p_out_float()],
        params: vec![],
    });

    Registry {
        version: "1.0.0",
        nodes,
    }
}
