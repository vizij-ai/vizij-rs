//! This module defines the node structure used in the Arora blackboard system.
//! Nodes can be either path nodes (containing other nodes) or item nodes (containing values).
//! This design allows for a hierarchical organization of data in the blackboard.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use arora_schema::value::Value;
use uuid::Uuid;

use crate::bb::{
    abb_traits::ABBNodeTrait, ArcABBPathNodeTrait, ArcAroraBlackboardTrait,
    ArcNamespacedSetterTrait, ItemsFormattable, TreeFormattable,
};
use crate::{
    bb::{ABBItemNode, ArcABBPathNode},
    ArcAroraBlackboard,
};

/// An abstract node in the blackboard structure, which can be either a path or an item.
///
/// Path nodes act as containers for other nodes, creating a hierarchical structure.
/// Item nodes contain actual data values that can be accessed and modified.
#[derive(Debug)]
pub enum ArcABBNode {
    /// A path node that can contain other nodes
    Path(ArcABBPathNode),
    /// An item node that contains a data value
    Item(ABBItemNode),
}

impl ArcABBNode {
    /// Converts an `ABBNode` to a `PathNode` if it is a path, otherwise returns None.
    ///
    /// # Returns
    /// An `Option<&ABBPathNode>` containing a reference to the path node if this is a path node,
    /// or `None` if this is an item node.
    pub fn as_path(&self) -> Option<&ArcABBPathNode> {
        match self {
            ArcABBNode::Path(ns) => Some(ns),
            _ => None,
        }
    }

    pub fn as_path_mut(&mut self) -> Option<&mut ArcABBPathNode> {
        match self {
            ArcABBNode::Path(ns) => Some(ns),
            _ => None,
        }
    }

    /// Converts an `ABBNode` to an `ItemNode` if it is an item, otherwise returns None.
    ///
    /// # Returns
    /// An `Option<&ABBItemNode>` containing a reference to the item node if this is an item node,
    /// or `None` if this is a path node.
    pub fn as_item(&self) -> Option<&ABBItemNode> {
        match self {
            ArcABBNode::Item(item) => Some(item),
            _ => None,
        }
    }
}

/// Implementation of `ABBNodeTrait` for `ABBNode`.
///
/// This implementation allows `ABBNode` to be used in the blackboard hierarchy,
/// delegating to the appropriate variant (Path or Item).
impl ABBNodeTrait for ArcABBNode {
    /// Returns a reference to the ID of this node.
    ///
    /// # Returns
    /// A `Result<&String, String>` containing a reference to the node's ID, or an error message
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        match self {
            ArcABBNode::Path(path) => path.get_id_ref(),
            ArcABBNode::Item(item) => item.get_id_ref(),
        }
    }

    /// Determines if this node is a path node.
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating whether this is a path node, or an error message
    fn is_path(&self) -> Result<bool, String> {
        match self {
            ArcABBNode::Path(_) => Ok(true),
            ArcABBNode::Item(_) => Ok(false),
        }
    }

    /// Returns a copy of the name of this node.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing a copy of the node's name, or an error message
    fn get_current_name_copy(&self) -> Result<String, String> {
        match self {
            ArcABBNode::Path(path) => path.get_current_name_copy(),
            ArcABBNode::Item(item) => item.get_current_name_copy(),
        }
    }

    /// Returns a copy of the ID of this node.
    ///
    /// # Returns
    /// A `Result<String, String>` containing a copy of the node's ID, or an error message
    fn get_id_copy(&self) -> Result<Uuid, String> {
        match self {
            ArcABBNode::Path(path) => path.get_id_copy(),
            ArcABBNode::Item(item) => item.get_id_copy(),
        }
    }

    /// Returns the full path of the node as a string.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String> {
        match self {
            ArcABBNode::Path(path) => path.get_full_path(),
            ArcABBNode::Item(item) => item.get_full_path(),
        }
    }
}

/// Implementation of `ABBNodeTrait` for `Arc<Mutex<ABBNode>>`.
///
/// This implementation allows `ABBNode` to be used in a thread-safe manner,
/// wrapping it in an `Arc<Mutex<>>` to enable shared ownership and mutable access.
impl ABBNodeTrait for Arc<Mutex<ArcABBNode>> {
    /// Not implemented for `Arc<Mutex<ABBNode>>` directly.
    ///
    /// # Returns
    /// A `Result<&String, String>` containing an error message
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        unimplemented!("ABBNode is wrapped in Arc<Mutex<>>, use get_id_copy() instead")
    }

    /// Determines if this node is a path node.
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating whether this is a path node, or an error message
    fn is_path(&self) -> Result<bool, String> {
        if let Ok(guard) = self.lock() {
            guard.is_path()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns a copy of the name of this node.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing a copy of the node's name, or an error message
    fn get_current_name_copy(&self) -> Result<String, String> {
        if let Ok(guard) = self.lock() {
            guard.get_current_name_copy()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns a copy of the ID of this node.
    ///
    /// # Returns
    /// A `Result<String, String>` containing a copy of the node's ID, or an error message
    fn get_id_copy(&self) -> Result<Uuid, String> {
        if let Ok(guard) = self.lock() {
            guard.get_id_ref().copied()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns the full path of the node as a string.
    ///    
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String> {
        if let Ok(guard) = self.lock() {
            guard.get_full_path()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `ABBPathNodeTrait` for `Arc<Mutex<ABBNode>>`.
///
/// This implementation provides methods to manipulate and access path nodes in a thread-safe manner.
impl ArcABBPathNodeTrait for Arc<Mutex<ArcABBNode>> {
    /// Checks if the given name exists in this path.
    ///
    /// # Arguments
    /// * `name` - The name to check for
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating if an entry with the given name exists, or an error message
    fn contains(&self, name: &str) -> Result<bool, String> {
        if let Ok(guard) = self.lock() {
            if let ArcABBNode::Path(path) = &*guard {
                path.contains(name)
            } else {
                Err("The given ABBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Inserts a new name-to-ID mapping in this path.
    ///
    /// # Arguments
    /// * `name` - The name to associate with the ID
    /// * `id` - The ID to map the name to
    ///
    /// # Returns
    /// A `Result<(), String>` indicating success or an error message
    fn insert(&mut self, name: String, id: Uuid) -> Result<(), String> {
        if let Ok(mut guard) = self.lock() {
            if let ArcABBNode::Path(path) = &mut *guard {
                path.insert(name, id)
            } else {
                Err("The given ABBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Gets the ID associated with a name in this path.
    ///
    /// # Arguments
    /// * `name` - The name to look up
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the ID if found, or an error message
    fn get_name_id(&self, name: &str) -> Result<Option<Uuid>, String> {
        if let Ok(guard) = self.lock() {
            if let ArcABBNode::Path(path) = &*guard {
                path.get_name_id(name)
            } else {
                Err("The given ABBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Retrieves a node by its ID from the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result<Option<Arc<Mutex<ABBNode>>>, String>` containing the node if found, or an error message
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String> {
        if let Ok(guard) = self.lock() {
            if let ArcABBNode::Path(path) = &*guard {
                path.get_node_by_id(id)
            } else {
                Err("The given ABBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns a copy of the map of item names and IDs in this path.
    ///
    /// # Returns
    /// A `Result<HashMap<String, String>, String>` containing name-to-ID mappings, or an error message
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String> {
        if let Ok(guard) = self.lock() {
            if let ArcABBNode::Path(path) = &*guard {
                path.get_names_copy()
            } else {
                Err("The given ABBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

impl ArcAroraBlackboardTrait for Arc<Mutex<ArcABBNode>> {
    /// Sets an item into the blackboard, given a Value and an ID.
    ///
    /// # Arguments
    /// * `value` - The value to set
    /// * `item_id` - The ID for the item
    /// * `name` - Optional name for the item (required when creating a new item)
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating success or an error message
    fn set_bb_item(
        &mut self,
        value: Value,
        item_id: &Uuid,
        name: Option<String>,
        full_path: Option<&str>,
    ) -> Result<bool, String> {
        if let Ok(mut guard) = self.lock() {
            if let ArcABBNode::Path(path) = &mut *guard {
                path.set_bb_item(value, item_id, name, full_path)
            } else {
                Err("ABBNode is not a path".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `ItemsFormattable` trait for `Arc<Mutex<ArcABBNode>>`.
impl ItemsFormattable for Arc<Mutex<ArcABBNode>> {
    fn format_items(&self, show_ids: bool) -> String {
        if let Ok(guard) = self.lock() {
            if let ArcABBNode::Path(path) = &*guard {
                ItemsFormattable::format_items(path, show_ids)
            } else {
                "ABBNode is not a path".to_string()
            }
        } else {
            "Failed to lock mutex".to_string()
        }
    }
}

/// Implementation of `NamespacedSetterTrait` for `Arc<Mutex<ABBNode>>`.
///
/// This implementation provides methods to set values in the blackboard
/// using namespaced paths, allowing for hierarchical organization of data.
impl ArcNamespacedSetterTrait for Arc<Mutex<ArcABBNode>> {
    /// Returns a reference to the blackboard that owns this node.
    fn get_blackboard(&self) -> Result<Arc<Mutex<ArcAroraBlackboard>>, String> {
        if let Ok(guard) = self.lock() {
            if let ArcABBNode::Path(path) = &*guard {
                path.get_blackboard()
            } else {
                Err("ABBNode is not a path".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `TreeFormattable` trait for `Arc<Mutex<ArcABBNode>>`.
impl TreeFormattable for Arc<Mutex<ArcABBNode>> {
    fn format_tree(&self, show_ids: bool) -> String {
        if let Ok(guard) = self.lock() {
            match &*guard {
                ArcABBNode::Path(path) => TreeFormattable::format_tree(path, show_ids),
                ArcABBNode::Item(_) => {
                    unimplemented!("Item node (use path node for tree formatting)")
                }
            }
        } else {
            "Failed to lock mutex".to_string()
        }
    }
}
