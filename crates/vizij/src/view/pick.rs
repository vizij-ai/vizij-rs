//! Picks: a pointer press on a face, resolved to the element under it or to
//! a miss.
//!
//! The ray is cast here, on the press, over each element's mesh as it is
//! drawn — its morph targets applied at their current weights, in world
//! space — as three.js casts it. A cast over the base vertices (Bevy's mesh
//! picking) misses an element a morph has moved, an opened mouth, and lands
//! on the element behind it. Bevy's picking only delivers the press: with no
//! mesh backend, every press reaches the window entity.
//!
//! The press is resolved against the faces whose rectangle holds the
//! pointer where it is drawn, the one drawn last first: the nearest element
//! its ray meets is the pick, and a press that meets no element of any of
//! them is a miss of the topmost. A press outside every face's drawn
//! rectangle is no face's.

use bevy::ecs::system::SystemParam;
use bevy::math::Affine3A;
use bevy::mesh::morph::{MorphAttributes, MorphWeights};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::picking::events::{Pointer, Press};
use bevy::picking::mesh_picking::ray_cast::{ray_mesh_intersection, Backfaces};
use bevy::prelude::*;

use super::{ElementTargets, Face, Picked, Picks, Superseded};

/// Resolve a press to a pick or a miss ([module docs](self)).
pub(super) fn on_press(
    press: On<Pointer<Press>>,
    faces: Query<&Face, Without<Superseded>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    elements: ElementMeshes,
    mut picks: ResMut<Picks>,
) {
    // The press bubbles up from its target; resolve it once.
    if press.entity != press.original_event_target() {
        return;
    }
    let position = press.pointer_location.position;
    let mut under: Vec<(&Face, &Camera, &GlobalTransform)> = faces
        .iter()
        .filter(|face| face.bindings.ready)
        .filter_map(|face| {
            let (camera, transform) = cameras.get(face.camera).ok()?;
            camera
                .is_active
                .then(|| camera.logical_viewport_rect())
                .flatten()?
                .contains(position)
                .then_some((face, camera, transform))
        })
        .collect();
    under.sort_by_key(|(_, camera, _)| std::cmp::Reverse(camera.order));
    for (face, camera, transform) in &under {
        let Ok(ray) = camera.viewport_to_world(transform, position) else {
            continue;
        };
        let nearest = face
            .bindings
            .elements
            .iter()
            .filter_map(|(id, targets)| Some((*id, elements.drawn(targets)?.hit(ray)?)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((element_id, _)) = nearest {
            log::info!("face {}: picked element {element_id}", face.id);
            picks.0.push(Picked {
                face_id: face.id.clone(),
                element_id: Some(element_id),
            });
            return;
        }
    }
    if let Some((face, _, _)) = under.first() {
        log::info!("face {}: a press on no element", face.id);
        picks.0.push(Picked {
            face_id: face.id.clone(),
            element_id: None,
        });
    }
}

/// The element meshes as drawn: what a pick casts over and the glow traces.
#[derive(SystemParam)]
pub(super) struct ElementMeshes<'w, 's> {
    meshes: Res<'w, Assets<Mesh>>,
    handles: Query<'w, 's, &'static Mesh3d>,
    transforms: Query<'w, 's, &'static GlobalTransform>,
    weights: Query<'w, 's, &'static MorphWeights>,
}

impl ElementMeshes<'_, '_> {
    /// The element's own mesh asset, `None` for a group or a mesh that is
    /// not a triangle list.
    pub(super) fn mesh(&self, targets: &ElementTargets) -> Option<(AssetId<Mesh>, &Mesh)> {
        let handle = &self.handles.get(targets.mesh?).ok()?.0;
        let mesh = self.meshes.get(handle)?;
        (mesh.primitive_topology() == PrimitiveTopology::TriangleList)
            .then_some((handle.id(), mesh))
    }

    /// The element's mesh as it is drawn this frame: its vertices with the
    /// morph targets applied at their current weights, in world space.
    pub(super) fn drawn(&self, targets: &ElementTargets) -> Option<DrawnMesh<'_>> {
        let (_, mesh) = self.mesh(targets)?;
        let base = mesh
            .try_attribute(Mesh::ATTRIBUTE_POSITION)
            .ok()?
            .as_float3()?;
        let weights = targets
            .morph
            .and_then(|entity| self.weights.get(entity).ok())
            .map(MorphWeights::weights)
            .unwrap_or_default();
        let deltas = mesh.try_morph_targets().ok().map(Vec::as_slice);
        let world = self.transforms.get(targets.mesh?).ok()?.affine();
        let positions = morphed(base, deltas, weights)
            .into_iter()
            .map(|p| world.transform_point3(Vec3::from(p)).to_array())
            .collect();
        Some(DrawnMesh {
            positions,
            indices: mesh.try_indices().ok(),
        })
    }
}

/// A mesh's triangles as drawn, in world space.
pub(super) struct DrawnMesh<'a> {
    pub(super) positions: Vec<[f32; 3]>,
    pub(super) indices: Option<&'a Indices>,
}

impl DrawnMesh<'_> {
    /// The distance along `ray` to the nearest triangle it meets, either
    /// side (the faces' materials are double-sided).
    fn hit(&self, ray: Ray3d) -> Option<f32> {
        let world = Affine3A::IDENTITY;
        let cull = Backfaces::Include;
        let positions = &self.positions;
        match self.indices {
            Some(Indices::U16(i)) => {
                ray_mesh_intersection(ray, &world, positions, None, Some(i), None, cull)
            }
            Some(Indices::U32(i)) => {
                ray_mesh_intersection(ray, &world, positions, None, Some(i), None, cull)
            }
            None => ray_mesh_intersection::<u32>(ray, &world, positions, None, None, None, cull),
        }
        .map(|hit| hit.distance)
    }
}

/// `base` positions moved by each morph target at its weight. `deltas` holds
/// the targets one after the other, `base.len()` displacements each, as
/// Bevy's glTF loader lays them out.
pub(super) fn morphed(
    base: &[[f32; 3]],
    deltas: Option<&[MorphAttributes]>,
    weights: &[f32],
) -> Vec<[f32; 3]> {
    let mut positions = base.to_vec();
    let (Some(deltas), n) = (deltas, base.len()) else {
        return positions;
    };
    for (target, weight) in weights.iter().enumerate() {
        if *weight == 0.0 {
            continue;
        }
        let Some(target) = deltas.get(target * n..(target + 1) * n) else {
            break;
        };
        for (position, delta) in positions.iter_mut().zip(target) {
            position[0] += weight * delta.position.x;
            position[1] += weight * delta.position.y;
            position[2] += weight * delta.position.z;
        }
    }
    positions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delta(x: f32, y: f32) -> MorphAttributes {
        MorphAttributes::new(Vec3::new(x, y, 0.0), Vec3::ZERO, Vec3::ZERO)
    }

    #[test]
    fn morph_targets_move_the_vertices_by_their_weights() {
        let base = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]];
        // Two targets: the first moves the second vertex up, the second
        // moves the first vertex right.
        let deltas = [
            delta(0.0, 0.0),
            delta(0.0, 2.0),
            delta(1.0, 0.0),
            delta(0.0, 0.0),
        ];
        assert_eq!(morphed(&base, Some(&deltas), &[]), base.to_vec());
        assert_eq!(
            morphed(&base, Some(&deltas), &[0.5, 1.0]),
            vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0]]
        );
        assert_eq!(morphed(&base, None, &[1.0]), base.to_vec());
    }

    /// A thin mesh a morph opens is hit where it is drawn open, and missed
    /// where its base vertices lay.
    #[test]
    fn a_pick_meets_the_mesh_where_its_morphs_put_it() {
        // A sliver at y ∈ [0, 0.01] whose morph opens it to y ∈ [0, 1].
        let base = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.01, 0.0],
            [0.0, 0.01, 0.0],
        ];
        let deltas = [
            delta(0.0, 0.0),
            delta(0.0, 0.0),
            delta(0.0, 0.99),
            delta(0.0, 0.99),
        ];
        let indices = Indices::U16(vec![0, 1, 2, 0, 2, 3]);
        let ray = Ray3d::new(Vec3::new(0.5, 0.5, 10.0), Dir3::NEG_Z);
        let drawn = |weight: f32| DrawnMesh {
            positions: morphed(&base, Some(&deltas), &[weight]),
            indices: Some(&indices),
        };
        assert_eq!(
            drawn(0.0).hit(ray),
            None,
            "the base sliver is below the ray"
        );
        assert_eq!(drawn(1.0).hit(ray), Some(10.0), "the opened mesh is hit");
    }
}
