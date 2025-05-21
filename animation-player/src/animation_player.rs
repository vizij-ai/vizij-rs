use std::collections::HashMap;
use std::sync::Mutex;

use lazy_static::lazy_static;
use uuid::Uuid;

use crate::animation_data::AnimationData;

// Lazy static initialization of a mutex-wrapped HashMap
lazy_static! {
    pub(crate) static ref ANIMATIONS: Mutex<HashMap<Uuid, AnimationData>> =
        Mutex::new(HashMap::new());
}

/// Loads an animation directly (for native Rust usage)
pub fn load_animation(animation_data: AnimationData) -> Uuid {
    // Generate a UUID for this animation
    let id = Uuid::new_v4();

    // Store the animation in our HashMap
    let mut animations = ANIMATIONS.lock().expect("Failed to lock animations mutex");
    animations.insert(id, animation_data);

    id
}

/// Retrieves an animation by UUID
/// Returns None if the animation is not found
pub fn get_animation(id: &Uuid) -> Option<AnimationData> {
    let animations = ANIMATIONS.lock().expect("Failed to lock animations mutex");
    animations.get(id).cloned()
}

/// Unloads/removes an animation from the static HashMap by UUID (for native Rust usage)
///
/// Returns true if the animation was successfully removed, false otherwise.
pub fn unload_animation(id: &Uuid) -> bool {
    let mut animations = ANIMATIONS.lock().expect("Failed to lock animations mutex");
    animations.remove(id).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animation_data::AnimationTransition;
    use std::collections::HashMap;

    #[test]
    fn test_load_animation_direct() {
        // Create a test animation
        let mut transitions = HashMap::new();
        transitions.insert(
            "transition1".to_string(),
            AnimationTransition {
                id: "transition1".to_string(),
                keypoints: ("key1".to_string(), "key2".to_string()),
                variant: "linear".to_string(),
                parameters: HashMap::new(),
            },
        );

        let animation = AnimationData {
            id: "test-anim".to_string(),
            name: "Test Animation".to_string(),
            tracks: vec![],
            transitions,
            duration: 5.0,
        };

        // Call the direct load function (no WASM involved)
        let uuid = load_animation(animation.clone());

        // Check that the animation was stored correctly
        let stored_animation = get_animation(&uuid).expect("Animation should be stored");
        assert_eq!(stored_animation.name, "Test Animation");
        assert_eq!(stored_animation.duration, 5.0);
        assert_eq!(stored_animation.id, "test-anim");
    }

    #[test]
    fn test_unloading_animation() {
        // Create a test animation
        let mut transitions = HashMap::new();
        transitions.insert(
            "transition1".to_string(),
            AnimationTransition {
                id: "transition1".to_string(),
                keypoints: ("key1".to_string(), "key2".to_string()),
                variant: "linear".to_string(),
                parameters: HashMap::new(),
            },
        );

        let animation = AnimationData {
            id: "test-anim-unload-direct".to_string(),
            name: "Test Animation".to_string(),
            tracks: vec![],
            transitions,
            duration: 5.0,
        };

        // Load the animation and get the UUID
        let id = load_animation(animation.clone());

        // Verify the animation was loaded
        let loaded_animation = get_animation(&id);
        assert!(loaded_animation.is_some());

        // Unload the animation
        let result = unload_animation(&id);
        assert!(result, "Animation should be successfully unloaded");

        // Verify the animation was unloaded
        let loaded_animation_after_unload = get_animation(&id);
        assert!(
            loaded_animation_after_unload.is_none(),
            "Animation should be removed after unloading"
        );

        // Try to unload a non-existent animation
        let random_id = Uuid::new_v4();
        let result = unload_animation(&random_id);
        assert!(
            !result,
            "Unloading a non-existent animation should return false"
        );
    }
}
