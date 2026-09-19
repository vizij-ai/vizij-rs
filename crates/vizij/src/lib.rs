//! Vizij: an arora with a head.
//!
//! One crate, two halves, every target. The [`view`] is the Bevy rendering of
//! a face (a GLB with `RobotData` bindings and a `VIZIJ_bundle`), applying a
//! device's pose each frame; the [`device`] is the arora running the face's
//! composed graphs behind it. The entry points compose the two: the desktop
//! binary (`main.rs`, feature `desktop`) with its CLI, terminal operator UI
//! and bridges; the browser module ([`web`], the wasm-bindgen surface behind
//! `@vizij/runtime`) and the Android activity build on the same library
//! without the desktop half.

pub mod device;
pub mod view;
#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(all(test, feature = "desktop"))]
mod memory_tests;
#[cfg(all(test, feature = "ros2-dds"))]
mod ros2_tests;
