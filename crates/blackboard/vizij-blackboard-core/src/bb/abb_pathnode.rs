//! This module defines the path node structure used in the Arora blackboard system.
//! Path nodes act as containers for other nodes, creating a hierarchical structure.
//! They maintain mappings from names to node IDs and provide methods to navigate the hierarchy.

use std::collections::HashMap;
use std::fmt::{self, Display};
use std::sync::{Arc, Mutex, Weak};

use arora_schema::gen_bb_uuid;
use arora_schema::value::Value;
use uuid::Uuid;

use crate::ArcAroraBlackboard;

use super::{
    ABBNodeTrait, ArcABBNode, ArcABBPathNodeTrait, ArcAroraBlackboardTrait,
    ArcNamespacedSetterTrait, ItemsFormattable, TreeFormattable,
};

/// A node that represents a path in the blackboard structure.
///
/// Path nodes act as containers for other nodes, creating a hierarchical namespace.
/// Each path node maintains a mapping from names to node IDs, allowing for easy navigation
/// of the hierarchy.
#[derive(Debug, Clone)]
pub struct ArcABBPathNode {
    /// A map of node names to their corresponding node IDs
    names: HashMap<String, Uuid>, // Component name -> Node ID in names
    /// The name of the current path node
    name: String,
    /// The unique ID of the path node
    id: Uuid,
    /// Thread-safe reference to the blackboard to allow for dynamic updates directly through the path node.
    /// This is a weak reference to avoid circular dependencies.
    bb: Option<Weak<Mutex<ArcAroraBlackboard>>>,

    /// Flag indicating whether this is the root path node
    is_root: bool,
    graph_node_id: Option<Uuid>, // Optional ID for graph nodes, can be None for non-graph nodes

    full_path: String, // Full path for this node
}

impl ArcABBPathNode {
    /// Creates a new ABBPathNode with a full path.
    ///
    /// This constructor allows for creating a path node with an optional full path.
    /// # Arguments
    /// * `name` - The name of the path node
    /// * `id` - The unique ID of the path node
    /// * `bb` - A thread-safe reference to the blackboard
    /// * `full_path` - An optional full path for the node
    /// # Returns
    /// A new `ABBPathNode` instance with the provided name, ID, and blackboard reference.
    /// If `full_path` is `None`, it will be set to `None`.

    pub fn new_with_full_path_and_graph_node(
        name: String,
        id: Uuid,
        bb: Arc<Mutex<ArcAroraBlackboard>>,
        full_path: &String,
        graph_node_id: Option<Uuid>,
    ) -> Self {
        Self {
            names: HashMap::new(),
            name,
            id,
            bb: Some(Arc::downgrade(&bb)), // Store a weak reference to the blackboard
            is_root: false,
            graph_node_id,                // Default to false, can be set later
            full_path: full_path.clone(), // Set the full path if provided
        }
    }

    pub fn new_with_full_path(
        name: String,
        id: Uuid,
        bb: Arc<Mutex<ArcAroraBlackboard>>,
        full_path: &String,
    ) -> Self {
        Self::new_with_full_path_and_graph_node(
            name, id, bb, full_path, None, // Default to false for non-graph nodes
        )
    }

    /*pub fn new(name: String, id: String, bb: Arc<Mutex<ArcAroraBlackboard>>) -> Self {
        Self::new_with_full_path(name, id, bb, &String::new())
    }*/

    /// Creates a new root ABBPathNode.
    ///
    /// A root path node is special because it represents the base of the hierarchy
    /// and initially has no reference to a blackboard.
    ///
    /// # Arguments
    /// * `name` - The name of the root path node
    ///
    /// # Returns
    /// A new `ABBPathNode` instance configured as a root node
    pub fn create_root(name: &String) -> Self {
        let fullpath = name.clone();
        Self {
            names: HashMap::new(),
            name: name.clone(),
            id: gen_bb_uuid(),
            bb: None,
            is_root: true,
            graph_node_id: None, // Root nodes are not graph nodes by default
            full_path: fullpath, // The full path for the root node is just its name
        }
    }

    /// Sets the blackboard reference for a root path node.
    ///
    /// This method should only be called on root path nodes.
    ///
    /// # Arguments
    /// * `bb` - A weak reference to the blackboard
    ///
    /// # Panics
    /// Panics if called on a non-root path node
    pub fn set_bb(&mut self, bb: Weak<Mutex<ArcAroraBlackboard>>) {
        if !self.is_root {
            panic!("Can only set bb for root node");
        }
        self.bb = Some(bb);
    }

    pub fn graph_node_id(&self) -> &Option<Uuid> {
        &self.graph_node_id
    }

    pub fn set_is_graph_node(&mut self, graph_node_id: Option<Uuid>) {
        self.graph_node_id = graph_node_id;
    }
}

/// Implementation of `ABBNodeTrait` for `ABBPathNode`.
///
/// This trait provides methods to access the name and ID of the path node,
impl ABBNodeTrait for ArcABBPathNode {
    /// Returns a reference to the ID of this path node.
    ///
    /// # Returns
    /// A `Result` containing a reference to the path node's ID
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        Ok(&self.id)
    }

    /// Determines if this node is a path node.
    ///
    /// # Returns
    /// A `Result` containing `true` for `ABBPathNode`
    fn is_path(&self) -> Result<bool, String> {
        Ok(true)
    }

    /// Returns a copy of the name of this path node.
    ///
    /// # Returns
    /// A `Result` containing an `Option<String>` with a copy of the path node's name
    fn get_current_name_copy(&self) -> Result<String, String> {
        Ok(self.name.clone())
    }

    /// Returns a copy of the ID of this path node.
    ///
    /// # Returns
    /// A `Result` containing a copy of the path node's ID
    fn get_id_copy(&self) -> Result<Uuid, String> {
        Ok(self.id.clone())
    }

    /// Returns the full path of the node as a string.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String> {
        Ok(self.full_path.clone())
    }
}

/// Implementation of `ABBPathNodeTrait` for `ABBPathNode`.
///
/// This trait provides methods to manage name-to-ID mappings and retrieve nodes by ID.
impl ArcABBPathNodeTrait for ArcABBPathNode {
    /// Checks if the given name exists in this path node.
    ///
    /// # Arguments
    /// * `name` - The name to check for
    ///
    /// # Returns
    /// A `Result` containing `true` if an entry with the given name exists, `false` otherwise
    fn contains(&self, name: &String) -> Result<bool, String> {
        Ok(self.names.contains_key(name))
    }

    /// Inserts a new name-to-ID mapping in this path node.
    ///
    /// # Arguments
    /// * `name` - The name to associate with the ID
    /// * `id` - The ID to map the name to
    fn insert(&mut self, name: String, id: Uuid) -> Result<(), String> {
        self.names.insert(name, id);
        Ok(())
    }

    /// Gets the ID associated with a name in this path node.
    ///
    /// # Arguments
    /// * `name` - The name to look up
    ///
    /// # Returns
    /// A `Result` containing an `Option<String>` with the ID if found, or `None` if not found
    fn get_name_id(&self, name: &String) -> Result<Option<Uuid>, String> {
        Ok(self.names.get(name).cloned())
    }

    /// Retrieves a node by its ID from the blackboard.
    ///
    /// This method delegates to the blackboard for the actual lookup using the hashed ID.
    /// It requires the weak reference to the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result` containing an `Option<Arc<Mutex<ABBNode>>>` with the node if found, or `None` if not found
    ///
    /// # Errors
    /// Returns an error if:
    /// - No blackboard reference is available
    /// - The blackboard no longer exists
    /// - Failed to lock the blackboard mutex
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String> {
        if let Some(ref bb) = self.bb {
            if let Some(arc_bb) = bb.upgrade() {
                let ret_node = if let Ok(guard) = arc_bb.lock() {
                    guard.get_node_by_id(id)
                } else {
                    Err("Failed to lock blackboard mutex".to_string())
                };
                ret_node
            } else {
                Err("Blackboard no longer exists".to_string())
            }
        } else {
            Err("No blackboard reference available".to_string())
        }
    }

    /// Returns a copy of all name-to-ID mappings in this path node.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap<String, String>` with name-to-ID mappings
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String> {
        Ok(self.names.clone())
    }
}

/// Implementation of `AroraBlackboardTrait` for `ABBPathNode`.
///
/// This trait provides methods to interact with the blackboard, such as printing items and setting values.
impl ArcAroraBlackboardTrait for ArcABBPathNode {
    /// Sets an item into the blackboard with the given value, ID, and optional name.
    ///
    /// This method delegates to the blackboard reference.
    ///
    /// # Arguments
    /// * `value` - The value to set
    /// * `item_id` - The ID for the item
    /// * `name` - Optional name for the item
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating success or an error message
    ///
    /// # Errors
    /// Returns an error if:
    /// - No blackboard reference is available
    /// - The blackboard no longer exists
    /// - Failed to lock the blackboard mutex
    fn set_bb_item(
        &mut self,
        value: Value,
        item_id: &Uuid,
        name: Option<String>,
        full_path: Option<&String>,
    ) -> Result<bool, String> {
        if let Some(ref bb) = self.bb {
            if let Some(arc_bb) = bb.upgrade() {
                if let Ok(mut guard) = arc_bb.lock() {
                    guard.set_bb_item(value, item_id, name, full_path)
                } else {
                    Err("Failed to lock blackboard mutex".to_string())
                }
            } else {
                Err("Blackboard no longer exists".to_string())
            }
        } else {
            Err("No blackboard reference available".to_string())
        }
    }
}

/// Implementation of `NamespacedSetterTrait` for `ABBPathNode`.
///
/// This trait provides methods to access the blackboard reference for setting values.
impl ArcNamespacedSetterTrait for ArcABBPathNode {
    /// Returns a reference to the blackboard.
    ///
    /// # Returns
    /// A `Result<Arc<Mutex<ArcAroraBlackboard>>, String>` containing the reference to the blackboard or an error
    ///
    /// # Errors
    /// Returns an error if:
    /// - No blackboard reference is available
    /// - The blackboard no longer exists
    fn get_blackboard(&self) -> Result<Arc<Mutex<ArcAroraBlackboard>>, String> {
        if let Some(ref bb) = self.bb {
            if let Some(arc_bb) = bb.upgrade() {
                Ok(arc_bb)
            } else {
                Err("Blackboard no longer exists".to_string())
            }
        } else {
            Err("No blackboard reference available".to_string())
        }
    }
}

/// Implementation of `Display` trait for `ArcABBPathNode`.
///
/// This provides a formatted string representation of the path node and its contents.
impl Display for ArcABBPathNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            TreeFormattable::format_tree(self, false).trim_end()
        )
    }
}

/// Implementation of `TreeFormattable` trait for `ArcABBPathNode`.
impl TreeFormattable for ArcABBPathNode {
    fn format_tree(&self, show_ids: bool) -> String {
        let mut output = String::new();
        let name = self.get_current_name_copy();
        if let Err(e) = name {
            return format!("Error getting name: {}", e);
        }
        output.push_str(&format!("{:?}' Namespace Tree:\n", name));
        let names = self.get_names_copy();
        if let Err(e) = names {
            return format!("Error getting names: {}", e);
        }
        for (name, ref_id) in names.unwrap() {
            self._format_tree_recursively(&name, &ref_id, 1, show_ids, &mut output);
        }
        output
    }
}

/// Implementation of `ItemsFormattable` trait for `ArcABBPathNode`.
impl ItemsFormattable for ArcABBPathNode {
    fn format_items(&self, show_ids: bool) -> String {
        if let Some(ref bb) = self.bb {
            if let Some(arc_bb) = bb.upgrade() {
                if let Ok(guard) = arc_bb.lock() {
                    ItemsFormattable::format_items(&*guard, show_ids)
                } else {
                    "Failed to lock blackboard mutex".to_string()
                }
            } else {
                "Blackboard no longer exists".to_string()
            }
        } else {
            "No blackboard reference available".to_string()
        }
    }
}
