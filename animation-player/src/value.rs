use serde::{Deserialize, Serialize};

pub type Boolean = bool;
pub type Number = f64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Euler {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RGB {
    pub r: f64,
    pub g: f64,
    pub b: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HSL {
    pub h: f64,
    pub s: f64,
    pub l: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Color {
    RGB(RGB),
    HSL(HSL),
}

// Define the Value enum
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Boolean(Boolean),
    Number(Number),
    String(String),
    Vector3(Vector3),
    Vector2(Vector2),
    Euler(Euler),
    Color(Color),
    StringsAndFloats((Vec<String>, Vec<f64>)),
}

impl TryInto<bool> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            Value::Boolean(value) => Ok(value),
            _ => Err("Value is not a boolean"),
        }
    }
}

impl TryInto<f64> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<f64, Self::Error> {
        match self {
            Value::Number(value) => Ok(value),
            _ => Err("Value is not a number"),
        }
    }
}

impl TryInto<String> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<String, Self::Error> {
        match self {
            Value::String(value) => Ok(value),
            _ => Err("Value is not a string"),
        }
    }
}

impl TryInto<Vector3> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<Vector3, Self::Error> {
        match self {
            Value::Vector3(value) => Ok(value),
            _ => Err("Value is not a Vector3"),
        }
    }
}

impl TryInto<Vector2> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<Vector2, Self::Error> {
        match self {
            Value::Vector2(value) => Ok(value),
            _ => Err("Value is not a Vector2"),
        }
    }
}

impl TryInto<Euler> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<Euler, Self::Error> {
        match self {
            Value::Euler(value) => Ok(value),
            _ => Err("Value is not an Euler"),
        }
    }
}

impl TryInto<Color> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<Color, Self::Error> {
        match self {
            Value::Color(value) => Ok(value),
            _ => Err("Value is not a Color"),
        }
    }
}

impl TryInto<RGB> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<RGB, Self::Error> {
        match self {
            Value::Color(Color::RGB(value)) => Ok(value),
            _ => Err("Value is not an RGB"),
        }
    }
}

impl TryInto<HSL> for Value {
    type Error = &'static str;

    fn try_into(self) -> Result<HSL, Self::Error> {
        match self {
            Value::Color(Color::HSL(value)) => Ok(value),
            _ => Err("Value is not an HSL"),
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Boolean(value)
    }
}

impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Number(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<Vector3> for Value {
    fn from(value: Vector3) -> Self {
        Value::Vector3(value)
    }
}

impl From<Vector2> for Value {
    fn from(value: Vector2) -> Self {
        Value::Vector2(value)
    }
}

impl From<Euler> for Value {
    fn from(value: Euler) -> Self {
        Value::Euler(value)
    }
}

impl From<Color> for Value {
    fn from(value: Color) -> Self {
        Value::Color(value)
    }
}

impl From<RGB> for Value {
    fn from(value: RGB) -> Self {
        Value::Color(Color::RGB(value))
    }
}

impl From<HSL> for Value {
    fn from(value: HSL) -> Self {
        Value::Color(Color::HSL(value))
    }
}
