// Adapted to use vizij_api_core::Value (f32-based) and f32 arithmetic.

use crate::types::{GraphSpec, InputConnection, NodeId, NodeSpec, NodeType};
use hashbrown::HashMap;
use vizij_api_core::shape::Field;
use vizij_api_core::{coercion, Shape, ShapeId, Value, WriteBatch, WriteOp};

#[derive(Clone, Debug, PartialEq)]
enum ValueLayout {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Quat,
    ColorRgba,
    Transform,
    Vector(usize),
    Record(Vec<(String, ValueLayout)>),
    Array(Vec<ValueLayout>),
    List(Vec<ValueLayout>),
    Tuple(Vec<ValueLayout>),
}

#[derive(Clone, Debug)]
struct FlatValue {
    layout: ValueLayout,
    data: Vec<f32>,
}

impl ValueLayout {
    fn scalar_len(&self) -> usize {
        match self {
            ValueLayout::Scalar => 1,
            ValueLayout::Vec2 => 2,
            ValueLayout::Vec3 => 3,
            ValueLayout::Vec4 => 4,
            ValueLayout::Quat => 4,
            ValueLayout::ColorRgba => 4,
            ValueLayout::Transform => 10,
            ValueLayout::Vector(len) => *len,
            ValueLayout::Record(fields) => {
                fields.iter().map(|(_, layout)| layout.scalar_len()).sum()
            }
            ValueLayout::Array(items) | ValueLayout::List(items) | ValueLayout::Tuple(items) => {
                items.iter().map(|layout| layout.scalar_len()).sum()
            }
        }
    }

    fn is_scalar(&self) -> bool {
        matches!(self, ValueLayout::Scalar)
    }

    fn reconstruct(&self, data: &[f32]) -> Value {
        match self {
            ValueLayout::Scalar => Value::Float(data.first().copied().unwrap_or(f32::NAN)),
            ValueLayout::Vec2 => {
                let mut arr = [0.0; 2];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec2(arr)
            }
            ValueLayout::Vec3 => {
                let mut arr = [0.0; 3];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec3(arr)
            }
            ValueLayout::Vec4 => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Vec4(arr)
            }
            ValueLayout::Quat => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::Quat(arr)
            }
            ValueLayout::ColorRgba => {
                let mut arr = [0.0; 4];
                for (i, slot) in arr.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                Value::ColorRgba(arr)
            }
            ValueLayout::Transform => {
                let mut pos = [0.0; 3];
                let mut rot = [0.0; 4];
                let mut scale = [0.0; 3];
                for (i, slot) in pos.iter_mut().enumerate() {
                    *slot = *data.get(i).unwrap_or(&f32::NAN);
                }
                for (i, slot) in rot.iter_mut().enumerate() {
                    *slot = *data.get(3 + i).unwrap_or(&f32::NAN);
                }
                for (i, slot) in scale.iter_mut().enumerate() {
                    *slot = *data.get(7 + i).unwrap_or(&f32::NAN);
                }
                Value::Transform { pos, rot, scale }
            }
            ValueLayout::Vector(len) => {
                let mut out = Vec::with_capacity(*len);
                out.extend((0..*len).map(|i| *data.get(i).unwrap_or(&f32::NAN)));
                Value::Vector(out)
            }
            ValueLayout::Record(fields) => {
                let mut map = HashMap::new();
                let mut offset = 0usize;
                for (key, layout) in fields.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    map.insert(key.clone(), layout.reconstruct(slice));
                }
                Value::Record(map)
            }
            ValueLayout::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::Array(out)
            }
            ValueLayout::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::List(out)
            }
            ValueLayout::Tuple(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut offset = 0usize;
                for layout in items.iter() {
                    let len = layout.scalar_len();
                    let slice = &data[offset..offset + len];
                    offset += len;
                    out.push(layout.reconstruct(slice));
                }
                Value::Tuple(out)
            }
        }
    }

    fn fill_with(&self, value: f32) -> Value {
        let len = self.scalar_len();
        let data = vec![value; len];
        self.reconstruct(&data)
    }
}

fn flatten_numeric(value: &Value) -> Option<FlatValue> {
    match value {
        Value::Float(f) => Some(FlatValue {
            layout: ValueLayout::Scalar,
            data: vec![*f],
        }),
        Value::Vec2(a) => Some(FlatValue {
            layout: ValueLayout::Vec2,
            data: a.to_vec(),
        }),
        Value::Vec3(a) => Some(FlatValue {
            layout: ValueLayout::Vec3,
            data: a.to_vec(),
        }),
        Value::Vec4(a) => Some(FlatValue {
            layout: ValueLayout::Vec4,
            data: a.to_vec(),
        }),
        Value::Quat(a) => Some(FlatValue {
            layout: ValueLayout::Quat,
            data: a.to_vec(),
        }),
        Value::ColorRgba(a) => Some(FlatValue {
            layout: ValueLayout::ColorRgba,
            data: a.to_vec(),
        }),
        Value::Transform { pos, rot, scale } => {
            let mut data = Vec::with_capacity(10);
            data.extend_from_slice(pos);
            data.extend_from_slice(rot);
            data.extend_from_slice(scale);
            Some(FlatValue {
                layout: ValueLayout::Transform,
                data,
            })
        }
        Value::Vector(vec) => Some(FlatValue {
            layout: ValueLayout::Vector(vec.len()),
            data: vec.clone(),
        }),
        Value::Record(map) => {
            let mut fields: Vec<_> = map.iter().collect();
            fields.sort_by(|a, b| a.0.cmp(b.0));
            let mut layouts = Vec::with_capacity(fields.len());
            let mut data = Vec::new();
            for (key, val) in fields {
                let flat = flatten_numeric(val)?;
                data.extend(&flat.data);
                layouts.push((key.clone(), flat.layout));
            }
            Some(FlatValue {
                layout: ValueLayout::Record(layouts),
                data,
            })
        }
        Value::Array(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::Array(layouts),
                data,
            })
        }
        Value::List(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::List(layouts),
                data,
            })
        }
        Value::Tuple(items) => {
            let mut layouts = Vec::with_capacity(items.len());
            let mut data = Vec::new();
            for item in items.iter() {
                let flat = flatten_numeric(item)?;
                data.extend(&flat.data);
                layouts.push(flat.layout);
            }
            Some(FlatValue {
                layout: ValueLayout::Tuple(layouts),
                data,
            })
        }
        _ => None,
    }
}

fn align_flattened(
    a: &FlatValue,
    b: &FlatValue,
) -> Result<(ValueLayout, Vec<f32>, Vec<f32>), ValueLayout> {
    if a.layout == b.layout {
        return Ok((a.layout.clone(), a.data.clone(), b.data.clone()));
    }
    if a.layout.is_scalar() {
        let layout = b.layout.clone();
        let len = layout.scalar_len();
        let repeated = vec![a.data.first().copied().unwrap_or(f32::NAN); len];
        return Ok((layout, repeated, b.data.clone()));
    }
    if b.layout.is_scalar() {
        let layout = a.layout.clone();
        let len = layout.scalar_len();
        let repeated = vec![b.data.first().copied().unwrap_or(f32::NAN); len];
        return Ok((layout, a.data.clone(), repeated));
    }

    let len_a = a.layout.scalar_len();
    let len_b = b.layout.scalar_len();
    if len_a >= len_b {
        Err(a.layout.clone())
    } else {
        Err(b.layout.clone())
    }
}

fn binary_numeric<F>(lhs: &Value, rhs: &Value, op: F) -> Value
where
    F: Fn(f32, f32) -> f32 + Copy,
{
    match (flatten_numeric(lhs), flatten_numeric(rhs)) {
        (Some(a), Some(b)) => match align_flattened(&a, &b) {
            Ok((layout, da, db)) => {
                let data: Vec<f32> = da.iter().zip(db.iter()).map(|(x, y)| op(*x, *y)).collect();
                layout.reconstruct(&data)
            }
            Err(layout) => layout.fill_with(f32::NAN),
        },
        (Some(a), None) => a.layout.fill_with(f32::NAN),
        (None, Some(b)) => b.layout.fill_with(f32::NAN),
        (None, None) => Value::Float(f32::NAN),
    }
}

fn unary_numeric<F>(input: &Value, op: F) -> Value
where
    F: Fn(f32) -> f32 + Copy,
{
    match flatten_numeric(input) {
        Some(flat) => {
            let data: Vec<f32> = flat.data.iter().map(|x| op(*x)).collect();
            flat.layout.reconstruct(&data)
        }
        None => Value::Float(f32::NAN),
    }
}

fn fold_numeric_variadic<F>(values: &[Value], op: F, empty_fallback: Value) -> Value
where
    F: Fn(f32, f32) -> f32 + Copy,
{
    if values.is_empty() {
        return empty_fallback;
    }
    let mut iter = values.iter();
    let mut acc = iter
        .next()
        .cloned()
        .unwrap_or_else(|| Value::Float(f32::NAN));
    for v in iter {
        acc = binary_numeric(&acc, v, op);
    }
    acc
}

#[derive(Clone, Debug)]
pub struct PortValue {
    pub value: Value,
    pub shape: Shape,
}

impl PortValue {
    pub fn new(value: Value) -> Self {
        let shape = infer_shape(&value);
        PortValue { value, shape }
    }
}

impl Default for PortValue {
    fn default() -> Self {
        PortValue::new(Value::Float(0.0))
    }
}

fn infer_shape(value: &Value) -> Shape {
    Shape::new(infer_shape_id(value))
}

fn infer_shape_id(value: &Value) -> ShapeId {
    match value {
        Value::Float(_) => ShapeId::Scalar,
        Value::Bool(_) => ShapeId::Bool,
        Value::Vec2(_) => ShapeId::Vec2,
        Value::Vec3(_) => ShapeId::Vec3,
        Value::Vec4(_) => ShapeId::Vec4,
        Value::Quat(_) => ShapeId::Quat,
        Value::ColorRgba(_) => ShapeId::ColorRgba,
        Value::Transform { .. } => ShapeId::Transform,
        Value::Vector(vec) => ShapeId::Vector {
            len: if vec.is_empty() {
                None
            } else {
                Some(vec.len())
            },
        },
        Value::Text(_) => ShapeId::Text,
        Value::Enum(tag, boxed) => ShapeId::Enum(vec![(tag.clone(), infer_shape_id(boxed))]),
        Value::Record(map) => {
            let mut fields: Vec<Field> = map
                .iter()
                .map(|(name, value)| Field {
                    name: name.clone(),
                    shape: infer_shape_id(value),
                })
                .collect();
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            ShapeId::Record(fields)
        }
        Value::Array(items) => {
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                let inner = if consistent {
                    first_shape
                } else {
                    ShapeId::Scalar
                };
                ShapeId::Array(Box::new(inner), items.len())
            } else {
                ShapeId::Array(Box::new(ShapeId::Scalar), 0)
            }
        }
        Value::List(items) => {
            if let Some(first) = items.first() {
                let first_shape = infer_shape_id(first);
                let consistent = items.iter().all(|item| infer_shape_id(item) == first_shape);
                let inner = if consistent {
                    first_shape
                } else {
                    ShapeId::Scalar
                };
                ShapeId::List(Box::new(inner))
            } else {
                ShapeId::List(Box::new(ShapeId::Scalar))
            }
        }
        Value::Tuple(items) => {
            let shapes = items.iter().map(infer_shape_id).collect();
            ShapeId::Tuple(shapes)
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct GraphRuntime {
    pub t: f32,
    pub outputs: HashMap<NodeId, HashMap<String, PortValue>>,
    pub writes: WriteBatch,
}

fn as_float(v: &Value) -> f32 {
    coercion::to_float(v)
}

fn as_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Text(s) => !s.is_empty(),
        _ => coercion::to_vector(v).iter().any(|x| *x != 0.0),
    }
}

fn read_inputs(
    rt: &GraphRuntime,
    inputs: &HashMap<String, InputConnection>,
) -> HashMap<String, PortValue> {
    inputs
        .iter()
        .map(|(input_key, conn)| {
            let val = rt
                .outputs
                .get(&conn.node_id)
                .and_then(|outputs| outputs.get(&conn.output_key))
                .cloned()
                .unwrap_or_else(|| PortValue::new(Value::Float(0.0)));
            (input_key.clone(), val)
        })
        .collect()
}

macro_rules! out_map {
    ($key:expr, $val:expr) => {{
        let mut map = HashMap::new();
        map.insert($key.to_string(), PortValue::new($val));
        map
    }};
    ($val:expr) => {
        out_map!("out", $val)
    };
}

pub fn eval_node(rt: &mut GraphRuntime, spec: &NodeSpec) -> Result<(), String> {
    let ivals = read_inputs(rt, &spec.inputs);
    let t = rt.t;
    let p = &spec.params;

    let get_input = |key: &str| {
        ivals
            .get(key)
            .cloned()
            .unwrap_or_else(|| PortValue::new(Value::Float(0.0)))
    };

    let mut outputs = match spec.kind {
        NodeType::Constant => out_map!(p.value.clone().unwrap_or(Value::Float(0.0))),
        NodeType::Slider => out_map!(Value::Float(p.value.as_ref().map(as_float).unwrap_or(0.0))),
        NodeType::MultiSlider => {
            let mut map = HashMap::new();
            let x = p.x.unwrap_or(0.0);
            let y = p.y.unwrap_or(0.0);
            let z = p.z.unwrap_or(0.0);
            map.insert("x".to_string(), PortValue::new(Value::Float(x)));
            map.insert("y".to_string(), PortValue::new(Value::Float(y)));
            map.insert("z".to_string(), PortValue::new(Value::Float(z)));
            map
        }
        NodeType::Add => {
            let inputs: Vec<Value> = ivals.values().map(|pv| pv.value.clone()).collect();
            let result = fold_numeric_variadic(&inputs, |x, y| x + y, Value::Float(0.0));
            out_map!(result)
        }
        NodeType::Subtract => {
            let lhs = get_input("lhs");
            let rhs = get_input("rhs");
            out_map!(binary_numeric(&lhs.value, &rhs.value, |x, y| x - y))
        }
        NodeType::Multiply => {
            let inputs: Vec<Value> = ivals.values().map(|pv| pv.value.clone()).collect();
            let result = fold_numeric_variadic(&inputs, |x, y| x * y, Value::Float(1.0));
            out_map!(result)
        }
        NodeType::Divide => {
            let lhs = get_input("lhs");
            let rhs = get_input("rhs");
            out_map!(binary_numeric(&lhs.value, &rhs.value, |x, y| if y != 0.0 {
                x / y
            } else {
                f32::NAN
            }))
        }
        NodeType::Power => {
            let base = get_input("base");
            let exp = get_input("exp");
            out_map!(binary_numeric(&base.value, &exp.value, |x, y| x.powf(y)))
        }
        NodeType::Log => {
            let val = get_input("value");
            let base = get_input("base");
            out_map!(binary_numeric(&val.value, &base.value, |x, b| x.log(b)))
        }
        NodeType::Sin => {
            let v = get_input("in");
            out_map!(unary_numeric(&v.value, |x| x.sin()))
        }
        NodeType::Cos => {
            let v = get_input("in");
            out_map!(unary_numeric(&v.value, |x| x.cos()))
        }
        NodeType::Tan => {
            let v = get_input("in");
            out_map!(unary_numeric(&v.value, |x| x.tan()))
        }

        NodeType::Time => out_map!(Value::Float(t)),
        NodeType::Oscillator => {
            let f = as_float(&get_input("frequency").value);
            let phase = as_float(&get_input("phase").value);
            out_map!(Value::Float((std::f32::consts::TAU * f * t + phase).sin()))
        }

        NodeType::And => out_map!(Value::Bool(
            as_bool(&get_input("lhs").value) && as_bool(&get_input("rhs").value)
        )),
        NodeType::Or => out_map!(Value::Bool(
            as_bool(&get_input("lhs").value) || as_bool(&get_input("rhs").value)
        )),
        NodeType::Not => out_map!(Value::Bool(!as_bool(&get_input("in").value))),
        NodeType::Xor => out_map!(Value::Bool(
            as_bool(&get_input("lhs").value) ^ as_bool(&get_input("rhs").value)
        )),

        NodeType::GreaterThan => out_map!(Value::Bool(
            as_float(&get_input("lhs").value) > as_float(&get_input("rhs").value)
        )),
        NodeType::LessThan => out_map!(Value::Bool(
            as_float(&get_input("lhs").value) < as_float(&get_input("rhs").value)
        )),
        NodeType::Equal => out_map!(Value::Bool(
            (as_float(&get_input("lhs").value) - as_float(&get_input("rhs").value)).abs() < 1e-6
        )),
        NodeType::NotEqual => out_map!(Value::Bool(
            (as_float(&get_input("lhs").value) - as_float(&get_input("rhs").value)).abs() > 1e-6
        )),
        NodeType::If => {
            let cond = as_bool(&get_input("cond").value);
            out_map!(if cond {
                get_input("then").value
            } else {
                get_input("else").value
            })
        }

        NodeType::Clamp => {
            let input = get_input("in");
            let min = get_input("min");
            let max = get_input("max");
            let clamped_low = binary_numeric(&input.value, &min.value, |x, m| x.max(m));
            out_map!(binary_numeric(&clamped_low, &max.value, |x, m| x.min(m)))
        }

        NodeType::Remap => {
            let value = get_input("in");
            let in_min = get_input("in_min");
            let in_max = get_input("in_max");
            let out_min = get_input("out_min");
            let out_max = get_input("out_max");

            let numer = binary_numeric(&value.value, &in_min.value, |v, min| v - min);
            let denom = binary_numeric(&in_max.value, &in_min.value, |max, min| max - min);
            let ratio = binary_numeric(
                &numer,
                &denom,
                |n, d| if d != 0.0 { n / d } else { f32::NAN },
            );
            let ratio_clamped = unary_numeric(&ratio, |x| x.clamp(0.0, 1.0));
            let span = binary_numeric(&out_max.value, &out_min.value, |max, min| max - min);
            let scaled = binary_numeric(&ratio_clamped, &span, |t, span| t * span);
            out_map!(binary_numeric(
                &scaled,
                &out_min.value,
                |scaled, min| scaled + min
            ))
        }

        NodeType::Vec3Cross => {
            let a_val = get_input("a");
            let b_val = get_input("b");
            match (flatten_numeric(&a_val.value), flatten_numeric(&b_val.value)) {
                (Some(a_flat), Some(b_flat))
                    if a_flat.layout.scalar_len() == 3 && b_flat.layout.scalar_len() == 3 =>
                {
                    let ax = a_flat.data[0];
                    let ay = a_flat.data[1];
                    let az = a_flat.data[2];
                    let bx = b_flat.data[0];
                    let by = b_flat.data[1];
                    let bz = b_flat.data[2];
                    let result = vec![ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx];
                    out_map!(a_flat.layout.reconstruct(&result))
                }
                (Some(a_flat), _) => out_map!(a_flat.layout.fill_with(f32::NAN)),
                (_, Some(b_flat)) => out_map!(b_flat.layout.fill_with(f32::NAN)),
                _ => out_map!(Value::Vec3([f32::NAN, f32::NAN, f32::NAN])),
            }
        }

        // Generic vector utilities
        NodeType::VectorConstant => {
            if let Some(val) = &p.value {
                out_map!(val.clone())
            } else {
                out_map!(Value::Vector(Vec::new()))
            }
        }
        NodeType::VectorAdd => {
            let a = get_input("a");
            let b = get_input("b");
            out_map!(binary_numeric(&a.value, &b.value, |x, y| x + y))
        }
        NodeType::VectorSubtract => {
            let a = get_input("a");
            let b = get_input("b");
            out_map!(binary_numeric(&a.value, &b.value, |x, y| x - y))
        }
        NodeType::VectorMultiply => {
            let a = get_input("a");
            let b = get_input("b");
            out_map!(binary_numeric(&a.value, &b.value, |x, y| x * y))
        }
        NodeType::VectorScale => {
            let scalar = get_input("scalar");
            let vector = get_input("v");
            out_map!(binary_numeric(&vector.value, &scalar.value, |x, s| x * s))
        }
        NodeType::VectorNormalize => {
            let value = get_input("in");
            match flatten_numeric(&value.value) {
                Some(flat) => {
                    let len_sq: f32 = flat.data.iter().map(|x| x * x).sum();
                    let len = len_sq.sqrt();
                    let normalized: Vec<f32> = if len > 0.0 {
                        flat.data.iter().map(|x| *x / len).collect()
                    } else {
                        vec![f32::NAN; flat.data.len()]
                    };
                    out_map!(flat.layout.reconstruct(&normalized))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::VectorDot => {
            let a = get_input("a");
            let b = get_input("b");
            match (flatten_numeric(&a.value), flatten_numeric(&b.value)) {
                (Some(fa), Some(fb)) => match align_flattened(&fa, &fb) {
                    Ok((_, da, db)) => {
                        let sum = da.iter().zip(db.iter()).map(|(x, y)| x * y).sum::<f32>();
                        out_map!(Value::Float(sum))
                    }
                    Err(_) => out_map!(Value::Float(f32::NAN)),
                },
                _ => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::VectorLength => {
            let value = get_input("in");
            match flatten_numeric(&value.value) {
                Some(flat) => {
                    let len_sq: f32 = flat.data.iter().map(|x| x * x).sum();
                    out_map!(Value::Float(len_sq.sqrt()))
                }
                None => out_map!(Value::Float(f32::NAN)),
            }
        }
        NodeType::VectorIndex => {
            let value = get_input("v");
            let idx = as_float(&get_input("index").value);
            if let Some(flat) = flatten_numeric(&value.value) {
                let i = idx.floor() as isize;
                let len = flat.data.len() as isize;
                if i >= 0 && i < len {
                    out_map!(Value::Float(flat.data[i as usize]))
                } else {
                    out_map!(Value::Float(f32::NAN))
                }
            } else {
                out_map!(Value::Float(f32::NAN))
            }
        }

        // Placeholder implementations to satisfy exhaustive match (schema may not expose these yet)

        // Join: flatten all inputs (order of map iteration is arbitrary here)
        NodeType::Join => {
            let mut out: Vec<f32> = Vec::new();
            for v in ivals.values() {
                if let Some(flat) = flatten_numeric(&v.value) {
                    out.extend(flat.data);
                }
            }
            out_map!(Value::Vector(out))
        }

        // Split: sizes param (floored). Emit variadic outputs: part1..partN.
        // If sum(sizes) != len(v), produce NaN vectors for each requested size.
        NodeType::Split => {
            let input = get_input("in");
            let v = flatten_numeric(&input.value)
                .map(|f| f.data)
                .unwrap_or_default();
            let sizes = p.sizes.clone().unwrap_or_default();
            let sizes_usize: Vec<usize> =
                sizes.iter().map(|x| x.floor().max(0.0) as usize).collect();

            let mut map: HashMap<String, PortValue> = HashMap::new();
            if sizes_usize.is_empty() {
                // No sizes specified: emit a single empty vector as 'part1'
                map.insert(
                    "part1".to_string(),
                    PortValue::new(Value::Vector(Vec::new())),
                );
                map
            } else {
                let total: usize = sizes_usize.iter().sum();
                if total == v.len() {
                    let mut offset = 0usize;
                    for (i, sz) in sizes_usize.iter().copied().enumerate() {
                        let slice = v[offset..offset + sz].to_vec();
                        offset += sz;
                        map.insert(
                            format!("part{}", i + 1),
                            PortValue::new(Value::Vector(slice)),
                        );
                    }
                    map
                } else {
                    // Mismatch: return NaN vectors for each requested size
                    for (i, sz) in sizes_usize.iter().copied().enumerate() {
                        map.insert(
                            format!("part{}", i + 1),
                            PortValue::new(Value::Vector(vec![f32::NAN; sz])),
                        );
                    }
                    map
                }
            }
        }

        // Reducers (vector -> scalar)
        NodeType::VectorMin => {
            let value = get_input("in");
            let out = match flatten_numeric(&value.value) {
                Some(flat) if !flat.data.is_empty() => {
                    flat.data.iter().fold(f32::INFINITY, |acc, x| acc.min(*x))
                }
                _ => f32::NAN,
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMax => {
            let value = get_input("in");
            let out = match flatten_numeric(&value.value) {
                Some(flat) if !flat.data.is_empty() => flat
                    .data
                    .iter()
                    .fold(f32::NEG_INFINITY, |acc, x| acc.max(*x)),
                _ => f32::NAN,
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMean => {
            let value = get_input("in");
            let out = match flatten_numeric(&value.value) {
                Some(flat) if !flat.data.is_empty() => {
                    let sum: f32 = flat.data.iter().sum();
                    sum / (flat.data.len() as f32)
                }
                _ => f32::NAN,
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMedian => {
            let mut data = flatten_numeric(&get_input("in").value)
                .map(|f| f.data)
                .unwrap_or_default();
            let out = if data.is_empty() {
                f32::NAN
            } else {
                data.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let n = data.len();
                if n % 2 == 1 {
                    data[n / 2]
                } else {
                    (data[n / 2 - 1] + data[n / 2]) / 2.0
                }
            };
            out_map!(Value::Float(out))
        }
        NodeType::VectorMode => {
            let data = flatten_numeric(&get_input("in").value)
                .map(|f| f.data)
                .unwrap_or_default();
            let out = if data.is_empty() {
                f32::NAN
            } else {
                let mut map: hashbrown::HashMap<i64, (f32, usize)> = hashbrown::HashMap::new();
                for x in data {
                    if x.is_nan() {
                        continue;
                    }
                    let key = x.to_bits() as i64;
                    let entry = map.entry(key).or_insert((x, 0));
                    entry.1 += 1;
                }
                if map.is_empty() {
                    f32::NAN
                } else {
                    let mut best_val = f32::NAN;
                    let mut best_count = 0usize;
                    for (_k, (val, cnt)) in map.iter() {
                        if *cnt > best_count {
                            best_count = *cnt;
                            best_val = *val;
                        } else if *cnt == best_count && val < &best_val {
                            best_val = *val;
                        }
                    }
                    best_val
                }
            };
            out_map!(Value::Float(out))
        }

        NodeType::InverseKinematics => {
            let l1 = as_float(&get_input("bone1").value);
            let l2 = as_float(&get_input("bone2").value);
            let l3 = as_float(&get_input("bone3").value);
            let theta = as_float(&get_input("theta").value);
            let x = as_float(&get_input("x").value);
            let y = as_float(&get_input("y").value);

            let wx = x - l3 * theta.cos();
            let wy = y - l3 * theta.sin();
            let dist_sq = wx * wx + wy * wy;

            out_map!(
                if dist_sq > (l1 + l2) * (l1 + l2) || dist_sq < (l1 - l2) * (l1 - l2) {
                    Value::Vec3([f32::NAN, f32::NAN, f32::NAN])
                } else {
                    // let dist = dist_sq.sqrt();
                    let cos_angle2 = (dist_sq - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
                    let angle2 = cos_angle2.acos();
                    let angle1 = wy.atan2(wx) - (l2 * angle2.sin()).atan2(l1 + l2 * angle2.cos());
                    let angle3 = theta - angle1 - angle2;
                    Value::Vec3([angle1, angle2, angle3])
                }
            )
        }

        NodeType::Output => out_map!(get_input("in").value),
    };

    let pending_write = spec.params.path.as_ref().and_then(|path| {
        outputs
            .get("out")
            .map(|pv| (path.clone(), pv.value.clone()))
    });

    enforce_output_shapes(spec, &mut outputs)?;
    if let Some((path, value)) = pending_write {
        rt.writes.push(WriteOp::new(path, value));
    }
    rt.outputs.insert(spec.id.clone(), outputs);
    Ok(())
}

fn enforce_output_shapes(
    spec: &NodeSpec,
    outputs: &mut HashMap<String, PortValue>,
) -> Result<(), String> {
    if spec.output_shapes.is_empty() {
        return Ok(());
    }

    for (key, declared) in spec.output_shapes.iter() {
        let port = outputs.get_mut(key).ok_or_else(|| {
            format!(
                "node '{}' missing declared output '{}' during evaluation",
                spec.id, key
            )
        })?;

        if !value_matches_shape(&declared.id, &port.value) {
            return Err(format!(
                "node '{}' output '{}' does not match declared shape {:?}",
                spec.id, key, declared.id
            ));
        }

        port.shape = declared.clone();
    }

    Ok(())
}

fn value_matches_shape(shape: &ShapeId, value: &Value) -> bool {
    match shape {
        ShapeId::Scalar => matches!(value, Value::Float(_)),
        ShapeId::Bool => matches!(value, Value::Bool(_)),
        ShapeId::Vec2 => matches!(value, Value::Vec2(_)),
        ShapeId::Vec3 => matches!(value, Value::Vec3(_)),
        ShapeId::Vec4 => matches!(value, Value::Vec4(_)),
        ShapeId::Quat => matches!(value, Value::Quat(_)),
        ShapeId::ColorRgba => matches!(value, Value::ColorRgba(_)),
        ShapeId::Transform => matches!(value, Value::Transform { .. }),
        ShapeId::Text => matches!(value, Value::Text(_)),
        ShapeId::Vector { len } => match value {
            Value::Vector(items) => match len {
                Some(expected) => items.len() == *expected,
                None => true,
            },
            _ => false,
        },
        ShapeId::Record(fields) => match value {
            Value::Record(map) => fields.iter().all(|field| {
                map.get(&field.name)
                    .map(|v| value_matches_shape(&field.shape, v))
                    .unwrap_or(false)
            }),
            _ => false,
        },
        ShapeId::Array(inner, len) => match value {
            Value::Array(items) => {
                items.len() == *len && items.iter().all(|item| value_matches_shape(inner, item))
            }
            _ => false,
        },
        ShapeId::List(inner) => match value {
            Value::List(items) => items.iter().all(|item| value_matches_shape(inner, item)),
            _ => false,
        },
        ShapeId::Tuple(entries) => match value {
            Value::Tuple(items) => {
                items.len() == entries.len()
                    && items
                        .iter()
                        .zip(entries.iter())
                        .all(|(item, shape)| value_matches_shape(shape, item))
            }
            _ => false,
        },
        ShapeId::Enum(variants) => match value {
            Value::Enum(tag, boxed) => variants
                .iter()
                .find(|(variant, _)| variant == tag)
                .is_some_and(|(_, shape)| value_matches_shape(shape, boxed)),
            _ => false,
        },
    }
}
pub fn evaluate_all(rt: &mut GraphRuntime, spec: &GraphSpec) -> Result<(), String> {
    let order = crate::topo::topo_order(&spec.nodes)?;
    for id in order {
        if let Some(node) = spec.nodes.iter().find(|n| n.id == id) {
            eval_node(rt, node)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GraphSpec, InputConnection, NodeParams, NodeType};
    use hashbrown::HashMap;

    fn constant_node(id: &str, value: Value) -> NodeSpec {
        NodeSpec {
            id: id.to_string(),
            kind: NodeType::Constant,
            params: NodeParams {
                value: Some(value),
                ..Default::default()
            },
            inputs: HashMap::new(),
            output_shapes: HashMap::new(),
        }
    }

    #[test]
    fn it_should_respect_declared_shape() {
        let mut node = constant_node("a", Value::Float(1.0));
        node.output_shapes
            .insert("out".to_string(), Shape::new(ShapeId::Scalar));

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("shape should match");
        let outputs = rt.outputs.get("a").expect("outputs present");
        let port = outputs.get("out").expect("out port present");
        assert!(matches!(port.shape.id, ShapeId::Scalar));
    }

    #[test]
    fn it_should_error_when_shape_mismatches() {
        let mut node = constant_node("a", Value::Float(1.0));
        node.output_shapes
            .insert("out".to_string(), Shape::new(ShapeId::Vec3));

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("should fail due to mismatch");
        assert!(err.contains("does not match declared shape"));
    }

    #[test]
    fn it_should_emit_write_for_output_nodes() {
        let mut output_inputs = HashMap::new();
        output_inputs.insert(
            "in".to_string(),
            InputConnection {
                node_id: "src".to_string(),
                output_key: "out".to_string(),
            },
        );

        let graph = GraphSpec {
            nodes: vec![
                constant_node("src", Value::Float(2.0)),
                NodeSpec {
                    id: "out".to_string(),
                    kind: NodeType::Output,
                    params: NodeParams {
                        path: Some(
                            vizij_api_core::TypedPath::parse("robot1/Arm/Joint.angle")
                                .expect("valid path"),
                        ),
                        ..Default::default()
                    },
                    inputs: output_inputs,
                    output_shapes: HashMap::new(),
                },
            ],
        };

        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &graph).expect("graph should evaluate");
        assert_eq!(rt.writes.iter().count(), 1);
        let op = rt.writes.iter().next().expect("write present");
        assert_eq!(op.path.to_string(), "robot1/Arm/Joint.angle");
        match op.value {
            Value::Float(f) => assert_eq!(f, 2.0),
            _ => panic!("expected float write"),
        }
    }

    #[test]
    fn it_should_infer_vector_length_hints() {
        let node = constant_node("vec", Value::Vector(vec![1.0, 2.0, 3.0]));
        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        evaluate_all(&mut rt, &spec).expect("graph should evaluate");
        let outputs = rt.outputs.get("vec").expect("outputs present");
        let port = outputs.get("out").expect("out port");
        match &port.shape.id {
            ShapeId::Vector { len } => assert_eq!(*len, Some(3)),
            other => panic!("expected vector shape, got {:?}", other),
        }
    }

    #[test]
    fn it_should_error_when_declared_output_missing() {
        let mut node = constant_node("a", Value::Float(1.0));
        node.output_shapes
            .insert("secondary".to_string(), Shape::new(ShapeId::Scalar));

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("missing declared output should error");
        assert!(err.contains("missing declared output"));
    }

    #[test]
    fn it_should_validate_vector_length_against_declared_shape() {
        let mut node = constant_node("a", Value::Vector(vec![1.0, 2.0, 3.0]));
        node.output_shapes.insert(
            "out".to_string(),
            Shape::new(ShapeId::Vector { len: Some(4) }),
        );

        let spec = GraphSpec { nodes: vec![node] };
        let mut rt = GraphRuntime::default();
        let err = evaluate_all(&mut rt, &spec).expect_err("vector length mismatch should error");
        assert!(err.contains("does not match declared shape"));
    }

    #[test]
    fn it_should_reject_invalid_paths_during_deserialization() {
        let json = r#"{
            "id": "node",
            "type": "output",
            "params": { "path": "robot/invalid/" },
            "inputs": {},
            "output_shapes": {}
        }"#;

        let err = serde_json::from_str::<NodeSpec>(json)
            .expect_err("invalid typed path should fail to parse");
        assert!(err.to_string().contains("path"));
    }
}
