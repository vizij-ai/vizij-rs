//! Value types for animation data

pub mod color;
pub mod euler; // Add the new module
pub mod transform;
pub mod utils;
pub mod value_enum;
pub mod vector2;
pub mod vector3;
pub mod vector4;

pub use color::*;
pub use transform::*;
pub use utils::*;
pub use value_enum::*;
pub use vector2::*;
pub use vector3::*;
pub use vector4::*;
