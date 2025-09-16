// Re-export the shared API Value/ValueKind so existing crate code that imports
// `crate::value::Value` continues to work during migration.
//
// The original local enum has been replaced by the unified `vizij_api_core::Value`.
// Keeping this shim avoids having to update every single import site at once.
pub use vizij_api_core::{Value, ValueKind};
