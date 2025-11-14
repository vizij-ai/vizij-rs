use arora_schema::keyvalue::{KeyValue, KeyValueField};
use arora_schema::value::Value;
use arora_schema::{gen_bb_uuid, gen_uuid_from_str};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use vizij_blackboard_core::PATH_SEPARATOR;
use vizij_blackboard_core::{
    arc_bb::{ArcBBNode, ArcBBPathNodeTrait, ArcNamespacedSetterTrait},
    arora_mem_space::{AMSNodeAccess, AroraMemSpace, AroraMemSpaceInterface, AroraMemSpaceType},
    traits::BBNodeTrait,
    ArcBlackboard,
};

/// Helper function to join path segments using PATH_SEPARATOR
/// Takes an inline list of path segments and returns a properly joined path
///
/// # Example
/// ```
/// let path = path(&["entity", "transform", "position", "x"]);
/// // Returns "entity.transform.position.x" (or uses the actual PATH_SEPARATOR)
/// ```
fn path(segments: &[&str]) -> String {
    segments.join(&PATH_SEPARATOR.to_string())
}

// Macro to generate tests for both ArcBlackboard and RcBlackboard
macro_rules! test_both_blackboards {
    ($test_name:ident, $test_impl:ident) => {
        #[test]
        fn $test_name() {
            $test_impl(AroraMemSpaceType::Arc);
        }

        paste::paste! {
            #[test]
            fn [<$test_name _arora>]() {
                $test_impl(AroraMemSpaceType::Rc);
            }
        }
    };
}

// ============================================================================
// New helper functions that work with AroraMemSpaceType
// ============================================================================

fn validate_item_ref<S: ToString + ?Sized>(
    bb: &AroraMemSpace,
    path: &S,
    expected_value: &Value,
    expected_id: &Uuid,
) {
    // Verify the value can be retrieved by path
    let value = bb.lookup(path);
    assert!(
        value.is_some(),
        "Value at path '{}' should exist",
        path.to_string()
    );
    assert_eq!(
        value.as_ref().unwrap(),
        expected_value,
        "Value mismatch at path '{}'",
        path.to_string()
    );

    // Verify the value can be retrieved by ID
    let value_by_id = bb.lookup_by_id(expected_id);
    assert!(
        value_by_id.is_some(),
        "Value with ID {:?} should exist",
        expected_id
    );
    assert_eq!(
        value_by_id.as_ref().unwrap(),
        expected_value,
        "Value mismatch for ID {:?}",
        expected_id
    );
}

fn assert_path_exists_ref<S: ToString + ?Sized>(bb: &AroraMemSpace, path: &S) {
    let value = bb.lookup(path);
    assert!(value.is_some(), "Path '{}' should exist", path.to_string());
}

fn assert_path_not_exists_ref<S: ToString + ?Sized>(bb: &AroraMemSpace, path: &S) {
    let value = bb.lookup(path);
    assert!(
        value.is_none(),
        "Path '{}' should not exist",
        path.to_string()
    );
}

fn contains_ref<S: ToString + ?Sized>(bb: &AroraMemSpace, path: &S) -> bool {
    bb.lookup(path).is_some()
}

fn unwrap_node_result(
    node: Result<Option<Arc<Mutex<ArcBBNode>>>, String>,
) -> Option<Arc<Mutex<ArcBBNode>>> {
    node.unwrap_or_default()
}

fn assert_node_exists(node: Result<Option<Arc<Mutex<ArcBBNode>>>, String>) {
    assert!(node.is_ok(), "Node result should be Ok");
    let unwrapped = node.unwrap();
    assert!(unwrapped.is_some(), "Node should be Some");
}

fn get_id_safe<T, E>(id_result: Result<T, E>) -> T
where
    E: std::fmt::Display,
{
    match id_result {
        Ok(id) => id,
        Err(e) => {
            println!("Error getting ID: {}", e);
            panic!("Failed to get ID: {}", e)
        }
    }
}

fn get_name_ref_safe(name_result: Result<String, String>) -> String {
    name_result.expect("Failed to get name reference")
}

// Single test implementation that works with both blackboard types via AroraMemSpaceType
fn test_bb_creation_impl(bb_type: AroraMemSpaceType) {
    let bb = AroraMemSpace::new(bb_type, "root");
    let name = bb.get_name();
    assert!(name.is_ok(), "Should be able to get blackboard name");
}

// Generate both test cases using the macro
test_both_blackboards!(test_bb_creation, test_bb_creation_impl);

fn test_add_simple_value_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add a simple value
    let value = Value::I32(42);
    let name = "test_var";
    let id = bb.set(name, value.clone()).unwrap();

    // Validate the item can be retrieved by path and by id
    validate_item_ref(&bb, name, &value, &id);

    // Verify it exists in the root namespace
    assert!(contains_ref(&bb, name));
}

test_both_blackboards!(test_add_simple_value, test_add_simple_value_impl);

fn test_single_level_namespace_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add a value in a single-level namespace
    let math_name = "math";
    let pi_name = "pi";
    let pi_value = Value::F32(std::f32::consts::PI);
    let full_path = path(&[math_name, pi_name]);
    let pi_id = bb.set(&full_path, pi_value.clone()).unwrap();

    // Verify the math namespace path exists
    assert_path_exists_ref(&bb, math_name);

    // Verify the full path exists and corresponds to the pi node
    validate_item_ref(&bb, &full_path, &pi_value, &pi_id);

    // Verify the pi node exists in the math namespace
    assert!(contains_ref(&bb, &full_path));
}

test_both_blackboards!(
    test_single_level_namespace,
    test_single_level_namespace_impl
);

fn test_multi_level_namespace_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add values in multi-level namespaces
    let pos_x = Value::F32(10.0);
    let pos_y = Value::F32(20.0);
    let pos_z = Value::F32(30.0);

    let x_id = bb
        .set(
            &path(&["entity", "transform", "position", "x"]),
            pos_x.clone(),
        )
        .unwrap();
    let y_id = bb
        .set(
            &path(&["entity", "transform", "position", "y"]),
            pos_y.clone(),
        )
        .unwrap();

    let z_id = bb
        .set(
            &path(&["entity", "transform", "position", "z"]),
            pos_z.clone(),
        )
        .unwrap();

    // Verify intermediate namespaces exist
    assert_path_exists_ref(&bb, "entity");
    assert_path_exists_ref(&bb, &path(&["entity", "transform"]));
    assert_path_exists_ref(&bb, &path(&["entity", "transform", "position"]));

    // Check values by full path
    validate_item_ref(
        &bb,
        &path(&["entity", "transform", "position", "x"]),
        &pos_x,
        &x_id,
    );
    validate_item_ref(
        &bb,
        &path(&["entity", "transform", "position", "y"]),
        &pos_y,
        &y_id,
    );
    validate_item_ref(
        &bb,
        &path(&["entity", "transform", "position", "z"]),
        &pos_z,
        &z_id,
    );
}

test_both_blackboards!(test_multi_level_namespace, test_multi_level_namespace_impl);

fn test_namespaces_with_multiple_values_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add multiple values to the same namespace
    let hp = Value::I32(100);
    let mp = Value::I32(50);
    let speed = Value::F32(5.5);

    let hp_id = bb
        .set(&path(&["player", "stats", "hp"]), hp.clone())
        .unwrap();
    let mp_id = bb
        .set(&path(&["player", "stats", "mp"]), mp.clone())
        .unwrap();
    let speed_id = bb
        .set(&path(&["player", "stats", "speed"]), speed.clone())
        .unwrap();

    // Verify the namespace exists
    assert_path_exists_ref(&bb, "player");

    // Verify the ids correspond to the expected values
    validate_item_ref(&bb, &path(&["player", "stats", "hp"]), &hp, &hp_id);
    validate_item_ref(&bb, &path(&["player", "stats", "mp"]), &mp, &mp_id);
    validate_item_ref(&bb, &path(&["player", "stats", "speed"]), &speed, &speed_id);
}

test_both_blackboards!(
    test_namespaces_with_multiple_values,
    test_namespaces_with_multiple_values_impl
);

fn test_custom_ids_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add a value with custom ID
    let value = Value::String("test_value".to_string());
    let custom_id = gen_bb_uuid();
    let custom_name = "custom_item";

    let returned_custom_id = bb
        .set_with_id(custom_name, value.clone(), &custom_id)
        .unwrap();
    assert_eq!(returned_custom_id, custom_id);

    // Verify we can get the node by name and by ID
    validate_item_ref(&bb, custom_name, &value, &custom_id);
}

test_both_blackboards!(test_custom_ids, test_custom_ids_impl);

fn test_non_existent_paths_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add some values
    bb.set(&path(&["a", "b", "c"]), Value::I32(1)).unwrap();

    // Test non-existent paths
    assert_path_not_exists_ref(&bb, "");
    assert_path_not_exists_ref(&bb, "x");
    assert_path_not_exists_ref(&bb, &path(&["a", "x"]));
    assert_path_not_exists_ref(&bb, &path(&["a", "b", "x"]));
    assert_path_not_exists_ref(&bb, &path(&["a", "b", "c", "d"]));
}

test_both_blackboards!(test_non_existent_paths, test_non_existent_paths_impl);

fn test_complex_values_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Test with more complex values
    let bool_value = Value::Boolean(true);
    let string_value = Value::String("Hello, World!".to_string());
    let array_value = Value::ArrayI32(vec![1, 2, 3]);

    let bool_id = bb
        .set(&path(&["values", "bool"]), bool_value.clone())
        .unwrap();
    let string_id = bb
        .set(&path(&["values", "string"]), string_value.clone())
        .unwrap();
    let array_id = bb
        .set(&path(&["values", "array"]), array_value.clone())
        .unwrap();

    // Get and validate the values
    validate_item_ref(&bb, &path(&["values", "bool"]), &bool_value, &bool_id);
    validate_item_ref(&bb, &path(&["values", "string"]), &string_value, &string_id);
    validate_item_ref(&bb, &path(&["values", "array"]), &array_value, &array_id);
}

test_both_blackboards!(test_complex_values, test_complex_values_impl);

fn test_empty_name_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");
    bb.set("", Value::I32(123)).unwrap();
}

#[test]
#[should_panic(expected = "Path cannot be empty when setting an item to the blackboard")]
fn test_empty_name() {
    test_empty_name_impl(AroraMemSpaceType::Arc);
}

#[test]
#[should_panic(expected = "Path cannot be empty when setting an item to the blackboard")]
fn test_empty_name_arora() {
    test_empty_name_impl(AroraMemSpaceType::Rc);
}

fn test_namespace_node_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add some values to create namespaces
    bb.set(&path(&["system", "config", "debug"]), Value::Boolean(true))
        .unwrap();
    bb.set(&path(&["system", "config", "log_level"]), Value::I32(3))
        .unwrap();

    // Verify the namespace paths exist
    assert_path_exists_ref(&bb, &path(&["system", "config"]));
    assert!(contains_ref(&bb, &path(&["system", "config", "debug"])));
    assert!(contains_ref(&bb, &path(&["system", "config", "log_level"])));
}

test_both_blackboards!(test_namespace_node, test_namespace_node_impl);

fn test_overwrite_existing_item_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Add a value
    let id1 = bb
        .set(&path(&["player", "health"]), Value::I32(100))
        .unwrap();

    // Verify initial value
    validate_item_ref(&bb, &path(&["player", "health"]), &Value::I32(100), &id1);

    // Overwrite with new value
    let id2 = bb
        .set(&path(&["player", "health"]), Value::I32(150))
        .unwrap();

    // IDs should be the same when overwriting
    assert_eq!(id1, id2);

    // Verify updated value
    validate_item_ref(&bb, &path(&["player", "health"]), &Value::I32(150), &id1);
}

test_both_blackboards!(
    test_overwrite_existing_item,
    test_overwrite_existing_item_impl
);

#[test]
fn test_remove_item() {
    fn test_remove_impl(bb_type: AroraMemSpaceType) {
        let mut bb = AroraMemSpace::new(bb_type, "root");

        // 1. Test removing a root level item
        bb.set(&path(&["name"]), Value::String("Alice".to_string()))
            .unwrap();
        assert!(bb.lookup(&path(&["name"])).is_some());

        bb.remove(&path(&["name"])).unwrap();
        assert!(
            bb.lookup(&path(&["name"])).is_none(),
            "Item should be removed from blackboard"
        );

        // 2. Test removing a nested item
        bb.set(&path(&["player", "health"]), Value::I32(100))
            .unwrap();
        bb.set(&path(&["player", "mana"]), Value::I32(50)).unwrap();

        assert!(bb.lookup(&path(&["player", "health"])).is_some());
        assert!(bb.lookup(&path(&["player", "mana"])).is_some());

        bb.remove(&path(&["player", "health"])).unwrap();
        assert!(
            bb.lookup(&path(&["player", "health"])).is_none(),
            "Nested item should be removed"
        );
        assert!(
            bb.lookup(&path(&["player", "mana"])).is_some(),
            "Other items in same path should remain"
        );

        // 3. Test removing by ID
        let id = bb.set(&path(&["score"]), Value::I32(999)).unwrap();
        assert!(bb.lookup_by_id(&id).is_some());

        bb.remove_by_id(&id).unwrap();
        assert!(
            bb.lookup_by_id(&id).is_none(),
            "Item should be removed by ID"
        );

        // 4. Test attempting to remove a non-existent item
        let result = bb.remove(&path(&["nonexistent"]));
        assert!(
            result.is_err(),
            "Removing non-existent item should return error"
        );

        // 5. Test removing a property tree and verify returned IDs
        let sav_id = bb
            .set(&path(&["settings", "audio", "volume"]), Value::I32(75))
            .unwrap();
        let svr_id = bb
            .set(
                &path(&["settings", "video", "resolution"]),
                Value::String("1920x1080".to_string()),
            )
            .unwrap();
        assert!(bb.lookup(&path(&["settings", "audio", "volume"])).is_some());
        assert!(bb
            .lookup(&path(&["settings", "video", "resolution"]))
            .is_some());

        // Remove the settings tree and capture all removed IDs
        let removed_ids = bb.remove(&path(&["settings"])).unwrap();

        // Verify the tree was removed
        assert!(
            bb.lookup(&path(&["settings", "audio", "volume"])).is_none(),
            "Audio settings should be removed with parent"
        );
        assert!(
            bb.lookup(&path(&["settings", "video", "resolution"]))
                .is_none(),
            "Video settings should be removed with parent"
        );
        assert!(
            bb.lookup_by_id(&sav_id).is_none(),
            "Audio volume item should be removed by ID"
        );
        assert!(
            bb.lookup_by_id(&svr_id).is_none(),
            "Video resolution item should be removed by ID"
        );

        // Verify all removed IDs are returned
        // Should include: settings, audio, video, volume, resolution (5 nodes total)
        assert_eq!(removed_ids.len(), 5, "Should have removed 5 nodes");
        assert!(
            removed_ids.contains(&sav_id),
            "Removed IDs should contain audio volume ID"
        );
        assert!(
            removed_ids.contains(&svr_id),
            "Removed IDs should contain video resolution ID"
        );
    }

    // Test both Rc and Arc implementations
    test_remove_impl(AroraMemSpaceType::Rc);
    test_remove_impl(AroraMemSpaceType::Arc);
}

fn test_path_conflict_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Create a namespace node
    bb.set(&path(&["player", "inventory", "gold"]), Value::I32(100))
        .unwrap();

    // Now try to use the namespace as a value path (should fail)
    let result = bb.set(
        &path(&["player", "inventory"]),
        Value::String("this should fail".to_string()),
    );
    assert!(
        result.is_err(),
        "Expected an error when trying to set a value at an existing path node"
    );
    let error_message = result.unwrap_err();
    let expected_path = path(&["player", "inventory"]);
    assert!(
        error_message.contains(&format!(
            "Path {} already exists as a BBPath node, cannot set it with a Value",
            expected_path
        )),
        "Error message should contain expected text, but got: {}",
        error_message
    );
}

test_both_blackboards!(test_path_conflict, test_path_conflict_impl);

#[test]
fn test_keyvalue_structure() {
    let mut bb = ArcBlackboard::new("root".to_string());

    let player_name = "player".to_string();
    let health_name = "health".to_string();
    let stats_name = "stats".to_string();
    let strength_name = "strength".to_string();
    let agility_name = "agility".to_string();

    let player_id = gen_uuid_from_str(&player_name);
    let stats_id = gen_uuid_from_str(&stats_name);
    let health_id = gen_uuid_from_str(&health_name);
    let strength_id = gen_uuid_from_str(&strength_name);
    let agility_id = gen_uuid_from_str(&agility_name);

    let player_kv: KeyValue = (
        player_id,
        [
            KeyValueField::new_with_id(health_name.clone(), health_id, Value::I32(100)),
            KeyValueField::new_nested_kv(
                "stats",
                &[
                    KeyValueField::new_with_id("strength", strength_id, Value::I32(50)),
                    KeyValueField::new_with_id("agility", agility_id, Value::I32(75)),
                ],
            ),
        ],
    )
        .into();

    // ## TEST 1 ##
    // Set the KeyValue structure
    let result = bb.set(&"player".to_string(), player_kv.into()).unwrap();
    assert!(!result.is_nil());

    // Check that values were set correctly
    let health_node = bb.get(&path(&["player", "health"]));
    assert_node_exists(health_node.clone());
    let strength_node = bb.get(&path(&["player", "stats", "strength"]));
    assert_node_exists(strength_node.clone());
    let agility_node = bb.get(&path(&["player", "stats", "agility"]));
    assert_node_exists(agility_node.clone());

    // Verify content
    let health_node_unwrapped = unwrap_node_result(health_node).expect("Health node should exist");
    match health_node_unwrapped.lock().unwrap().as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::I32(100)));
            assert_eq!(get_id_safe(item.get_id_ref()), &health_id);
        }
        None => panic!("Expected Item node for health"),
    }

    let strength_node_unwrapped =
        unwrap_node_result(strength_node).expect("Strength node should exist");
    match strength_node_unwrapped.lock().unwrap().as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::I32(50)));
            assert_eq!(get_id_safe(item.get_id_ref()), &strength_id);
        }
        None => panic!("Expected Item node for strength"),
    }

    let agility_node_unwrapped =
        unwrap_node_result(agility_node).expect("Agility node should exist");
    match agility_node_unwrapped.lock().unwrap().as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::I32(75)));
            assert_eq!(get_id_safe(item.get_id_ref()), &agility_id);
        }
        None => panic!("Expected Item node for agility"),
    }

    // ## TEST 2 ##
    // Try setting a keyvalue into an intermediate node by its path
    let mut new_stats_kv: KeyValue = (
        stats_id,
        [
            KeyValueField::new_with_id("strength", strength_id, Value::I32(100)),
            KeyValueField::new_with_id("agility", agility_id, Value::I32(100)),
        ],
    )
        .into();
    // Update the stats KeyValue structure
    let result = bb
        .set(&path(&["player", "stats"]), new_stats_kv.clone().into())
        .unwrap();

    assert!(!result.is_nil());

    // Check that values were set correctly
    let updated_strength_node = bb.get(&path(&["player", "stats", "strength"]));
    assert_node_exists(updated_strength_node.clone());

    let updated_agility_node = bb.get(&path(&["player", "stats", "agility"]));
    assert_node_exists(updated_agility_node.clone());

    // Verify content
    let updated_strength_node_unwrapped =
        unwrap_node_result(updated_strength_node).expect("Updated strength node should exist");
    match updated_strength_node_unwrapped.lock().unwrap().as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::I32(100)));
            assert_eq!(get_id_safe(item.get_id_ref()), &strength_id);
        }
        None => panic!("Expected Item node for updated strength"),
    }

    let updated_agility_node_unwrapped =
        unwrap_node_result(updated_agility_node).expect("Updated agility node should exist");
    match updated_agility_node_unwrapped.lock().unwrap().as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::I32(100)));
            assert_eq!(get_id_safe(item.get_id_ref()), &agility_id);
        }
        None => panic!("Expected Item node for updated agility"),
    }

    // Test setting the stats keyvalue directly on the player node
    // ## TEST 3 ##
    // mutate new_stats_kv agility and strength values to 200

    new_stats_kv.set_field_value(&agility_name, Value::I32(200));
    new_stats_kv.set_field_value(&strength_name, Value::I32(200));

    let player_node = bb.get(&"player");
    assert_node_exists(player_node.clone());
    let player_node_unwrapped = unwrap_node_result(player_node).expect("Player node should exist");
    {
        let mut player_node_guard = player_node_unwrapped.lock().unwrap();
        let result = player_node_guard
            .as_path_mut()
            .expect("Player node should be a path")
            .set("stats", new_stats_kv.into());

        assert!(result.is_ok(), "Expected Ok result when setting stats");
    }
}

fn test_type_compatibility_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Set initial value as integer
    let id = bb.set(&path(&["player", "level"]), Value::I32(10)).unwrap();

    // Update with compatible type should work
    bb.set(&path(&["player", "level"]), Value::I32(20)).unwrap();

    // Check if updated correctly
    validate_item_ref(&bb, &path(&["player", "level"]), &Value::I32(20), &id);
}

test_both_blackboards!(test_type_compatibility, test_type_compatibility_impl);

fn test_incompatible_type_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Set initial value as integer
    bb.set(&path(&["player", "score"]), Value::I32(100))
        .unwrap();

    // Try to update with incompatible type (should panic)
    bb.set(
        &path(&["player", "score"]),
        Value::String("hundred".to_string()),
    )
    .unwrap();
}

#[test]
#[should_panic(expected = "Incompatible value type for existing item")]
fn test_incompatible_type() {
    test_incompatible_type_impl(AroraMemSpaceType::Arc);
}

#[test]
#[should_panic(expected = "Incompatible value type for existing item")]
fn test_incompatible_type_arora() {
    test_incompatible_type_impl(AroraMemSpaceType::Rc);
}

fn test_get_using_node_trait_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    // Set up a multi-level structure
    bb.set(
        &path(&["game", "world", "player", "position", "x"]),
        Value::F32(10.0),
    )
    .unwrap();
    bb.set(
        &path(&["game", "world", "player", "position", "y"]),
        Value::F32(20.0),
    )
    .unwrap();

    // Note: Individual nodes don't implement navigation methods - only the blackboard root does.
    // We can verify the structure exists by using lookup_arc_node/lookup_node to check nodes exist,
    // and validate values via lookup.
    match bb_type {
        AroraMemSpaceType::Arc => {
            // Verify nodes exist at various depths using lookup_arc_node
            assert!(
                bb.lookup_arc_node(&path(&["game", "world"])).is_some(),
                "World node should exist"
            );
            assert!(
                bb.lookup_arc_node(&path(&["game", "world", "player"]))
                    .is_some(),
                "Player node should exist"
            );
            assert!(
                bb.lookup_arc_node(&path(&["game", "world", "player", "position"]))
                    .is_some(),
                "Position node should exist"
            );
            assert!(
                bb.lookup_arc_node(&path(&["game", "world", "player", "position", "x"]))
                    .is_some(),
                "X node should exist"
            );

            // Verify the final value
            let x_node = bb
                .lookup_arc_node(&path(&["game", "world", "player", "position", "x"]))
                .expect("X node should exist");
            let guard = x_node.lock().unwrap();
            match guard.as_item() {
                Some(item) => {
                    assert_eq!(item.get_value(), Some(&Value::F32(10.0)));
                }
                None => panic!("Expected Item node"),
            }
        }
        AroraMemSpaceType::Rc => {
            // For Rc-based blackboards, verify values exist via lookup
            assert_eq!(
                bb.lookup(&path(&["game", "world", "player", "position", "x"])),
                Some(Value::F32(10.0))
            );
            assert_eq!(
                bb.lookup(&path(&["game", "world", "player", "position", "y"])),
                Some(Value::F32(20.0))
            );

            // Verify intermediate paths exist
            assert_path_exists_ref(&bb, &path(&["game", "world"]));
            assert_path_exists_ref(&bb, &path(&["game", "world", "player"]));
            assert_path_exists_ref(&bb, &path(&["game", "world", "player", "position"]));
        }
    }
}

test_both_blackboards!(test_get_using_node_trait, test_get_using_node_trait_impl);

fn test_get_complex_path_using_node_trait_impl(bb_type: AroraMemSpaceType) {
    let mut bb = AroraMemSpace::new(bb_type, "root");

    let excalibur = Value::String("Excalibur".to_string());
    // Set up a complex structure
    bb.set(
        &path(&["game", "world", "player", "position", "x"]),
        Value::F32(10.0),
    )
    .unwrap();
    bb.set(
        &path(&["game", "world", "player", "position", "y"]),
        Value::F32(20.0),
    )
    .unwrap();
    bb.set(
        &path(&["game", "world", "player", "inventory", "items", "sword"]),
        excalibur.clone(),
    )
    .unwrap();

    match bb_type {
        AroraMemSpaceType::Arc => {
            // Verify various nodes exist using lookup_arc_node
            assert!(
                bb.lookup_arc_node(&"game").is_some(),
                "Game node should exist"
            );

            let world_node = bb.lookup_arc_node(&path(&["game", "world"]));
            assert!(world_node.is_some(), "World node should exist");

            // Verify the world node is a namespace with the correct name
            {
                let world_arc = world_node.unwrap();
                let world_guard = world_arc.lock().unwrap();
                assert!(world_guard.is_path().unwrap());
                assert_eq!(
                    get_name_ref_safe(world_guard.get_current_name_copy()),
                    "world".to_string()
                );
            }

            // Verify all intermediate nodes exist
            assert!(
                bb.lookup_arc_node(&path(&["game", "world", "player"]))
                    .is_some(),
                "Player node should exist"
            );
            assert!(
                bb.lookup_arc_node(&path(&["game", "world", "player", "inventory"]))
                    .is_some(),
                "Inventory node should exist"
            );
            assert!(
                bb.lookup_arc_node(&path(&["game", "world", "player", "inventory", "items"]))
                    .is_some(),
                "Items node should exist"
            );

            // Verify final sword value
            let sword_node = bb
                .lookup_arc_node(&path(&[
                    "game",
                    "world",
                    "player",
                    "inventory",
                    "items",
                    "sword",
                ]))
                .expect("Sword node should exist");
            let sword_guard = sword_node.lock().unwrap();
            match sword_guard.as_item() {
                Some(item) => {
                    assert_eq!(item.get_value(), Some(&excalibur));
                }
                None => panic!("Expected Excalibur Item node"),
            }
        }
        AroraMemSpaceType::Rc => {
            // For Rc-based blackboards, verify all values exist via lookup
            assert_eq!(
                bb.lookup(&path(&["game", "world", "player", "position", "x"])),
                Some(Value::F32(10.0))
            );
            assert_eq!(
                bb.lookup(&path(&["game", "world", "player", "position", "y"])),
                Some(Value::F32(20.0))
            );
            assert_eq!(
                bb.lookup(&path(&[
                    "game",
                    "world",
                    "player",
                    "inventory",
                    "items",
                    "sword"
                ])),
                Some(excalibur)
            );

            // Verify intermediate paths exist
            assert_path_exists_ref(&bb, "game");
            assert_path_exists_ref(&bb, &path(&["game", "world"]));
            assert_path_exists_ref(&bb, &path(&["game", "world", "player"]));
            assert_path_exists_ref(&bb, &path(&["game", "world", "player", "inventory"]));
            assert_path_exists_ref(
                &bb,
                &path(&["game", "world", "player", "inventory", "items"]),
            );
        }
    }
}

test_both_blackboards!(
    test_get_complex_path_using_node_trait,
    test_get_complex_path_using_node_trait_impl
);
