//! Rewrites the Value-bearing JSON embedded in Vizij face-bundle `.glb`
//! files to canonical arora `Value` serde.
//!
//! [`glb`] is a minimal codec for the GLB container; [`migrate`] walks the
//! glTF JSON document and rewrites the Value payloads carried by
//! `VIZIJ_bundle` graph documents and `RobotData` feature defaults. The
//! README lists the exact document paths touched.

pub mod glb;
pub mod migrate;
