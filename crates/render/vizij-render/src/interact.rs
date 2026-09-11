//! Pointer hits and screen-space anchors: what a host needs to put its own UI
//! over the canvas.
//!
//! Both are sinks the host installs, mirroring [`crate::PoseFeed`] on the way
//! in. The renderer reports where things are and what was hit; it has no
//! opinion about what a host does with that — draw DOM tooltips over a browser
//! canvas, open a native context menu, or log it.

use bevy::camera::primitives::Aabb;
use bevy::camera::{Camera, SubCameraView};
use bevy::ecs::event::EntityEvent;
use bevy::picking::events::{Click, Pointer};
use bevy::prelude::*;
use serde::Serialize;

use crate::view::ViewCamera;

/// The glTF node name of the element an entity belongs to.
///
/// Carried by the mesh entities the scene join binds, so a pointer hit can be
/// answered in the vocabulary the face's metadata uses rather than in entity
/// ids, which mean nothing outside this process.
#[derive(Component, Clone, Debug)]
pub struct FaceElement(pub String);

/// Where a pointer hit goes.
#[derive(Resource)]
pub struct PickSink(pub Box<dyn Fn(&str) + Send + Sync>);

impl PickSink {
    pub fn new(sink: impl Fn(&str) + Send + Sync + 'static) -> Self {
        Self(Box::new(sink))
    }
}

/// One element's screen-space box, in logical pixels of the render target.
///
/// `x`/`y` is the projected centre; the bounds are the projected extent of the
/// element's axis-aligned bounding box, which is what an overlay anchors to —
/// a label wants the element's top edge, not its origin.
#[derive(Clone, Debug, Serialize)]
pub struct Anchor {
    pub element: String,
    pub x: f32,
    pub y: f32,
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

/// What a host installs to receive the per-frame anchors.
pub type AnchorFn = Box<dyn Fn(&[Anchor]) + Send + Sync>;

/// Where the per-frame screen-space anchors go.
///
/// Called once per frame with every bound element. A host that overlays UI on
/// the canvas needs this every frame, not on demand: the camera moves, and a
/// label that lags by a frame reads as detached.
#[derive(Resource)]
pub struct AnchorSink(pub AnchorFn);

impl AnchorSink {
    pub fn new(sink: impl Fn(&[Anchor]) + Send + Sync + 'static) -> Self {
        Self(Box::new(sink))
    }
}

/// The host's view offset, read each frame.
///
/// Bevy's [`SubCameraView`] is the analogue of three's `setViewOffset`: it
/// renders a sub-rectangle of a larger notional viewport, which is how a
/// canvas keeps its subject centred while a panel covers part of it. A host
/// that animates such a panel returns a changing offset here.
#[derive(Resource)]
pub struct ViewOffsetFeed(pub Box<dyn Fn() -> Option<SubCameraView> + Send + Sync>);

impl ViewOffsetFeed {
    pub fn new(feed: impl Fn() -> Option<SubCameraView> + Send + Sync + 'static) -> Self {
        Self(Box::new(feed))
    }
}

/// Reports a click as the element that was hit.
///
/// The hit entity is a mesh, which may sit below the named node the element
/// binds, so the answer is the nearest ancestor carrying a [`FaceElement`].
pub(crate) fn report_click(
    event: On<Pointer<Click>>,
    sink: Option<Res<PickSink>>,
    elements: Query<&FaceElement>,
    parents: Query<&ChildOf>,
) {
    let Some(sink) = sink else { return };
    if let Some(element) = element_of(event.event_target(), &elements, &parents) {
        (sink.0)(&element);
    }
}

/// The element an entity belongs to: itself, or its nearest ancestor carrying
/// one. `None` when nothing on the chain is part of a bound element.
fn element_of(
    entity: Entity,
    elements: &Query<&FaceElement>,
    parents: &Query<&ChildOf>,
) -> Option<String> {
    let mut current = entity;
    loop {
        if let Ok(element) = elements.get(current) {
            return Some(element.0.clone());
        }
        current = parents.get(current).ok()?.parent();
    }
}

/// Applies the host's view offset to the view camera.
pub(crate) fn apply_view_offset(
    feed: Option<Res<ViewOffsetFeed>>,
    mut cameras: Query<&mut Camera, With<ViewCamera>>,
) {
    let Some(feed) = feed else { return };
    let offset = (feed.0)();
    for mut camera in &mut cameras {
        if camera.sub_camera_view != offset {
            camera.sub_camera_view = offset;
        }
    }
}

/// Projects every bound element's bounding box to screen space.
///
/// Runs in `Last`, after transform propagation, so the positions published are
/// the ones this frame drew rather than the previous frame's.
pub(crate) fn publish_anchors(
    sink: Option<Res<AnchorSink>>,
    camera: Query<(&Camera, &GlobalTransform), With<ViewCamera>>,
    elements: Query<(&FaceElement, &GlobalTransform, &Aabb)>,
) {
    let Some(sink) = sink else { return };
    let Ok((camera, camera_transform)) = camera.single() else {
        return;
    };

    let mut anchors = Vec::new();
    for (element, transform, aabb) in &elements {
        let centre = Vec3::from(aabb.center);
        let extents = Vec3::from(aabb.half_extents);
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        let mut off_screen = false;
        for corner in 0..8u8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let world = transform.transform_point(centre + sign * extents);
            match camera.world_to_viewport(camera_transform, world) {
                Ok(point) => {
                    min = min.min(point);
                    max = max.max(point);
                }
                // A corner behind the camera or outside the target makes the
                // box meaningless; a host cannot anchor to half a projection.
                Err(_) => {
                    off_screen = true;
                    break;
                }
            }
        }
        if off_screen {
            continue;
        }
        anchors.push(Anchor {
            element: element.0.clone(),
            x: (min.x + max.x) / 2.0,
            y: (min.y + max.y) / 2.0,
            min_x: min.x,
            min_y: min.y,
            max_x: max.x,
            max_y: max.y,
        });
    }
    (sink.0)(&anchors);
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::SystemState;

    use super::*;

    /// Walks a spawned chain the way the observer does.
    fn element_for(world: &mut World, entity: Entity) -> Option<String> {
        let mut state: SystemState<(Query<&FaceElement>, Query<&ChildOf>)> =
            SystemState::new(world);
        let (elements, parents) = state.get(world).expect("the queries resolve");
        element_of(entity, &elements, &parents)
    }

    #[test]
    fn a_hit_answers_with_the_nearest_ancestor_that_is_an_element() {
        // A pointer hits a mesh, which sits below the named node an element
        // binds — so the answer has to come from up the chain, not the hit.
        let mut world = World::new();
        let node = world.spawn(FaceElement("L_EyeWhite".into())).id();
        let group = world.spawn(ChildOf(node)).id();
        let mesh = world.spawn(ChildOf(group)).id();

        assert_eq!(element_for(&mut world, mesh).as_deref(), Some("L_EyeWhite"));
    }

    #[test]
    fn the_nearest_element_wins_over_a_further_one() {
        // Elements nest, and a hit belongs to the innermost one that claims it.
        let mut world = World::new();
        let outer = world.spawn(FaceElement("Face".into())).id();
        let inner = world
            .spawn((FaceElement("Mouth".into()), ChildOf(outer)))
            .id();
        let mesh = world.spawn(ChildOf(inner)).id();

        assert_eq!(element_for(&mut world, mesh).as_deref(), Some("Mouth"));
    }

    #[test]
    fn an_element_answers_for_itself() {
        let mut world = World::new();
        let node = world.spawn(FaceElement("Jaw".into())).id();

        assert_eq!(element_for(&mut world, node).as_deref(), Some("Jaw"));
    }

    #[test]
    fn a_hit_on_nothing_bound_answers_nothing() {
        // The scene holds entities no element claims — the grid, a camera, a
        // mesh the join skipped. A hit on one is not a selection.
        let mut world = World::new();
        let root = world.spawn_empty().id();
        let child = world.spawn(ChildOf(root)).id();

        assert_eq!(element_for(&mut world, child), None);
    }
}
