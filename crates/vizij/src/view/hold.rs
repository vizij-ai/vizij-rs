//! Rig outputs held against the device — the authoring inspector's lock.
//!
//! A held animatable keeps what the view shows while its device writes on;
//! a held component keeps its value while the animatable's other components
//! follow the device. Components are `x`, `y`, `z` of a vector or an euler,
//! or `r`, `g`, `b` of a colour (`r` is `x`, `g` is `y`, `b` is `z`), and
//! hold nothing on a scalar output. A hold is the view declining the device's
//! writes: the device and its store run on untouched, so a release shows the
//! device's current value at once.
//!
//! Holds are kept per face id, across the face's reloads: what a held
//! component keeps is what the scene shows, and a reloaded scene shows its
//! GLB's values until the device writes.

use std::collections::HashMap;
use std::str::FromStr;

use bevy::prelude::Resource;
use uuid::Uuid;

use super::Decoded;

/// One held output: an animatable, or one component of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HoldTarget {
    pub animatable: Uuid,
    /// The component held: 0 (`x`/`r`), 1 (`y`/`g`) or 2 (`z`/`b`); `None`
    /// holds the whole output.
    pub component: Option<usize>,
}

impl FromStr for HoldTarget {
    type Err = String;

    /// `<animatable id>` or `<animatable id>:<component>`, the id bare or as
    /// the rig path it is written under (`rig/<faceId>/<id>`).
    fn from_str(path: &str) -> Result<Self, Self::Err> {
        let last = path.rsplit('/').next().unwrap_or(path);
        let (id, component) = match last.split_once(':') {
            Some((id, component)) => (id, Some(component)),
            None => (last, None),
        };
        let animatable =
            Uuid::parse_str(id.trim()).map_err(|_| format!("{path:?} names no animatable id"))?;
        let component = match component {
            None => None,
            Some("x" | "r") => Some(0),
            Some("y" | "g") => Some(1),
            Some("z" | "b") => Some(2),
            Some(other) => {
                return Err(format!(
                    "{path:?}: {other:?} is no component (x, y, z, r, g or b)"
                ))
            }
        };
        Ok(Self {
            animatable,
            component,
        })
    }
}

/// What is held of one animatable: the whole output, and each component on
/// its own — held and released independently, as the inspector locks them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Held {
    whole: bool,
    components: [bool; 3],
}

impl Held {
    fn is_empty(&self) -> bool {
        !self.whole && !self.components.contains(&true)
    }
}

/// The holds of every face, by face id.
#[derive(Resource, Default)]
pub(crate) struct Holds(HashMap<String, HashMap<Uuid, Held>>);

impl Holds {
    pub(crate) fn hold(&mut self, face_id: &str, targets: Vec<HoldTarget>) {
        let face = self.0.entry(face_id.to_string()).or_default();
        for target in targets {
            let held = face.entry(target.animatable).or_default();
            match target.component {
                Some(i) => held.components[i] = true,
                None => held.whole = true,
            }
        }
    }

    /// Release `targets`, or every hold of the face given `None`.
    pub(crate) fn release(&mut self, face_id: &str, targets: Option<Vec<HoldTarget>>) {
        let Some(targets) = targets else {
            self.0.remove(face_id);
            return;
        };
        let Some(face) = self.0.get_mut(face_id) else {
            return;
        };
        for target in targets {
            if let Some(held) = face.get_mut(&target.animatable) {
                match target.component {
                    Some(i) => held.components[i] = false,
                    None => held.whole = false,
                }
                if held.is_empty() {
                    face.remove(&target.animatable);
                }
            }
        }
        if face.is_empty() {
            self.0.remove(face_id);
        }
    }

    pub(crate) fn clear(&mut self, face_id: &str) {
        self.0.remove(face_id);
    }

    /// The face's holds, `None` when it holds nothing.
    pub(crate) fn of(&self, face_id: &str) -> Option<&HashMap<Uuid, Held>> {
        self.0.get(face_id)
    }
}

/// What the scene takes of the device's `value` for animatable `id` under
/// the face's holds: the value itself when nothing of it is held, `None`
/// when it is held whole, and, with components held, the value with those
/// components taken from what the scene `shown`s — or `None` when the scene
/// shows nothing to take them from.
pub(crate) fn resolve(
    holds: &HashMap<Uuid, Held>,
    id: Uuid,
    value: Decoded,
    shown: impl FnOnce() -> Option<Decoded>,
) -> Option<Decoded> {
    let Some(held) = holds.get(&id) else {
        return Some(value);
    };
    if held.whole {
        return None;
    }
    match value {
        Decoded::Scalar(_) => Some(value),
        Decoded::Triple(mut v) => {
            let Some(Decoded::Triple(kept)) = shown() else {
                return None;
            };
            for (i, held) in held.components.iter().enumerate() {
                if *held {
                    v[i] = kept[i];
                }
            }
            Some(Decoded::Triple(v))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "6b8f0c1e-3f6a-4c55-9d1e-2b7a4c0e9f11";

    fn target(path: &str) -> HoldTarget {
        path.parse().expect("a hold target")
    }

    #[test]
    fn a_target_names_an_animatable_and_maybe_a_component() {
        let id = Uuid::parse_str(ID).unwrap();
        assert_eq!(
            target(ID),
            HoldTarget {
                animatable: id,
                component: None
            }
        );
        assert_eq!(target(&format!("{ID}:x")).component, Some(0));
        assert_eq!(target(&format!("{ID}:g")).component, Some(1));
        assert_eq!(target(&format!("{ID}:b")).component, Some(2));
        let rig_path = target(&format!("rig/quori_latest/{ID}:z"));
        assert_eq!((rig_path.animatable, rig_path.component), (id, Some(2)));
        assert!("not-an-id".parse::<HoldTarget>().is_err());
        assert!(format!("{ID}:w").parse::<HoldTarget>().is_err());
    }

    #[test]
    fn a_whole_hold_declines_every_write_and_releases() {
        let id = Uuid::parse_str(ID).unwrap();
        let mut holds = Holds::default();
        holds.hold("face", vec![target(ID)]);
        let face = holds.of("face").unwrap();
        assert_eq!(
            resolve(face, id, Decoded::Scalar(0.5), || None),
            None,
            "a held scalar"
        );
        assert_eq!(
            resolve(face, id, Decoded::Triple([1.0, 2.0, 3.0]), || None),
            None
        );
        holds.release("face", Some(vec![target(ID)]));
        assert!(holds.of("face").is_none(), "nothing left held");
    }

    #[test]
    fn a_held_component_keeps_what_the_scene_shows() {
        let id = Uuid::parse_str(ID).unwrap();
        let mut holds = Holds::default();
        holds.hold("face", vec![target(&format!("{ID}:y"))]);
        let face = holds.of("face").unwrap();
        let shown = || Some(Decoded::Triple([0.0, 9.0, 0.0]));
        assert_eq!(
            resolve(face, id, Decoded::Triple([1.0, 2.0, 3.0]), shown),
            Some(Decoded::Triple([1.0, 9.0, 3.0]))
        );
        // Nothing shown to keep it from: the write is declined whole.
        assert_eq!(
            resolve(face, id, Decoded::Triple([1.0, 2.0, 3.0]), || None),
            None
        );
        // A component holds nothing on a scalar output.
        assert_eq!(
            resolve(face, id, Decoded::Scalar(0.25), shown),
            Some(Decoded::Scalar(0.25))
        );
        // Another animatable is free.
        assert_eq!(
            resolve(face, Uuid::nil(), Decoded::Scalar(0.25), shown),
            Some(Decoded::Scalar(0.25))
        );
    }

    /// The whole output and its components are held and released on their
    /// own, as the inspector's locks are.
    #[test]
    fn whole_and_component_holds_are_independent() {
        let id = Uuid::parse_str(ID).unwrap();
        let mut holds = Holds::default();
        holds.hold("face", vec![target(ID), target(&format!("{ID}:x"))]);
        holds.release("face", Some(vec![target(ID)]));
        let face = holds.of("face").expect("the component is still held");
        assert_eq!(
            resolve(face, id, Decoded::Triple([1.0, 2.0, 3.0]), || Some(
                Decoded::Triple([7.0, 7.0, 7.0])
            )),
            Some(Decoded::Triple([7.0, 2.0, 3.0]))
        );
        holds.release("face", None);
        assert!(holds.of("face").is_none());
    }
}
