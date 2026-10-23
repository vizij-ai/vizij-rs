//! What Vizij's graph format does on the tree, as the functions of the
//! module [`MODULE`]: an `input` node reading its store key and fitting the
//! value to its declared shape, an `output` node passing its value to its
//! sink, and the check of a node's declared output shapes, which the
//! importer places after each node that declares them.
//!
//! [`VizijFunctions::bridge`] answers the calls in-process and passes every
//! other call to the bridge it wraps.

use std::sync::OnceLock;

use arora_behavior::FunctionIndex;
use arora_types::call::{Call, CallBridge, CallError, CallResult, Callable, CallableId};
use arora_types::value::Value;
use hashbrown::HashMap;
use uuid::Uuid;
use vizij_api_core::{Shape, TypedPath};

use crate::eval::vizij;
use crate::functions::{describe, output_field, Module};

/// The module of Vizij's graph-format functions.
pub const MODULE: Module = Module {
    id: Uuid::from_u128(0x7669_7a69_6a67_7261_7068_0000_0000_0001),
    name: "@vizij/graph",
};

fn derived(role: &str) -> Uuid {
    Uuid::new_v5(&MODULE.id, role.as_bytes())
}

macro_rules! id {
    ($(#[$doc:meta])* $name:ident, $role:literal) => {
        $(#[$doc])*
        pub fn $name() -> Uuid {
            static ID: OnceLock<Uuid> = OnceLock::new();
            *ID.get_or_init(|| derived($role))
        }
    };
}

id!(
    /// `input(node, in, default, path, shape)`: the value an `input` node
    /// gives, what its store key holds (`in`) or its `default`, fitted to
    /// its declared `shape`.
    input_function,
    "function:input"
);
id!(
    /// `output(node, in, shape)`: the value an `output` node passes to its
    /// sink, checked against its declared `shape`.
    output_function,
    "function:output"
);
id!(
    /// `check(node, in, shapes, ports)`: `in`, the value a node returned,
    /// once each output port `shapes` declares holds a value of its shape.
    check_function,
    "function:check"
);
id!(
    /// The node's id in the graph, which names it in an error.
    node_param,
    "node"
);
id!(
    /// The value a function reads: the store key's, or a node's return.
    in_param,
    "in"
);
id!(
    /// An `input` node's default value.
    default_param,
    "default"
);
id!(
    /// An `input` node's store path, as text.
    path_param,
    "path"
);
id!(
    /// A node's declared shape of its `out` port, as JSON.
    shape_param,
    "shape"
);
id!(
    /// A node's declared output shapes by port, as JSON.
    shapes_param,
    "shapes"
);
id!(
    /// The output ports of a node returning a structure of them, in order.
    ports_param,
    "ports"
);

/// The module's functions.
pub fn function_index() -> &'static FunctionIndex {
    static INDEX: OnceLock<FunctionIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let parameter = |id: Uuid, name: &str| (id, name.to_string(), false);
        [
            (
                input_function(),
                "input",
                vec![
                    parameter(node_param(), "node"),
                    parameter(in_param(), "in"),
                    parameter(default_param(), "default"),
                    parameter(path_param(), "path"),
                    parameter(shape_param(), "shape"),
                ],
            ),
            (
                output_function(),
                "output",
                vec![
                    parameter(node_param(), "node"),
                    parameter(in_param(), "in"),
                    parameter(shape_param(), "shape"),
                ],
            ),
            (
                check_function(),
                "check",
                vec![
                    parameter(node_param(), "node"),
                    parameter(in_param(), "in"),
                    parameter(shapes_param(), "shapes"),
                    parameter(ports_param(), "ports"),
                ],
            ),
        ]
        .into_iter()
        .map(|(id, name, parameters)| (id, describe(MODULE, id, name, parameters)))
        .collect()
    })
}

/// A node's static arguments as last decoded: the text each came as, and
/// what it decodes to.
#[derive(Default)]
struct Statics {
    path: Option<(String, TypedPath)>,
    shape: Option<(String, Shape)>,
    shapes: Option<(String, HashMap<String, Shape>)>,
}

/// The functions of [`MODULE`], run in-process, with what each node's static
/// arguments decode to.
#[derive(Default)]
pub struct VizijFunctions {
    statics: HashMap<String, Statics>,
}

impl VizijFunctions {
    /// A bridge answering this module's calls and passing every other to
    /// `inner`.
    pub fn bridge<'a>(&'a mut self, inner: &'a mut dyn CallBridge) -> Bridge<'a> {
        Bridge {
            functions: self,
            inner,
        }
    }

    /// Forget what is kept for each node `keep` refuses.
    pub(crate) fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.statics.retain(|node, _| keep(node));
    }

    /// Answer a call of `function` with `args`.
    pub fn call_borrowed(
        &mut self,
        function: Uuid,
        args: &[(Uuid, &Value)],
    ) -> Result<CallResult, CallError> {
        self.evaluate(function, args)
            .map(|ret| CallResult {
                ret,
                mutated: Vec::new(),
            })
            .map_err(|message| CallError::Generic { message })
    }

    fn evaluate(&mut self, function: Uuid, args: &[(Uuid, &Value)]) -> Result<Value, String> {
        let arg = |id: Uuid| {
            args.iter()
                .find(|(parameter, _)| *parameter == id)
                .and_then(|(_, value)| present(value))
        };
        let text = |id: Uuid| match arg(id) {
            Some(Value::String(text)) => Some(text.as_str()),
            _ => None,
        };
        let node = text(node_param()).unwrap_or_default();
        let statics = match self.statics.get_mut(node) {
            Some(statics) => statics,
            None => self.statics.entry(node.to_string()).or_default(),
        };
        if function == input_function() {
            let path = decode(&mut statics.path, text(path_param()), |text| {
                TypedPath::parse(text).map_err(|e| e.to_string())
            })?;
            let shape = decode(&mut statics.shape, text(shape_param()), parse_json)?;
            let value = vizij::input(
                node,
                path,
                shape,
                arg(in_param()).cloned(),
                arg(default_param()),
            )?;
            // A value fitted to a shape may still not have it: a shape's
            // `NaN`-filled value is not always of the shape.
            if let Some(shape) = shape {
                vizij::check_shape(node, "out", shape, &value)?;
            }
            Ok(value)
        } else if function == output_function() {
            let shape = decode(&mut statics.shape, text(shape_param()), parse_json)?;
            let value = arg(in_param()).cloned().unwrap_or(Value::F32(0.0));
            if let Some(shape) = shape {
                vizij::check_shape(node, "out", shape, &value)?;
            }
            Ok(value)
        } else if function == check_function() {
            // The node's return as it is: a check passes it on unchanged.
            let value = args
                .iter()
                .find(|(parameter, _)| *parameter == in_param())
                .map_or(Value::Unit, |(_, value)| (*value).clone());
            let Some(shapes) = decode(&mut statics.shapes, text(shapes_param()), parse_json)?
            else {
                return Ok(value);
            };
            match arg(ports_param()) {
                // A node returning a structure of its ports.
                Some(_) => {
                    for (port, shape) in shapes {
                        let field = output_field(port);
                        let held = match &value {
                            Value::Structure(structure) => structure
                                .fields
                                .iter()
                                .find(|f| f.id == field)
                                .map(|f| f.value.as_ref())
                                .filter(|value| **value != Value::Unit),
                            _ => None,
                        };
                        let held = held.ok_or_else(|| {
                            format!(
                                "node '{node}' missing declared output '{port}' during evaluation"
                            )
                        })?;
                        vizij::check_shape(node, port, shape, held)?;
                    }
                }
                // A node returning its `out` port alone.
                None => {
                    for (port, shape) in shapes {
                        if port != "out" {
                            return Err(format!(
                                "node '{node}' missing declared output '{port}' during evaluation"
                            ));
                        }
                        vizij::check_shape(node, port, shape, &value)?;
                    }
                }
            }
            Ok(value)
        } else {
            Err(format!("{} has no function {function}", MODULE.name))
        }
    }
}

/// What `text` decodes to, decoding it again only when it changed since
/// `cached` was decoded.
fn decode<'a, T>(
    cached: &'a mut Option<(String, T)>,
    text: Option<&str>,
    parse: impl Fn(&str) -> Result<T, String>,
) -> Result<Option<&'a T>, String> {
    let Some(text) = text else {
        *cached = None;
        return Ok(None);
    };
    if cached.as_ref().is_none_or(|(known, _)| known != text) {
        *cached = Some((text.to_string(), parse(text)?));
    }
    Ok(cached.as_ref().map(|(_, value)| value))
}

fn parse_json<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|e| format!("a declared shape: {e}"))
}

/// `value`, unless it stands for no argument: unit, or an empty option.
fn present(value: &Value) -> Option<&Value> {
    match value {
        Value::Option(Some(value)) => present(value),
        Value::Option(None) | Value::Unit => None,
        value => Some(value),
    }
}

/// A [`CallBridge`] answering [`MODULE`]'s calls in-process and passing every
/// other call, and the callables, to the bridge it wraps.
pub struct Bridge<'a> {
    functions: &'a mut VizijFunctions,
    inner: &'a mut dyn CallBridge,
}

impl CallBridge for Bridge<'_> {
    fn arora_call(&mut self, call: Call) -> Result<CallResult, CallError> {
        if call.module_id == Some(MODULE.id) {
            let args: Vec<(Uuid, &Value)> = call
                .args
                .iter()
                .map(|field| (field.id, &*field.value))
                .collect();
            self.functions.call_borrowed(call.id, &args)
        } else {
            self.inner.arora_call(call)
        }
    }

    fn arora_call_borrowed(
        &mut self,
        module_id: Option<Uuid>,
        id: Uuid,
        args: &[(Uuid, &Value)],
    ) -> Result<CallResult, CallError> {
        if module_id == Some(MODULE.id) {
            self.functions.call_borrowed(id, args)
        } else {
            self.inner.arora_call_borrowed(module_id, id, args)
        }
    }

    fn arora_register_callable(&mut self, callable: std::rc::Rc<dyn Callable>) -> CallableId {
        self.inner.arora_register_callable(callable)
    }

    fn arora_unregister_callable(&mut self, callable_id: &CallableId) {
        self.inner.arora_unregister_callable(callable_id)
    }

    fn arora_call_indirect(&mut self, callable_id: &CallableId) -> Result<Value, CallError> {
        self.inner.arora_call_indirect(callable_id)
    }
}
