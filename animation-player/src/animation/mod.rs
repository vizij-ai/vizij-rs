//! Animation data structures

pub mod baking;
pub mod data;
pub mod group; // Add the new module to make it accessible
pub mod ids;
pub mod instance;
pub mod keypoint;
pub mod metadata;
pub mod track;
pub mod transition;

pub use baking::*;
pub use data::*;
pub use ids::*;
pub use instance::*;
pub use keypoint::*;
pub use metadata::*;
pub use track::*;
pub use transition::*;
