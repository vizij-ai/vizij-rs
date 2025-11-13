//! This module defines the path node structure used in the Arora blackboard system.
//! Path nodes act as containers for other nodes, creating a hierarchical structure.
//! They maintain mappings from names to node IDs and provide methods to navigate the hierarchy.

use arora_schema::gen_bb_uuid;
use arora_schema::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::rc::{Rc, Weak};
use uuid::Uuid;

use super::{ABBNode, ABBPathNodeTrait, AroraBlackboard, NamespacedSetterTrait};
use crate::traits::{
    BBNodeTrait, BBPathNodeTrait, BlackboardTrait, ItemsFormattable, TreeFormattable,
};

/// A node that represents a path in the blackboard structure.
///
/// Path nodes act as containers for other nodes, creating a hierarchical namespace.
/// Each path node maintains a mapping from names to node IDs, allowing for easy navigation
/// of the hierarchy.
#[derive(Debug, Clone)]
pub struct ABBPathNode {
    /// A map of node names to their corresponding node IDs
    names: HashMap<String, Uuid>, // Component name -> Node Id
    /// The name of the current path node
    name: String,
    /// The unique ID of the path node
    id: Uuid,
    /// Thread-safe reference to the blackboard to allow for dynamic updates directly through the path node.
    /// This is a weak reference to avoid circular dependencies.
    bb: Option<Weak<RefCell<AroraBlackboard>>>,

    /// Flag indicating whether this is the root path node
    is_root: bool,
    graph_node_id: Option<Uuid>, // Optional ID for graph nodes, can be None for non-graph nodes

    full_path: String, // Full path for this node
}

impl ABBPathNode {
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
        bb: Weak<RefCell<AroraBlackboard>>,
        full_path: &str,
        graph_node_id: Option<Uuid>,
    ) -> Self {
        Self {
            names: HashMap::new(),
            name,
            id,
            bb: Some(bb),
            is_root: false,
            graph_node_id,                   // Default to false, can be set later
            full_path: full_path.to_owned(), // Set the full path if provided
        }
    }

    pub fn new_with_full_path(
        name: String,
        id: Uuid,
        bb: Weak<RefCell<AroraBlackboard>>,
        full_path: &str,
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
    pub fn create_root(name: &str) -> Self {
        let fullpath = name.to_owned();
        Self {
            names: HashMap::new(),
            name: name.to_owned(),
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
    pub fn set_bb(&mut self, bb: Weak<RefCell<AroraBlackboard>>) {
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
impl BBNodeTrait for ABBPathNode {
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
        Ok(self.id)
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
impl BBPathNodeTrait for ABBPathNode {
    /// Checks if the given name exists in this path node.
    ///
    /// # Arguments
    /// * `name` - The name to check for
    ///
    /// # Returns
    /// A `Result` containing `true` if an entry with the given name exists, `false` otherwise
    fn contains(&self, name: &str) -> Result<bool, String> {
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
    fn get_name_id(&self, name: &str) -> Result<Option<Uuid>, String> {
        Ok(self.names.get(name).cloned())
    }

    /// Returns a copy of all name-to-ID mappings in this path node.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap<String, String>` with name-to-ID mappings
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String> {
        Ok(self.names.clone())
    }

    fn _format_tree_recursively(
        &self,
        name: &str,
        id: &Uuid,
        depth: usize,
        show_ids: bool,
        output: &mut String,
    ) {
        ABBPathNodeTrait::_format_tree_recursively(self, name, id, depth, show_ids, output);
    }
}

/// Implementation of `ABBPathNodeTrait` for `ABBPathNode`.
///
/// This trait provides methods to manage name-to-ID mappings and retrieve nodes by ID.
impl ABBPathNodeTrait for ABBPathNode {
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
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Rc<RefCell<ABBNode>>>, String> {
        if let Some(ref bb_ref) = self.bb {
            if let Some(bb) = bb_ref.upgrade() {
                bb.borrow().get_node_by_id(id)
            } else {
                Err("Blackboard no longer exists".to_string())
            }
        } else {
            Err("No blackboard reference available".to_string())
        }
    }
}

/// Implementation of `AroraBlackboardTrait` for `ABBPathNode`.
///
/// This trait provides methods to interact with the blackboard, such as printing items and setting values.
impl BlackboardTrait for ABBPathNode {
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
    fn set_bb_item(
        &mut self,
        value: Value,
        item_id: &Uuid,
        name: Option<String>,
        full_path: Option<&str>,
    ) -> Result<bool, String> {
        if let Some(ref bb_ref) = self.bb {
            if let Some(bb) = bb_ref.upgrade() {
                bb.borrow_mut().set_bb_item(value, item_id, name, full_path)
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
impl NamespacedSetterTrait for ABBPathNode {
    /// Returns a reference to the blackboard.
    ///
    /// # Returns
    /// A `Result<Weak<RefCell<AroraBlackboard>>, String>` containing the blackboard reference, or an error message
    ///
    /// # Errors
    /// Returns an error if:
    /// - No blackboard reference is available
    /// - The blackboard no longer exists
    fn get_blackboard(&self) -> Result<Weak<RefCell<AroraBlackboard>>, String> {
        if let Some(ref bb_ref) = self.bb {
            Ok(bb_ref.clone())
        } else {
            Err("No blackboard reference available".to_string())
        }
    }

    fn _insert_entry(&mut self, id: Uuid, item: Rc<RefCell<ABBNode>>) {
        self.names.insert(
            item.borrow()
                .get_current_name_copy()
                .unwrap_or_else(|_| "Unnamed".to_string()),
            id,
        );
    }
}

/// Implementation of `Display` trait for `ABBPathNode`.
///
/// This provides a formatted string representation of the path node and its contents.
impl Display for ABBPathNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            TreeFormattable::format_tree(self, false).trim_end()
        )
    }
}

/// Implementation of `TreeFormattable` trait for `ABBPathNode`.
impl TreeFormattable for ABBPathNode {
    fn format_tree(&self, show_ids: bool) -> String {
        let mut output = String::new();
        let name = self.get_current_name_copy();
        if let Err(e) = name {
            return format!("Error getting name: {}", e);
        }
        output.push_str(&format!("{:?} Namespace Tree:\n", name.unwrap()));
        let names = self.get_names_copy();
        if let Err(e) = names {
            return format!("Error getting names: {}", e);
        }
        for (name, node_ref) in names.unwrap() {
            BBPathNodeTrait::_format_tree_recursively(
                self,
                &name,
                &node_ref,
                1,
                show_ids,
                &mut output,
            );
        }
        output
    }
}

/// Implementation of `ItemsFormattable` trait for `ABBPathNode`.
impl ItemsFormattable for ABBPathNode {
    fn format_items(&self, show_ids: bool) -> String {
        if let Some(ref bb) = self.bb {
            ItemsFormattable::format_items(
                &*bb.upgrade().expect("Blackboard no longer exists").borrow(),
                show_ids,
            )
        } else {
            "No blackboard reference available".to_string()
        }
    }
}
