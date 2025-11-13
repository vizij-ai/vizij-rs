//! This module defines the node structure used in the Arora blackboard system.
//! Nodes can be either path nodes (containing other nodes) or item nodes (containing values).
//! This design allows for a hierarchical organization of data in the blackboard.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use arora_schema::value::Value;
use uuid::Uuid;

use crate::{
    arc_abb::{ArcBBPathNode, ArcBBPathNodeTrait, ArcNamespacedSetterTrait},
    traits::{BBNodeTrait, BBPathNodeTrait, BlackboardTrait, ItemsFormattable, TreeFormattable},
    BBItemNode,
};

use super::ArcBlackboard;

/// An abstract node in the blackboard structure, which can be either a path or an item.
///
/// Path nodes act as containers for other nodes, creating a hierarchical structure.
/// Item nodes contain actual data values that can be accessed and modified.
#[derive(Debug)]
pub enum ArcBBNode {
    /// A path node that can contain other nodes
    Path(ArcBBPathNode),
    /// An item node that contains a data value
    Item(BBItemNode),
}

impl ArcBBNode {
    /// Converts an `ArcBBNode` to a `ArcBBPathNode` if it is a path, otherwise returns None.
    ///
    /// # Returns
    /// An `Option<&ArcBBPathNode>` containing a reference to the path node if this is a path node,
    /// or `None` if this is an item node.
    pub fn as_path(&self) -> Option<&ArcBBPathNode> {
        match self {
            ArcBBNode::Path(ns) => Some(ns),
            _ => None,
        }
    }

    pub fn as_path_mut(&mut self) -> Option<&mut ArcBBPathNode> {
        match self {
            ArcBBNode::Path(ns) => Some(ns),
            _ => None,
        }
    }

    /// Converts an `ArcBBNode` to an `BBItemNode` if it is an item, otherwise returns None.
    ///
    /// # Returns
    /// An `Option<&BBItemNode>` containing a reference to the item node if this is an item node,
    /// or `None` if this is a path node.
    pub fn as_item(&self) -> Option<&BBItemNode> {
        match self {
            ArcBBNode::Item(item) => Some(item),
            _ => None,
        }
    }
}

/// Implementation of `BBNodeTrait` for `BBNode`.
///
/// This implementation allows `BBNode` to be used in the blackboard hierarchy,
/// delegating to the appropriate variant (Path or Item).
impl BBNodeTrait for ArcBBNode {
    /// Returns a reference to the ID of this node.
    ///
    /// # Returns
    /// A `Result<&String, String>` containing a reference to the node's ID, or an error message
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        match self {
            ArcBBNode::Path(path) => path.get_id_ref(),
            ArcBBNode::Item(item) => item.get_id_ref(),
        }
    }

    /// Determines if this node is a path node.
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating whether this is a path node, or an error message
    fn is_path(&self) -> Result<bool, String> {
        match self {
            ArcBBNode::Path(_) => Ok(true),
            ArcBBNode::Item(_) => Ok(false),
        }
    }

    /// Returns a copy of the name of this node.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing a copy of the node's name, or an error message
    fn get_current_name_copy(&self) -> Result<String, String> {
        match self {
            ArcBBNode::Path(path) => path.get_current_name_copy(),
            ArcBBNode::Item(item) => item.get_current_name_copy(),
        }
    }

    /// Returns a copy of the ID of this node.
    ///
    /// # Returns
    /// A `Result<String, String>` containing a copy of the node's ID, or an error message
    fn get_id_copy(&self) -> Result<Uuid, String> {
        match self {
            ArcBBNode::Path(path) => path.get_id_copy(),
            ArcBBNode::Item(item) => item.get_id_copy(),
        }
    }

    /// Returns the full path of the node as a string.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String> {
        match self {
            ArcBBNode::Path(path) => path.get_full_path(),
            ArcBBNode::Item(item) => item.get_full_path(),
        }
    }
}

/// Implementation of `BBNodeTrait` for `Arc<Mutex<BBNode>>`.
///
/// This implementation allows `BBNode` to be used in a thread-safe manner,
/// wrapping it in an `Arc<Mutex<>>` to enable shared ownership and mutable access.
impl BBNodeTrait for Arc<Mutex<ArcBBNode>> {
    /// Not implemented for `Arc<Mutex<BBNode>>` directly.
    ///
    /// # Returns
    /// A `Result<&String, String>` containing an error message
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        unimplemented!("BBNode is wrapped in Arc<Mutex<>>, use get_id_copy() instead")
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

/// Implementation of `BBPathNodeTrait` for `Arc<Mutex<BBNode>>`.
///
/// This implementation provides methods to manipulate and access path nodes in a thread-safe manner.
impl BBPathNodeTrait for Arc<Mutex<ArcBBNode>> {
    /// Checks if the given name exists in this path.
    ///
    /// # Arguments
    /// * `name` - The name to check for
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating if an entry with the given name exists, or an error message
    fn contains(&self, name: &str) -> Result<bool, String> {
        if let Ok(guard) = self.lock() {
            if let ArcBBNode::Path(path) = &*guard {
                path.contains(name)
            } else {
                Err("The given BBNode is not a path node.".to_string())
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
            if let ArcBBNode::Path(path) = &mut *guard {
                path.insert(name, id)
            } else {
                Err("The given BBNode is not a path node.".to_string())
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
            if let ArcBBNode::Path(path) = &*guard {
                path.get_name_id(name)
            } else {
                Err("The given BBNode is not a path node.".to_string())
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
            if let ArcBBNode::Path(path) = &*guard {
                path.get_names_copy()
            } else {
                Err("The given BBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    fn _format_tree_recursively(
        &self,
        name: &str,
        id: &Uuid,
        depth: usize,
        show_ids: bool,
        output: &mut String,
    ) {
        if let Ok(guard) = self.lock() {
            if let ArcBBNode::Path(path) = &*guard {
                ArcBBPathNodeTrait::_format_tree_recursively(
                    path, name, id, depth, show_ids, output,
                )
            } else {
                output.push_str("The given BBNode is not a path node.\n");
            }
        } else {
            output.push_str("Failed to lock mutex\n");
        }
    }
}

/// Implementation of `BBPathNodeTrait` for `Arc<Mutex<BBNode>>`.
///
/// This implementation provides methods to manipulate and access path nodes in a thread-safe manner.
impl ArcBBPathNodeTrait for Arc<Mutex<ArcBBNode>> {
    /// Retrieves a node by its ID from the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result<Option<Arc<Mutex<BBNode>>>, String>` containing the node if found, or an error message
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Arc<Mutex<ArcBBNode>>>, String> {
        if let Ok(guard) = self.lock() {
            if let ArcBBNode::Path(path) = &*guard {
                path.get_node_by_id(id)
            } else {
                Err("The given BBNode is not a path node.".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

impl BlackboardTrait for Arc<Mutex<ArcBBNode>> {
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
            if let ArcBBNode::Path(path) = &mut *guard {
                path.set_bb_item(value, item_id, name, full_path)
            } else {
                Err("BBNode is not a path".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `ItemsFormattable` trait for `Arc<Mutex<ArcBBNode>>`.
impl ItemsFormattable for Arc<Mutex<ArcBBNode>> {
    fn format_items(&self, show_ids: bool) -> String {
        if let Ok(guard) = self.lock() {
            if let ArcBBNode::Path(path) = &*guard {
                ItemsFormattable::format_items(path, show_ids)
            } else {
                "BBNode is not a path".to_string()
            }
        } else {
            "Failed to lock mutex".to_string()
        }
    }
}

/// Implementation of `NamespacedSetterTrait` for `Arc<Mutex<BBNode>>`.
///
/// This implementation provides methods to set values in the blackboard
/// using namespaced paths, allowing for hierarchical organization of data.
impl ArcNamespacedSetterTrait for Arc<Mutex<ArcBBNode>> {
    /// Returns a reference to the blackboard that owns this node.
    fn get_blackboard(&self) -> Result<Arc<Mutex<ArcBlackboard>>, String> {
        if let Ok(guard) = self.lock() {
            if let ArcBBNode::Path(path) = &*guard {
                path.get_blackboard()
            } else {
                Err("BBNode is not a path".to_string())
            }
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `TreeFormattable` trait for `Arc<Mutex<ArcBBNode>>`.
impl TreeFormattable for Arc<Mutex<ArcBBNode>> {
    fn format_tree(&self, show_ids: bool) -> String {
        if let Ok(guard) = self.lock() {
            match &*guard {
                ArcBBNode::Path(path) => TreeFormattable::format_tree(path, show_ids),
                ArcBBNode::Item(_) => {
                    unimplemented!("Item node (use path node for tree formatting)")
                }
            }
        } else {
            "Failed to lock mutex".to_string()
        }
    }
}
