use arora_schema::value::Value;
use vizij_blackboard_core::simple_blackboard::{ItemHolder, SimpleBlackboard};

#[test]
fn test_blackboard_creation() {
    let bb = SimpleBlackboard::new("TestBlackboard".to_string());
    assert_eq!(bb.name(), "TestBlackboard");
    assert!(bb.index().is_empty());
}

#[test]
fn test_add_root_level_item() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add an item at root level
    let _ = bb.add_item(&"health", Value::I32(100));

    // Verify it was added correctly
    let value = bb.get_item(&"health");
    assert!(value.is_some());
    if let Some(Value::I32(val)) = value {
        assert_eq!(*val, 100);
    } else {
        panic!("Expected Int value but got {:?}", value);
    }
}

#[test]
fn test_overwrite_existing_item() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add an item
    let _ = bb.add_item(&"score", Value::I32(50));

    // Verify initial value
    if let Some(Value::I32(val)) = bb.get_item(&"score") {
        assert_eq!(*val, 50);
    } else {
        panic!(
            "Expected initial Int value but got {:?}",
            bb.get_item(&"score")
        );
    }

    // Overwrite the item
    let _ = bb.add_item(&"score", Value::I32(100));

    // Verify new value
    if let Some(Value::I32(val)) = bb.get_item(&"score") {
        assert_eq!(*val, 100);
    } else {
        panic!(
            "Expected updated Int value but got {:?}",
            bb.get_item(&"score")
        );
    }
}

#[test]
fn test_add_nested_items() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add nested items
    let _ = bb.add_item(&"player.stats.strength", Value::I32(50));
    let _ = bb.add_item(&"player.stats.agility", Value::I32(75));
    let _ = bb.add_item(&"player.inventory.gold", Value::I32(1000));

    // Verify they were added correctly
    if let Some(Value::I32(strength)) = bb.get_item(&"player.stats.strength") {
        assert_eq!(*strength, 50);
    } else {
        panic!(
            "Expected strength Int value but got {:?}",
            bb.get_item(&"player.stats.strength")
        );
    }

    if let Some(Value::I32(agility)) = bb.get_item(&"player.stats.agility") {
        assert_eq!(*agility, 75);
    } else {
        panic!(
            "Expected agility Int value but got {:?}",
            bb.get_item(&"player.stats.agility")
        );
    }

    if let Some(Value::I32(gold)) = bb.get_item(&"player.inventory.gold") {
        assert_eq!(*gold, 1000);
    } else {
        panic!(
            "Expected gold Int value but got {:?}",
            bb.get_item(&"player.inventory.gold")
        );
    }
}

#[test]
fn test_different_value_types() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add different types of values
    let _ = bb.add_item(&"is_active", Value::Boolean(true));
    let _ = bb.add_item(&"name", Value::String("Player1".to_string()));
    let _ = bb.add_item(&"position.x", Value::F64(10.5));
    let _ = bb.add_item(
        &"inventory.items",
        Value::ArrayString(vec![
            "Sword".to_string(),
            "Shield".to_string(),
            "Potion".to_string(),
        ]),
    );

    // Verify bool value
    if let Some(Value::Boolean(val)) = bb.get_item(&"is_active") {
        assert_eq!(*val, true);
    } else {
        panic!(
            "Expected Bool value but got {:?}",
            bb.get_item(&"is_active")
        );
    }

    // Verify string value
    if let Some(Value::String(val)) = bb.get_item(&"name") {
        assert_eq!(val, "Player1");
    } else {
        panic!("Expected String value but got {:?}", bb.get_item(&"name"));
    }

    // Verify float value
    if let Some(Value::F64(val)) = bb.get_item(&"position.x") {
        assert_eq!(*val, 10.5);
    } else {
        panic!(
            "Expected Float value but got {:?}",
            bb.get_item(&"position.x")
        );
    }

    // Verify array value
    if let Some(Value::ArrayString(val)) = bb.get_item(&"inventory.items") {
        assert_eq!(val.len(), 3);
        assert_eq!(val[0], "Sword");
        assert_eq!(val[1], "Shield");
        assert_eq!(val[2], "Potion");
    } else {
        panic!(
            "Expected ArrayString value but got {:?}",
            bb.get_item(&"inventory.items")
        );
    }
}

#[test]
fn test_deep_nesting() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add deeply nested items
    let _ = bb.add_item(
        &"game.world.region.zone.npc.dialogue.greeting",
        Value::String("Hello adventurer!".to_string()),
    );

    // Verify deep nesting works
    if let Some(Value::String(greeting)) =
        bb.get_item(&"game.world.region.zone.npc.dialogue.greeting")
    {
        assert_eq!(greeting, "Hello adventurer!");
    } else {
        panic!(
            "Expected String value in deep nesting but got {:?}",
            bb.get_item(&"game.world.region.zone.npc.dialogue.greeting")
        );
    }
}

#[test]
fn test_remove_root_item() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add and then remove a root item
    let _ = bb.add_item(&"temp", Value::I32(100));

    // Verify it exists
    assert!(bb.get_item(&"temp").is_some());

    // Remove it
    let removed = bb.remove_item(&"temp");

    // Verify removal returned the value
    if let Some(Value::I32(val)) = removed {
        assert_eq!(val, 100);
    } else {
        panic!("Expected Int value from removal but got {:?}", removed);
    }

    // Verify it no longer exists
    assert!(bb.get_item(&"temp").is_none());
}

#[test]
fn test_remove_nested_item() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add nested items
    let _ = bb.add_item(&"player.stats.strength", Value::I32(50));
    let _ = bb.add_item(&"player.stats.agility", Value::I32(75));

    // Verify they exist
    assert!(bb.get_item(&"player.stats.strength").is_some());
    assert!(bb.get_item(&"player.stats.agility").is_some());

    // Remove one of them
    let removed = bb.remove_item(&"player.stats.strength");

    // Verify removal returned the value
    if let Some(Value::I32(val)) = removed {
        assert_eq!(val, 50);
    } else {
        panic!("Expected Int value from removal but got {:?}", removed);
    }

    // Verify it no longer exists but the other one does
    assert!(bb.get_item(&"player.stats.strength").is_none());
    assert!(bb.get_item(&"player.stats.agility").is_some());

    // Remove the other one
    let removed = bb.remove_item(&"player.stats.agility");

    // Verify removal returned the value
    if let Some(Value::I32(val)) = removed {
        assert_eq!(val, 75);
    } else {
        panic!("Expected Int value from removal but got {:?}", removed);
    }

    // Verify it no longer exists
    assert!(bb.get_item(&"player.stats.agility").is_none());
}

#[test]
fn test_nonexistent_item() {
    let bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Try to get a nonexistent item
    let value = bb.get_item(&"does_not_exist");
    assert!(value.is_none());

    // Try to get a nonexistent nested item
    let value = bb.get_item(&"does_not.exist.at.all");
    assert!(value.is_none());
}

#[test]
fn test_partial_path_exists() {
    let mut bb: SimpleBlackboard = SimpleBlackboard::new("TestBlackboard".to_string());

    // Create a partial path
    let _ = bb.add_item(&"game.settings.graphics", Value::String("High".to_string()));

    // Try to get something beyond the existing path
    let value = bb.get_item(&"game.settings.graphics.resolution");
    assert!(value.is_none());

    // Try to get something in a different branch
    let value = bb.get_item(&"game.settings.audio");
    assert!(value.is_none());
}

#[test]
fn test_remove_nonexistent_item() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Try to remove something that doesn't exist
    let removed = bb.remove_item(&"does_not_exist");
    assert!(removed.is_none());

    // Add an item then try to remove a nonexistent sibling
    let _ = bb.add_item(&"player.stats.strength", Value::I32(50));

    let removed = bb.remove_item(&"player.stats.nonexistent");
    assert!(removed.is_none());

    // Make sure the existing item is still there
    assert!(bb.get_item(&"player.stats.strength").is_some());
}

#[test]
fn test_path_conflict() {
    let mut bb = SimpleBlackboard::new("TestBlackboard".to_string());

    // Add a leaf node
    bb.add_item(
        &"player.stats",
        Value::String("These are stats".to_string()),
    )
    .expect("First add_item should succeed");

    // Now try to use it as a path component (this should return an error)
    let result = bb.add_item(&"player.stats.strength", Value::I32(50));

    assert!(
        result.is_err(),
        "Expected an error when adding to a path that conflicts with a leaf node"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("Path conflict"),
        "Error message should mention path conflict: {}",
        err
    );
}
