use arora_schema::keyvalue::{KeyValue, KeyValueField};
use arora_schema::value::Value;
use arora_schema::{gen_bb_uuid, gen_uuid_from_str};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
use vizij_blackboard_core::{
    bb::{ABBNodeTrait, ArcABBNode, ArcABBPathNodeTrait, ArcNamespacedSetterTrait},
    ArcAroraBlackboard,
};

// Utility functions for handling Results and Options
fn is_some_node(node: Result<Option<Arc<Mutex<ArcABBNode>>>, String>) -> bool {
    node.is_ok() && node.unwrap().is_some()
}

fn is_path_node(node: Result<Option<Arc<Mutex<ArcABBNode>>>, String>) -> bool {
    if let Ok(Some(node_arc)) = node {
        if let Ok(node_guard) = node_arc.lock() {
            return node_guard.is_path().unwrap_or(false);
        }
    }
    false
}

fn contains_node(node_result: Result<Option<Arc<Mutex<ArcABBNode>>>, String>, name: &str) -> bool {
    if let Ok(Some(node_arc)) = node_result {
        if let Ok(node_guard) = node_arc.lock() {
            if let Ok(is_path) = node_guard.is_path() {
                if is_path {
                    if let Some(path) = node_guard.as_path() {
                        return path.contains(name).unwrap_or(false);
                    }
                }
            }
        }
    }
    false
}

// Helper function to unwrap a Result<Option<Arc<Mutex<ABBNode>>>, String> to Option<Arc<Mutex<ABBNode>>>
fn unwrap_node_result(
    node: Result<Option<Arc<Mutex<ArcABBNode>>>, String>,
) -> Option<Arc<Mutex<ArcABBNode>>> {
    node.unwrap_or_default()
}

// Helper function that simplifies unwrapping and checking if a node is present
fn assert_node_exists(node: Result<Option<Arc<Mutex<ArcABBNode>>>, String>) {
    assert!(node.is_ok(), "Node result should be Ok");
    let unwrapped = node.unwrap();
    assert!(unwrapped.is_some(), "Node should be Some");
}

// Helper function that simplifies checking if a node is a path
fn assert_node_is_path(node: Result<Option<Arc<Mutex<ArcABBNode>>>, String>) {
    assert!(is_path_node(node));
}

// Generic helper function that works with both references and copies
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

// Helper function to get an Option<&String> from a Result<Option<&String>, String>
fn get_name_ref_safe(name_result: Result<String, String>) -> String {
    name_result.expect("Failed to get name reference")
}

fn validate_item<S: ToString + ?Sized>(
    name: &S,
    value: &Value,
    id: &Uuid,
    node: Result<Option<Arc<Mutex<ArcABBNode>>>, String>,
) {
    // Check if the node is Some
    assert!(node.is_ok());
    let node = node.unwrap();
    assert!(node.is_some());

    if let Some(node_arc) = node {
        let node = node_arc.lock().unwrap();
        if let Some(item) = node.as_item() {
            assert_eq!(
                get_name_ref_safe(item.get_current_name_copy()),
                name.to_string()
            );
            assert_eq!(item.get_value(), Some(value));
            assert_eq!(get_id_safe(item.get_id_ref()), id);
        } else {
            panic!("Expected Item node");
        }
    } else {
        panic!("Node is None");
    }
}

#[test]
fn test_bb_creation() {
    let bb: Arc<Mutex<ArcAroraBlackboard>> = ArcAroraBlackboard::new("root".to_string());
    if let Ok(bb_names) = bb.get_names_copy() {
        // Check if the names contain "root"
        assert!(bb_names.is_empty());
    } else {
        panic!("Failed to get names from blackboard");
    }
}

#[test]
fn test_add_simple_value() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add a simple value
    let value = &Value::I32(42);
    let name = &"test_var".to_string();
    let id = bb.set(name, value.clone()).unwrap();
    // Retrieve the node by id
    let node_by_id = bb.get_node_by_id(&id);
    validate_item(name, value, &id, node_by_id);
    // Verify it exists in the root namespace
    assert!(bb.contains(name).unwrap());

    // Retrieve the node by namespace
    let node = bb.get(name);
    validate_item(name, value, &id, node);

    // Retrieve the node by name
    let node_by_name = bb.get(name);
    validate_item(name, value, &id, node_by_name);
}

#[test]
fn test_single_level_namespace() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add a value in a single-level namespace
    let math_name = &"math".to_string();
    let pi_name = &"pi".to_string();
    let pi_value = &Value::F32(std::f32::consts::PI);
    let path = &format!("{}.{}", math_name, pi_name);
    let pi_id = bb.set(path, pi_value.clone()).unwrap();

    // Verify the math namespace path exists
    let math_node = bb.get(math_name);
    assert!(is_some_node(math_node.clone()));

    // Verify the math node is a namespace
    assert!(is_path_node(math_node.clone()));

    // Verify the full path exists in the BB namespace and corresponds to the pi node
    let pi_node = bb.get(path);
    validate_item(pi_name, pi_value, &pi_id, pi_node);

    // verify the pi node exists in the math node
    assert!(contains_node(math_node.clone(), pi_name));

    // Get the math node and then get pi from it
    if let Ok(Some(math_arc)) = math_node {
        let pi_node_in_math = math_arc.get(pi_name);
        validate_item(pi_name, pi_value, &pi_id, pi_node_in_math);
    } else {
        panic!("Math node should exist");
    }

    // Verify the correct node is retrieved by id
    let node_by_id = bb.get_node_by_id(&pi_id);
    validate_item(pi_name, pi_value, &pi_id, node_by_id);
}

#[test]
fn test_multi_level_namespace() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add values in multi-level namespaces
    let pos_x = &Value::F32(10.0);
    let pos_y = &Value::F32(20.0);
    let pos_z = &Value::F32(30.0);

    let x_id = bb
        .set(&"entity.transform.position.x", pos_x.clone())
        .unwrap();
    let y_id = bb
        .set(&"entity.transform.position.y", pos_y.clone())
        .unwrap();
    let z_id = bb
        .set(&"entity.transform.position.z", pos_z.clone())
        .unwrap();

    // Verify intermediate namespaces exist
    assert_node_exists(bb.get(&"entity"));
    assert_node_exists(bb.get(&"entity.transform"));
    assert_node_exists(bb.get(&"entity.transform.position"));

    // Check values by full path
    let x_node = bb.get(&"entity.transform.position.x");
    let y_node = bb.get(&"entity.transform.position.y");
    let z_node = bb.get(&"entity.transform.position.z");

    validate_item(&"x", pos_x, &x_id, x_node);
    validate_item(&"y", pos_y, &y_id, y_node);
    validate_item(&"z", pos_z, &z_id, z_node);

    // verify the entity node exists and is a namespace
    let entity_node = bb.get(&"entity");
    assert_node_exists(entity_node.clone());
    let entity_node_unwrapped =
        unwrap_node_result(entity_node.clone()).expect("Entity node should exist");
    {
        let entity_node_guard = entity_node_unwrapped.lock().unwrap();
        assert!(entity_node_guard.is_path().unwrap());
    }

    // verify the transform node exists and is a namespace
    let transform_node = bb.get(&"entity.transform");
    assert_node_exists(transform_node.clone());
    let transform_node_unwrapped =
        unwrap_node_result(transform_node.clone()).expect("Transform node should exist");
    {
        let transform_node_guard = transform_node_unwrapped.lock().unwrap();
        assert!(transform_node_guard.is_path().unwrap());
    }

    // verify the transform node exists in the entity node
    let transform_node_in_entity = entity_node_unwrapped.get(&"transform");
    assert_node_exists(transform_node_in_entity.clone());
    let transform_node_in_entity_unwrapped =
        unwrap_node_result(transform_node_in_entity).expect("Transform in entity should exist");
    {
        let transform_node_in_entity_guard = transform_node_in_entity_unwrapped.lock().unwrap();
        assert!(transform_node_in_entity_guard.is_path().unwrap());
    }

    // verify the transform node id matches the one in the entity node
    let check_id = get_id_safe(transform_node_in_entity_unwrapped.get_id_copy());
    let target_id = get_id_safe(transform_node_unwrapped.get_id_copy());
    assert_eq!(check_id, target_id);

    // verify the position node exists and is a namespace
    let position_node = bb.get(&"entity.transform.position");
    assert_node_exists(position_node.clone());
    let position_node_unwrapped =
        unwrap_node_result(position_node.clone()).expect("Position node should exist");
    {
        let position_node_guard = position_node_unwrapped.lock().unwrap();
        assert!(position_node_guard.is_path().unwrap());
    }

    // verify the position node exists in the transform node
    let position_node_in_transform = transform_node_unwrapped.get(&"position");
    assert_node_exists(position_node_in_transform.clone());
    let position_node_in_transform_unwrapped =
        unwrap_node_result(position_node_in_transform.clone())
            .expect("Position in transform should exist");
    {
        let position_node_in_transform_guard = position_node_in_transform_unwrapped.lock().unwrap();
        assert!(position_node_in_transform_guard.is_path().unwrap());
    }

    // verify the position node id matches the one in the transform node
    let check_id = get_id_safe(position_node_in_transform_unwrapped.get_id_copy());
    let target_id = get_id_safe(position_node_unwrapped.get_id_copy());
    assert_eq!(check_id, target_id);
}

#[test]
fn test_namespaces_with_multiple_values() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add multiple values to the same namespace
    let hp = Value::I32(100);
    let mp = Value::I32(50);
    let speed = Value::F32(5.5);

    let hp_id = bb.set(&"player.stats.hp".to_string(), hp.clone()).unwrap();
    let mp_id = bb.set(&"player.stats.mp".to_string(), mp.clone()).unwrap();
    let speed_id = bb
        .set(&"player.stats.speed".to_string(), speed.clone())
        .unwrap();

    // Verify the namespace exists
    let player_node = bb.get(&"player");
    assert_node_exists(player_node.clone());
    assert_node_is_path(player_node);

    // verify the ids correspond to the expected values
    let hp_node = bb.get(&"player.stats.hp");
    validate_item(&"hp".to_string(), &hp, &hp_id, hp_node);

    let mp_node = bb.get(&"player.stats.mp");
    validate_item(&"mp".to_string(), &mp, &mp_id, mp_node);

    let speed_node = bb.get(&"player.stats.speed");
    validate_item(&"speed".to_string(), &speed, &speed_id, speed_node);
}

#[test]
fn test_custom_ids() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add a value with custom ID
    let value = Value::String("test_value".to_string());
    let custom_id = gen_bb_uuid();
    let custom_name = "custom_item".to_string();

    let returned_custom_id = bb
        .set_with_id(&custom_name, value.clone(), Some(custom_id))
        .unwrap();
    assert!(returned_custom_id == custom_id);

    // Verify we can get the node by name
    let node = bb.get(&custom_name);
    assert_node_exists(node.clone());

    // Verify the ID was preserved
    let node_unwrapped = unwrap_node_result(node).expect("Node should exist");
    {
        let node_guard = node_unwrapped.lock().unwrap();
        let item = node_guard.as_item().unwrap();
        assert_eq!(get_id_safe(item.get_id_ref()), &custom_id);
        assert_eq!(item.get_value(), Some(&value));
        assert_eq!(item.get_current_name_copy().unwrap(), custom_name);
    }

    // Verify we can get the node by ID
    let node_by_id = bb.get_node_by_id(&custom_id);
    validate_item(&"custom_item".to_string(), &value, &custom_id, node_by_id);
}

#[test]
fn test_non_existent_paths() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add some values
    bb.set(&"a.b.c".to_string(), Value::I32(1)).unwrap();

    // Test non-existent paths
    assert!(unwrap_node_result(bb.get(&"")).is_none());
    assert!(unwrap_node_result(bb.get(&"x")).is_none());
    assert!(unwrap_node_result(bb.get(&"a.x")).is_none());
    assert!(unwrap_node_result(bb.get(&"a.b.x")).is_none());
    assert!(unwrap_node_result(bb.get(&"a.b.c.d")).is_none());
}

#[test]
fn test_complex_values() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Test with more complex values
    let bool_value = Value::Boolean(true);
    let string_value = Value::String("Hello, World!".to_string());
    let array_value = Value::ArrayI32(vec![1, 2, 3]);

    let bool_id = bb
        .set(&"values.bool".to_string(), bool_value.clone())
        .unwrap();
    let string_id = bb
        .set(&"values.string".to_string(), string_value.clone())
        .unwrap();
    let array_id = bb
        .set(&"values.array".to_string(), array_value.clone())
        .unwrap();

    // get and validate the values
    let bool_node = bb.get(&"values.bool");
    validate_item(&"bool".to_string(), &bool_value, &bool_id, bool_node);

    let string_node = bb.get(&"values.string");
    validate_item(
        &"string".to_string(),
        &string_value,
        &string_id,
        string_node,
    );

    let array_node = bb.get(&"values.array");
    validate_item(&"array".to_string(), &array_value, &array_id, array_node);
}

#[test]
#[should_panic(expected = "Path cannot be empty when setting an item to the blackboard")]
fn test_empty_name() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());
    bb.set(&"".to_string(), Value::I32(123)).unwrap();
}

#[test]
fn test_namespace_node() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add some values to create namespaces
    bb.set(&"system.config.debug".to_string(), Value::Boolean(true))
        .unwrap();
    bb.set(&"system.config.log_level".to_string(), Value::I32(3))
        .unwrap();

    // Get the namespace node
    let config_node = bb.get(&"system.config");
    assert_node_exists(config_node.clone());

    let config_node_unwrapped = unwrap_node_result(config_node).expect("Config node should exist");
    let config_node_guard = config_node_unwrapped.lock().unwrap();
    let namespace = config_node_guard.as_path().unwrap();

    assert!(namespace.contains("debug").unwrap());
    assert!(namespace.contains("log_level").unwrap());
    assert!(!get_id_safe(namespace.get_id_ref()).is_nil());
}

#[test]
fn test_overwrite_existing_item() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Add a value
    let id1 = bb.set(&"player.health", Value::I32(100)).unwrap();

    // Verify initial value
    let node = bb.get(&"player.health");
    assert_node_exists(node.clone());
    validate_item(&"health", &Value::I32(100), &id1, node);

    // Overwrite with new value
    let id2 = bb.set(&"player.health", Value::I32(150)).unwrap();

    // IDs should be the same when overwriting
    assert_eq!(id1, id2);

    // Verify updated value
    let node = bb.get(&"player.health");
    assert_node_exists(node.clone());
    validate_item(&"health", &Value::I32(150), &id1, node);
}

#[test]
fn test_remove_item() {
    // Test pending - would need to implement remove functionality
    // This test should verify:
    // 1. Removing a root level item
    // 2. Removing a nested item
    // 3. Attempting to remove a non-existent item

    // For now, this is a placeholder to highlight that remove functionality
    // should be implemented in ArcAroraBlackboard
}

#[test]
fn test_path_conflict() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Create a namespace node
    bb.set(&"player.inventory.gold", Value::I32(100)).unwrap();

    // Now try to use the namespace as a value path (should panic)
    let result = bb.set(
        &"player.inventory",
        Value::String("this should fail".to_string()),
    );
    assert!(
        result.is_err(),
        "Expected an error when trying to set a value at an existing path node"
    );
    let error_message = result.unwrap_err();
    assert!(
        error_message.contains(
            "Path player.inventory already exists as a BBPath node, cannot set it with a Value"
        ),
        "Error message should contain expected text, but got: {}",
        error_message
    );
}

#[test]
fn test_keyvalue_structure() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

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
    let health_node = bb.get(&"player.health");
    assert_node_exists(health_node.clone());
    let strength_node = bb.get(&"player.stats.strength");
    assert_node_exists(strength_node.clone());
    let agility_node = bb.get(&"player.stats.agility");
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
        .set(&"player.stats".to_string(), new_stats_kv.clone().into())
        .unwrap();

    assert!(!result.is_nil());

    // Check that values were set correctly
    let updated_strength_node = bb.get(&"player.stats.strength");
    assert_node_exists(updated_strength_node.clone());

    let updated_agility_node = bb.get(&"player.stats.agility");
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

#[test]
fn test_type_compatibility() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Set initial value as integer
    bb.set(&"player.level", Value::I32(10)).unwrap();

    // Update with compatible type should work
    bb.set(&"player.level", Value::I32(20)).unwrap();

    // Check if updated correctly
    let node = bb.get(&"player.level");
    let node_unwrapped = unwrap_node_result(node).expect("Level node should exist");
    let node_guard = node_unwrapped.lock().unwrap();
    match node_guard.as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::I32(20)));
        }
        None => panic!("Expected Item node"),
    }
}

#[test]
#[should_panic(expected = "Incompatible value type for existing item")]
fn test_incompatible_type() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Set initial value as integer
    bb.set(&"player.score", Value::I32(100)).unwrap();

    // Try to update with incompatible type (should panic)
    bb.set(&"player.score", Value::String("hundred".to_string()))
        .unwrap();
}

#[test]
fn test_get_using_node_trait() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    // Set up a multi-level structure
    bb.set(&"game.world.player.position.x", Value::F32(10.0))
        .unwrap();
    bb.set(&"game.world.player.position.y", Value::F32(20.0))
        .unwrap();

    // Get the world node
    let world_node = bb.get(&"game.world");
    assert_node_exists(world_node.clone());

    // Use the ABBNodeTrait methods to navigate
    let world_node_unwrapped = unwrap_node_result(world_node).expect("World node should exist");
    let player_node = world_node_unwrapped.get(&"player");
    assert_node_exists(player_node.clone());

    let player_node_unwrapped = unwrap_node_result(player_node).expect("Player node should exist");
    let position_node = player_node_unwrapped.get(&"position");
    assert_node_exists(position_node.clone());

    let position_node_unwrapped =
        unwrap_node_result(position_node).expect("Position node should exist");
    let x_node = position_node_unwrapped.get(&"x");
    assert_node_exists(x_node.clone());

    // Verify final value
    let x_node_unwrapped = unwrap_node_result(x_node).expect("X node should exist");
    let guard = x_node_unwrapped.lock().unwrap();
    match guard.as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&Value::F32(10.0)));
        }
        None => panic!("Expected Item node"),
    }
}

#[test]
fn test_get_complex_path_using_node_trait() {
    let mut bb = ArcAroraBlackboard::new("root".to_string());

    let excalibur = Value::String("Excalibur".to_string());
    // Set up a complex structure
    bb.set(&"game.world.player.position.x", Value::F32(10.0))
        .unwrap();
    bb.set(&"game.world.player.position.y", Value::F32(20.0))
        .unwrap();
    bb.set(
        &"game.world.player.inventory.items.sword",
        excalibur.clone(),
    )
    .unwrap();

    // Get the world node
    let game_node = bb.get(&"game");
    println!("Game node: {:?}", game_node);
    assert_node_exists(game_node.clone());
    let game_node_unwrapped = unwrap_node_result(game_node).expect("Game node should exist");

    // Use the ABBNodeTrait methods to navigate
    let world_node_from_game = game_node_unwrapped.get(&"world");
    assert_node_exists(world_node_from_game.clone());
    let world_node_from_game_unwrapped =
        unwrap_node_result(world_node_from_game.clone()).expect("World node should exist");

    // Verify the world node is a namespace with the correct name and id
    {
        let world_node_guard = world_node_from_game_unwrapped.lock().unwrap();
        assert!(world_node_guard.is_path().unwrap());
        assert_eq!(
            get_name_ref_safe(world_node_guard.get_current_name_copy()),
            "world".to_string()
        );
    }

    let player_node = game_node_unwrapped.get(&"world.player");
    assert_node_exists(player_node);

    let inventory_node_from_world = world_node_from_game_unwrapped.get(&"player.inventory");
    assert_node_exists(inventory_node_from_world.clone());

    let inventory_node_from_world_unwrapped =
        unwrap_node_result(inventory_node_from_world.clone()).expect("Inventory node should exist");
    let items_node_from_inventory = inventory_node_from_world_unwrapped.get(&"items");
    assert_node_exists(items_node_from_inventory);

    let sword_node_from_inventory = inventory_node_from_world_unwrapped.get(&"items.sword");
    assert_node_exists(sword_node_from_inventory.clone());

    // Verify final value
    let sword_node_unwrapped =
        unwrap_node_result(sword_node_from_inventory).expect("Sword node should exist");
    let sword_guard = sword_node_unwrapped.lock().unwrap();
    match sword_guard.as_item() {
        Some(item) => {
            assert_eq!(item.get_value(), Some(&excalibur));
        }
        None => panic!("Expected Excalibur Item node"),
    }
}
