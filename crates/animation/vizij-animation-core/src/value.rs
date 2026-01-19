//! Re-exports for the shared `Value` types used by the animation runtime.
//!
//! This shim keeps existing `crate::value::Value` imports working while the
//! codebase migrates to `vizij_api_core::Value`.
pub use vizij_api_core::{Value, ValueKind};
