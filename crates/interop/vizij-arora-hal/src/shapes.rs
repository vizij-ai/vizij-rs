//! [`ShapeHal`]: the Vizij shape-creation seam (VIZ-53 scaffold).
//!
//! ## Why shapes live in the HAL
//!
//! In the Vizij-on-Arora split, **Arora owns the runtime** and Vizij supplies
//! the device-specific pieces around it. Behaviors (node graphs, animation, the
//! behavior tree) are portable and address the world only through the shared
//! [`DataStore`](arora_types::data::DataStore). *Creating and editing shapes* is
//! not portable behavior — it is a Vizij-specific capability of the "device"
//! (the workspace / renderer). So, exactly like a rig exposes bones and morphs
//! through [`RigHal`](crate::RigHal), a workspace exposes shape creation through
//! a **HAL interface**: the app holds a pointer to something implementing
//! [`ShapeHal`] and calls it to spawn or edit shapes.
//!
//! ```
//! use std::sync::Arc;
//! use vizij_arora_hal::shapes::{ShapeHal, ShapeSpec, ShapeKind};
//!
//! /// The workspace app holds a *pointer* to a shape-creating HAL interface;
//! /// it never depends on a concrete implementation.
//! struct WorkspaceApp {
//!     shapes: Arc<dyn ShapeHal>,
//! }
//!
//! impl WorkspaceApp {
//!     fn spawn_dot(&self) {
//!         let _ = self.shapes.create_shape(ShapeSpec {
//!             kind: ShapeKind::Point,
//!             name: Some("cursor".into()),
//!             ..Default::default()
//!         });
//!     }
//! }
//! ```
//!
//! This module is a **scaffold**: the trait, its handle/spec types and their doc
//! contract. A concrete implementation (backed by the actual renderer / scene
//! graph) is a separate step — the surface here is what the workspace app and a
//! renderer agree on. Live *graph* editing is a distinct concern handled on the
//! Arora side; see `TODO(ARORA-53)` in the behavior crate.

use std::collections::HashMap;

use arora_hal::HalResult;
use vizij_api_core::Value as VValue;

/// An opaque handle to a shape held by a [`ShapeHal`]. Returned by
/// [`ShapeHal::create_shape`] and used to address later edits/removals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ShapeId(pub u64);

/// The kind of primitive a shape is. Intentionally small for the scaffold; the
/// set grows as the renderer gains primitives (curves, text, images, groups…).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShapeKind {
    /// A single point / dot.
    #[default]
    Point,
    /// A straight segment between two endpoints.
    Line,
    /// A connected sequence of segments.
    Polyline,
    /// A circle / ellipse.
    Circle,
    /// An axis-aligned rectangle.
    Rectangle,
    /// A free-form path.
    Path,
    /// An imported/authored mesh.
    Mesh,
}

/// A request to create a shape.
///
/// Geometry and style beyond the [`kind`](ShapeSpec::kind) ride in
/// [`params`](ShapeSpec::params) as native Vizij [`Value`](VValue)s (e.g.
/// `"radius" -> Float`, `"points" -> List`, `"color" -> ColorRgba`), so the
/// vocabulary is open without changing this type. Placement is an optional
/// Vizij `Transform` value.
#[derive(Clone, Debug, Default)]
pub struct ShapeSpec {
    /// Which primitive to create.
    pub kind: ShapeKind,
    /// Optional human/debug name.
    pub name: Option<String>,
    /// World placement as a Vizij `Transform` value (translation/rotation/scale).
    pub transform: Option<VValue>,
    /// Kind-specific geometry and style, keyed by parameter name.
    pub params: HashMap<String, VValue>,
}

/// A partial edit to an existing shape. `None` / empty fields are left unchanged.
#[derive(Clone, Debug, Default)]
pub struct ShapePatch {
    /// Replace the placement transform when `Some`.
    pub transform: Option<VValue>,
    /// Overlay these parameters onto the shape's current ones.
    pub params: HashMap<String, VValue>,
}

/// The shape-creation seam a Vizij workspace exposes to the app.
///
/// The workspace app holds a pointer to one of these (`Arc<dyn ShapeHal>` /
/// `Box<dyn ShapeHal>`) and drives shape lifecycle through it. It is `Send +
/// Sync` and takes `&self` throughout so a single instance can be shared (a
/// concrete impl uses interior mutability, as [`RigHal`](crate::RigHal) does).
///
/// This is the scaffolded contract only; implementations land separately.
pub trait ShapeHal: Send + Sync {
    /// Create a shape from `spec`, returning its handle.
    fn create_shape(&self, spec: ShapeSpec) -> HalResult<ShapeId>;

    /// Apply a partial edit to the shape `id`. Errors if `id` is unknown.
    fn update_shape(&self, id: ShapeId, patch: ShapePatch) -> HalResult<()>;

    /// Remove the shape `id`. Errors if `id` is unknown.
    fn remove_shape(&self, id: ShapeId) -> HalResult<()>;

    /// Handles of all shapes this HAL currently holds.
    fn shapes(&self) -> HalResult<Vec<ShapeId>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_hal::HalError;
    use std::sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    };

    /// A throwaway in-memory `ShapeHal` — enough to prove the trait is object-safe,
    /// shareable behind a pointer, and usable by an app. NOT the real impl.
    #[derive(Default)]
    struct MockShapeHal {
        next: AtomicU64,
        store: Mutex<HashMap<ShapeId, ShapeSpec>>,
    }

    impl ShapeHal for MockShapeHal {
        fn create_shape(&self, spec: ShapeSpec) -> HalResult<ShapeId> {
            let id = ShapeId(self.next.fetch_add(1, Ordering::Relaxed));
            self.store.lock().unwrap().insert(id, spec);
            Ok(id)
        }
        fn update_shape(&self, id: ShapeId, patch: ShapePatch) -> HalResult<()> {
            let mut store = self.store.lock().unwrap();
            let spec = store
                .get_mut(&id)
                .ok_or_else(|| HalError::NoSuchKey(format!("{id:?}")))?;
            if patch.transform.is_some() {
                spec.transform = patch.transform;
            }
            spec.params.extend(patch.params);
            Ok(())
        }
        fn remove_shape(&self, id: ShapeId) -> HalResult<()> {
            self.store
                .lock()
                .unwrap()
                .remove(&id)
                .map(|_| ())
                .ok_or_else(|| HalError::NoSuchKey(format!("{id:?}")))
        }
        fn shapes(&self) -> HalResult<Vec<ShapeId>> {
            Ok(self.store.lock().unwrap().keys().copied().collect())
        }
    }

    /// Mirrors the module-doc pattern: the app holds only a pointer to the seam.
    struct WorkspaceApp {
        shapes: Arc<dyn ShapeHal>,
    }

    #[test]
    fn app_creates_and_edits_shapes_through_the_pointer() {
        let app = WorkspaceApp {
            shapes: Arc::new(MockShapeHal::default()),
        };

        let id = app
            .shapes
            .create_shape(ShapeSpec {
                kind: ShapeKind::Circle,
                name: Some("dot".into()),
                ..Default::default()
            })
            .expect("create");

        app.shapes
            .update_shape(
                id,
                ShapePatch {
                    params: HashMap::from([("radius".into(), VValue::Float(2.0))]),
                    ..Default::default()
                },
            )
            .expect("update");

        assert_eq!(app.shapes.shapes().unwrap(), vec![id]);

        app.shapes.remove_shape(id).expect("remove");
        assert!(app.shapes.shapes().unwrap().is_empty());
        // Editing a removed shape is an error.
        assert!(app.shapes.update_shape(id, ShapePatch::default()).is_err());
    }
}
