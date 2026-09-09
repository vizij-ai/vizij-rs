//! The render boundary: what draws a Vizij face, separated from what runs it.
//!
//! This crate holds the half of the view that is about pixels — reading a
//! GLB's Vizij metadata, joining it to the spawned Bevy scene, fitting the
//! camera, and applying values onto transforms, materials and morph weights.
//! It knows nothing about devices, runtimes, bridges or operators.
//!
//! # The seam
//!
//! Values arrive through [`PoseFeed`], a closure the host installs. That is
//! the whole input contract: a host holding an Arora device passes
//! `move || rig.pose()`; a host replaying a recording passes a closure over
//! its own samples; a test passes a fixed vector. Nothing here depends on
//! where the values came from, so nothing here has to change when that
//! changes.
//!
//! Keys are [`vizij_api_core::TypedPath`]s and values are
//! [`vizij_api_core::Value`], which is the same type Arora's data plane
//! carries — a host converts nothing.

pub mod meta;
pub mod view;

pub use meta::{Binding, Element, FaceMeta, FeatureKind};
pub use view::{
    BindingIndex, Face, Fit, OffscreenTarget, PoseFeed, ViewCamera, ViewOptions, ViewPlugin,
    ViewSystems,
};
