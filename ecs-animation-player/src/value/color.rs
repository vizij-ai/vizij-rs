use crate::value::utils::hash_f64;
use bevy::prelude::Reflect;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

/// Color representation supporting multiple formats
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub enum Color {
    /// RGB color (0.0-1.0 range)
    Rgb { r: f64, g: f64, b: f64 },
    /// RGBA color (0.0-1.0 range)
    Rgba { r: f64, g: f64, b: f64, a: f64 },
    /// HSL color (h: 0-360, s: 0-1, l: 0-1)
    Hsl { h: f64, s: f64, l: f64 },
    /// HSLA color (h: 0-360, s: 0-1, l: 0-1, a: 0-1)
    Hsla { h: f64, s: f64, l: f64, a: f64 },
    /// Hex color (#RRGGBB or #RRGGBBAA)
    Hex(String),
}

impl Default for Color {
    fn default() -> Self {
        Color::Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        }
    }
}

impl Hash for Color {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Rgb { r, g, b } => {
                0u8.hash(state); // discriminant
                hash_f64(*r, state);
                hash_f64(*g, state);
                hash_f64(*b, state);
            }
            Self::Rgba { r, g, b, a } => {
                1u8.hash(state); // discriminant
                hash_f64(*r, state);
                hash_f64(*g, state);
                hash_f64(*b, state);
                hash_f64(*a, state);
            }
            Self::Hsl { h, s, l } => {
                2u8.hash(state); // discriminant
                hash_f64(*h, state);
                hash_f64(*s, state);
                hash_f64(*l, state);
            }
            Self::Hsla { h, s, l, a } => {
                3u8.hash(state); // discriminant
                hash_f64(*h, state);
                hash_f64(*s, state);
                hash_f64(*l, state);
                hash_f64(*a, state);
            }
            Self::Hex(hex) => {
                4u8.hash(state); // discriminant
                hex.hash(state);
            }
        }
    }
}

impl Color {
    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self::Rgb { r, g, b }
    }

    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self::Rgba { r, g, b, a }
    }

    pub fn hsl(h: f64, s: f64, l: f64) -> Self {
        Self::Hsl { h, s, l }
    }

    pub fn hsla(h: f64, s: f64, l: f64, a: f64) -> Self {
        Self::Hsla { h, s, l, a }
    }

    pub fn hex(hex: impl Into<String>) -> Self {
        Self::Hex(hex.into())
    }

    /// Convert to RGBA format
    pub fn to_rgba(&self) -> (f64, f64, f64, f64) {
        match self {
            Self::Rgb { r, g, b } => (*r, *g, *b, 1.0),
            Self::Rgba { r, g, b, a } => (*r, *g, *b, *a),
            Self::Hsl { h, s, l } => {
                let (r, g, b) = hsl_to_rgb(*h, *s, *l);
                (r, g, b, 1.0)
            }
            Self::Hsla { h, s, l, a } => {
                let (r, g, b) = hsl_to_rgb(*h, *s, *l);
                (r, g, b, *a)
            }
            Self::Hex(hex) => {
                // Simple hex parsing - could be enhanced
                let hex = hex.trim_start_matches('#');
                match hex.len() {
                    6 => {
                        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;
                        (r, g, b, 1.0)
                    }
                    8 => {
                        let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0) as f64 / 255.0;
                        let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0) as f64 / 255.0;
                        let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0) as f64 / 255.0;
                        let a = u8::from_str_radix(&hex[6..8], 16).unwrap_or(255) as f64 / 255.0;
                        (r, g, b, a)
                    }
                    _ => (0.0, 0.0, 0.0, 1.0),
                }
            }
        }
    }
}

/// Convert HSL to RGB
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (f64, f64, f64) {
    let h = h / 360.0;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 1.0 / 6.0 {
        (c, x, 0.0)
    } else if h < 2.0 / 6.0 {
        (x, c, 0.0)
    } else if h < 3.0 / 6.0 {
        (0.0, c, x)
    } else if h < 4.0 / 6.0 {
        (0.0, x, c)
    } else if h < 5.0 / 6.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r + m, g + m, b + m)
}
