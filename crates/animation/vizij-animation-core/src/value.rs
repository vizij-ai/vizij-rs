//! Compatibility shim re-exporting the shared Vizij value types.
//!
//! Older animation-core modules still import `crate::value::Value`, so this module keeps that
//! path stable while delegating to `vizij_api_core`.

pub use vizij_api_core::{Value, ValueKind};
