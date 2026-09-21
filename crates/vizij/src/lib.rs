//! Vizij: an arora with a head.
//!
//! One crate, every target: components of a device, and one entry point that
//! spawns a device out of them. The [`view`] is the Bevy rendering of a face
//! (a GLB with `RobotData` bindings and a `VIZIJ_bundle`), applying a rig's
//! pose each frame; the [`face`] is that GLB composed into the behavior graph
//! an Arora runs, over the `RigHal` the view reads; the [`modules`] are the
//! host modules any Arora loads. None of them is a device — a robot that is
//! one already folds them into its own builder beside its own hardware. The
//! [`native`] driver is the stand-alone case: the face as the whole of a
//! device's hardware, on a worker thread under arora's operator flow, with
//! the bridges a build adds; the desktop binary (`main.rs`, feature
//! `desktop`) puts a CLI, a window and a terminal UI on it. The browser
//! module and the Android activity build on the same library without the
//! desktop half.

pub mod face;
pub mod modules;
#[cfg(not(target_arch = "wasm32"))]
pub mod native;
pub mod view;

#[cfg(all(test, feature = "desktop"))]
mod memory_tests;
#[cfg(all(test, feature = "ros2-dds"))]
mod ros2_tests;
