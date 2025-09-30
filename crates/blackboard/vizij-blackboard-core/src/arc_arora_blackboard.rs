//! This module defines a blackboard structure that can hold various types of data and allows for dynamic updates.
//! The blackboard is designed to be thread-safe and can be shared across multiple threads.
//! It uses a unique ID for each item to ensure that items can be referenced and updated without ambiguity.
//! It includes a namespace path feature which allows to organize, set and access items in a hierarchical manner.
//! While all storage is done in a single HashMap, the namespace path allows to create a tree-like structure where its leafs reference the actual data.
//!
//! The code below defines various types identifiable by the prefix ABB which stands for AroraBlackBoard.
//! In particular, it contains the following main types:
//! - ArcAroraBlackboard: The main struct representing the blackboard, which contains a collection of nodes indexed by their IDs
//! - ABBNode: An abstract node in the blackboard structure
//! - Each BB item is encapsulated in an ABBNode, which can be either a path or an item
//! - If it is a path, it can contain other nodes, allowing for a hierarchical namespaced structure
//! - If it is an item, it contains a value and its type
//! - ABBPathNode: A node that represents a path in the blackboard structure
//! - ABBItemNode: A node that represents a value item in the blackboard structure

use arora_schema::{gen_bb_uuid, value::Value};
use std::{
    collections::HashMap,
    fmt::{self, Display},
    sync::{Arc, Mutex, Weak},
};
use uuid::Uuid;

use super::bb::{ArcABBNode, ArcABBPathNode};

use super::{
    adt,
    bb::{
        ABBItemNode, ABBNodeTrait, ArcABBPathNodeTrait, ArcAroraBlackboardTrait,
        ArcNamespacedSetterTrait, ItemsFormattable, TreeFormattable,
    },
};

/// A struct representing the blackboard, which contains a collection of nodes indexed by their IDs.
/// This struct is designed to be thread-safe and can be shared across threads.
/// The blackboard is initialized with a path node with the same ID as the blackboard itself, which serves as the root namespace.
/// This allows for a hierarchical structure where nodes can be added or removed dynamically.
#[derive(Debug)]
pub struct ArcAroraBlackboard {
    /// The ID of the blackboard, which is used as the root path node.
    /// Allows to distinguish between multiple BBs in a system.
    id: Uuid,
    /// This HashMap is the owner of all nodes.
    items: HashMap<Uuid, Arc<Mutex<ArcABBNode>>>,
    /// A self-reference that can be passed around without cloning.
    self_ref: Option<Weak<Mutex<ArcAroraBlackboard>>>,
}

impl ArcAroraBlackboard {
    /// Creates a new blackboard with the given ID.
    ///
    /// This function performs the following steps:
    /// 1. Creates a root path node with the given ID
    /// 2. Creates the blackboard structure
    /// 3. Links the root path node to the blackboard
    /// 4. Sets up the self-reference for the blackboard
    ///
    /// # Arguments
    /// * `id` - A string identifier for the blackboard
    ///
    /// # Returns
    /// An `Arc<Mutex<ArcAroraBlackboard>>` containing the newly created blackboard
    pub fn new<S: ToString>(name: S) -> Arc<Mutex<Self>> {
        let id = gen_bb_uuid();
        // Step 1: Create a path node with a temporary bb placeholder
        let path_node = ArcABBPathNode::create_root(&name.to_string());

        // Step 2: Create a struct with the wrapped path node
        let mut bb: ArcAroraBlackboard = Self {
            id: id,
            items: HashMap::new(),
            self_ref: None,
        };

        // Initialize the items HashMap with the path node that will act as the root, using the BB id as key
        bb.items
            .insert(id, Arc::new(Mutex::new(ArcABBNode::Path(path_node))));

        // Create the Arc<Mutex<>> that will be returned
        let arc_bb = Arc::new(Mutex::new(bb));

        // Create a Weak reference to copy over to the path node
        let weak_bb = Arc::downgrade(&arc_bb);

        // Now update the bb field with the weak reference
        if let Ok(mut guard) = arc_bb.lock() {
            if let Some(node_ref) = guard.items.get_mut(&id) {
                if let Ok(mut node_guard) = node_ref.lock() {
                    if let ArcABBNode::Path(ref mut path) = *node_guard {
                        // Set the weak reference directly
                        path.set_bb(weak_bb.clone());
                    } else {
                        panic!("Base node is not a BBPath");
                    }
                } else {
                    panic!("Failed to lock path node");
                }
            }
            guard.self_ref = Some(weak_bb);
        }

        arc_bb
    }

    /// Inserts a node into the blackboard's items HashMap.
    ///
    /// This method is used by the `NamespacedSetterTrait` to add items to the blackboard.
    ///
    /// # Arguments
    /// * `id` - The unique identifier for the node
    /// * `item` - The node to insert, wrapped in an Arc<Mutex<>>
    pub fn insert_item_in_hash(&mut self, id: Uuid, item: Arc<Mutex<ArcABBNode>>) {
        // Insert the item into the items HashMap
        self.items.insert(id, item);
    }

    // Additional methods go here
}

/// Implementation of JsonSerializable for ArcAroraBlackboard.
impl JsonSerializable for ArcAroraBlackboard {
    fn to_json(&self) -> Result<serde_json::Value, String> {
        // Preallocate collections with capacity hints to avoid reallocations
        let item_count = self.items.len();
        let mut items = serde_json::Map::with_capacity(item_count); // Maximum case, all are items
        let mut structure = serde_json::Map::with_capacity(item_count);

        // Prepare static string references to avoid repeated allocations
        const TYPE_KEY: &str = "type";
        const ID_KEY: &str = "id";
        const NAME_KEY: &str = "name";
        const CHILDREN_KEY: &str = "children";
        const PATH_TYPE: &str = "path";
        const GRAPH_NODE_ID: &str = "graph_node_id";
        const ITEM_TYPE: &str = "item";
        const VALUE_TYPE_KEY: &str = "value_type";
        const UNNAMED: &str = "(unnamed)";

        // First pass: collect all nodes and their data to minimize lock contention
        // This approach batches the work and reduces the number of lock operations
        let mut node_data = Vec::with_capacity(item_count);

        for (id, node_arc) in &self.items {
            if let Ok(node_guard) = node_arc.lock() {
                match &*node_guard {
                    ArcABBNode::Path(path) => {
                        // Get name and children in one lock operation
                        let name = path
                            .get_current_name_copy()
                            .ok()
                            .unwrap_or_else(|| UNNAMED.to_string());
                        let names = path.get_names_copy().unwrap_or_default();
                        let graph_node_id = if let Some(gnid) = path.graph_node_id() {
                            gnid.to_string()
                        } else {
                            "".to_string()
                        };
                        node_data.push((
                            id.to_string(),
                            true,
                            name,
                            None,
                            None,
                            names,
                            graph_node_id,
                        ));
                    }
                    ArcABBNode::Item(item) => {
                        if let Some(name) = item.get_current_name_copy().ok() {
                            if let Some(value) = item.get_value() {
                                let value_type = item.get_value_type().to_string();
                                // Clone minimal data while under lock
                                node_data.push((
                                    id.to_string(),
                                    false,
                                    name.to_string(),
                                    Some(value.clone()),
                                    Some(value_type),
                                    HashMap::new(),
                                    "".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Second pass: process the collected data without holding locks
        for (id, is_path, name, value_opt, value_type_opt, names, graph_node_id) in node_data {
            if is_path {
                // Build path node structure
                let mut path_obj = serde_json::Map::new();
                path_obj.insert(
                    TYPE_KEY.to_string(),
                    serde_json::Value::String(PATH_TYPE.to_string()),
                );
                path_obj.insert(ID_KEY.to_string(), serde_json::Value::String(id.clone()));
                path_obj.insert(NAME_KEY.to_string(), serde_json::Value::String(name));

                // Add children if any
                if !names.is_empty() {
                    let children_json: Vec<serde_json::Value> = names
                        .values()
                        .map(|child_id| serde_json::Value::String(child_id.to_string()))
                        .collect();

                    path_obj.insert(
                        CHILDREN_KEY.to_string(),
                        serde_json::Value::Array(children_json),
                    );
                }

                path_obj.insert(
                    GRAPH_NODE_ID.to_string(),
                    serde_json::Value::String(graph_node_id),
                );

                structure.insert(id, serde_json::Value::Object(path_obj));
            } else {
                // Process item node
                if let Some(value) = value_opt {
                    // Leverage the Serialize trait directly
                    match serde_json::to_value(&value) {
                        Ok(json_value) => {
                            items.insert(id.clone(), json_value);

                            // Build item structure
                            let mut item_obj = serde_json::Map::new();
                            item_obj.insert(
                                TYPE_KEY.to_string(),
                                serde_json::Value::String(ITEM_TYPE.to_string()),
                            );
                            item_obj
                                .insert(ID_KEY.to_string(), serde_json::Value::String(id.clone()));
                            item_obj.insert(NAME_KEY.to_string(), serde_json::Value::String(name));

                            if let Some(value_type) = value_type_opt {
                                item_obj.insert(
                                    VALUE_TYPE_KEY.to_string(),
                                    serde_json::Value::String(value_type),
                                );
                            }

                            structure.insert(id, serde_json::Value::Object(item_obj));
                        }
                        Err(e) => {
                            // Fall back to string representation if serialization fails
                            items.insert(id, serde_json::Value::String(value.to_string()));

                            // Log the error but don't block the process
                            eprintln!("Error serializing value for {}: {}", name, e);
                        }
                    }
                }
            }
        }

        // Build the final JSON structure
        let mut result = serde_json::Map::with_capacity(3);
        result.insert(
            "id".to_string(),
            serde_json::Value::String(self.id.to_string()),
        );
        result.insert("items".to_string(), serde_json::Value::Object(items));
        result.insert(
            "structure".to_string(),
            serde_json::Value::Object(structure),
        );

        Ok(serde_json::Value::Object(result))
    }
}

/// Implementation of `ABBNodeTrait` for `ArcAroraBlackboard`
///
/// This implementation allows the blackboard itself to be treated as a node in the
/// hierarchy, with the blackboard ID serving as its name and identifier.
impl ABBNodeTrait for ArcAroraBlackboard {
    /// Returns a reference to the ID of the blackboard.
    ///
    /// # Returns
    /// A `Result` containing a reference to the blackboard's ID
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        Ok(&self.id)
    }

    /// Determines if this node is a path node, otherwise it's an item node.
    ///
    /// For `ArcAroraBlackboard`, this always returns true since a blackboard
    /// is conceptually a root path node.
    ///
    /// # Returns
    /// A `Result` containing `true` for `ArcAroraBlackboard`
    fn is_path(&self) -> Result<bool, String> {
        Ok(true)
    }

    /// Returns a copy of the name of the blackboard.
    ///
    /// # Returns
    /// A `Result` containing an `Option<String>` with a copy of the blackboard's ID as its name
    fn get_current_name_copy(&self) -> Result<String, String> {
        Ok(self.id.to_string())
    }

    /// Returns a copy of the ID of the blackboard.
    ///
    /// # Returns
    /// A `Result` containing a copy of the blackboard's ID
    fn get_id_copy(&self) -> Result<Uuid, String> {
        Ok(self.id)
    }

    /// Returns the full path of the node as a string.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String> {
        // For the blackboard, the full path is the name of its base node.
        // To retrieve it, we must first check that the blackboard's id exists in the items HashMap.
        if let Some(base_node) = self.items.get(&self.id) {
            if let Ok(node_guard) = base_node.lock() {
                if let ArcABBNode::Path(path) = &*node_guard {
                    return path.get_full_path();
                }
                return Err("Base node is not a BBPath".to_string());
            } else {
                return Err(format!(
                    "Failed to lock base node for blackboard {}",
                    self.id
                ));
            }
        }
        Err(format!(
            "Base node for blackboard {} not found in items",
            self.id
        ))
    }
}

/// Implementation of `ABBPathNodeTrait` for `ArcAroraBlackboard`
///
/// This implementation allows the blackboard to be treated as a path node.
impl ArcABBPathNodeTrait for ArcAroraBlackboard {
    /// Checks if the given name exists in the root namespace of the blackboard.
    ///
    /// This delegates to the path node at the root of the blackboard.
    /// Recall that we retrieve the root node using the blackboard's ID.
    ///
    /// # Arguments
    /// * `name` - The name to check for
    ///
    /// # Returns
    /// A `Result` containing `true` if an entry with the given name exists, `false` otherwise
    fn contains(&self, name: &String) -> Result<bool, String> {
        if let Some(base_node) = self.items.get(&self.id) {
            if let Ok(node_guard) = base_node.lock() {
                if let ArcABBNode::Path(path) = &*node_guard {
                    return path.contains(name);
                }
            }
        }
        // Return false if the base node isn't found or isn't a BBPath
        Ok(false)
    }

    /// Inserts a new name-to-ID mapping in the root namespace of the blackboard.
    ///
    /// This delegates to the path node at the root of the blackboard.
    /// Recall that we retrieve the root node using the blackboard's ID.
    ///
    /// # Arguments
    /// * `name` - The name to associate with the ID
    /// * `id` - The ID to map the name to
    ///
    /// # Returns
    /// A `Result<(), String>` indicating success or failure
    fn insert(&mut self, name: String, id: Uuid) -> Result<(), String> {
        if let Some(base_node) = self.items.get_mut(&self.id) {
            if let Ok(mut node_guard) = base_node.lock() {
                if let ArcABBNode::Path(path) = &mut *node_guard {
                    return path.insert(name, id);
                }
            }
        }
        Ok(())
    }

    /// Gets the ID associated with a name in the root namespace.
    ///
    /// This delegates to the path node at the root of the blackboard.
    /// Recall that we retrieve the root node using the blackboard's ID.
    ///
    /// # Arguments
    /// * `name` - The name to look up
    ///
    /// # Returns
    /// A `Result` containing an `Option<String>` with the ID if found, or `None` if not found
    fn get_name_id(&self, name: &String) -> Result<Option<Uuid>, String> {
        if let Some(base_node) = self.items.get(&self.id) {
            if let Ok(node_guard) = base_node.lock() {
                if let ArcABBNode::Path(path) = &*node_guard {
                    return path.get_name_id(name);
                }
            }
        }
        // Return None if the base node isn't found or isn't a BBPath
        Ok(None)
    }

    /// Retrieves a node by its ID from the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result` containing an `Option<Arc<Mutex<ABBNode>>>` with the node if found, or `None` if not found
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String> {
        Ok(self.items.get(id).cloned())
    }

    /// Returns a copy of all name-to-ID mappings in the root namespace.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap<String, String>` with name-to-ID mappings
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String> {
        if let Some(base_node) = self.items.get(&self.id) {
            if let Ok(node_guard) = base_node.lock() {
                if let ArcABBNode::Path(path) = &*node_guard {
                    return path.get_names_copy();
                }
            }
        }
        // Return empty HashMap if the base node isn't found or isn't a BBPath
        Ok(HashMap::new())
    }
}

/// Implementation of `AroraBlackboardTrait` for `ArcAroraBlackboard`.
///
/// This implementation provides methods to interact with the blackboard,
/// such as printing items and setting items with values.
impl ArcAroraBlackboardTrait for ArcAroraBlackboard {
    /// Sets an item into the blackboard with the given value, ID, and optional name.
    ///
    /// If the item already exists, its value is updated. If it doesn't exist,
    /// a new item is created with the given name.
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
        full_path: Option<&String>,
    ) -> Result<bool, String> {
        if self.items.contains_key(item_id) {
            if let Some(node_arc) = self.items.get(item_id) {
                // Items exists, so get its existing item and update its value

                // Lock the mutex to get access to the node
                if let Ok(mut node_guard) = node_arc.lock() {
                    // Check that the node is an Item node
                    if let ArcABBNode::Item(ref mut item) = *node_guard {
                        // Confirm item_id matches the existing item
                        if item_id != item.get_id_ref()? {
                            return Err(format!("Item ID does not match existing item ID, existing item: {}, new item: {}",
                                item.get_id_ref()?, item_id));
                        }

                        let value =
                            adt::utils::quick_convert_to_type(&value, item.get_value_type(), false);

                        // Check if the value is Some
                        if let Some(_) = item.get_value() {
                            if adt::utils::is_compatible_type(&value, item.get_value_type()) {
                                // Set the new value
                                item.set_value(value);
                            } else {
                                return Err(format!("Incompatible value type for existing item {}: expected {:?}, got {:?}",
                                    item_id, item.get_value_type(), value));
                            }
                            return Ok(true);
                        } else {
                            return Err(format!("Existing item {} has no value to set", item_id));
                        }
                    } else {
                        return Err(format!("Tried to set an existing id {} with a new value but it was not an Item", item_id));
                    }
                } else {
                    return Err(format!("Failed to lock mutex for node {}", item_id));
                }
            } else {
                return Err(format!(
                    "Tried to set an existing id {} with a new value but it was not found",
                    item_id
                ));
            }
        } else {
            if name.is_none() || name.clone().unwrap().is_empty() {
                return Err(
                    "Name cannot be empty when adding a new item to the blackboard".to_string(),
                );
            }

            let item_result = ABBItemNode::from_value(
                name.as_ref().unwrap(),
                value,
                item_id.clone(),
                full_path.unwrap_or(&name.as_ref().unwrap().clone()),
            );

            match item_result {
                Ok(Some(item)) => {
                    // Create the node and insert it into items HashMap (as owner)
                    let node: ArcABBNode = ArcABBNode::Item(item);
                    let node_arc = Arc::new(Mutex::new(node));
                    self.items.insert(item_id.clone(), node_arc);
                    return Ok(true);
                }
                Ok(None) => {
                    return Err("Failed to create ABBItemNode: returned None".to_string());
                }
                Err(e) => {
                    return Err(format!("Failed to create ABBItemNode: {}", e));
                }
            }
        }
    }
}

/// Implementation of `NamespacedSetterTrait` for `ArcAroraBlackboard`.
///
/// This trait allows the blackboard to be used to set items in a namespaced manner.
impl ArcNamespacedSetterTrait for ArcAroraBlackboard {
    /// Returns a reference to the blackboard itself.
    ///
    /// # Returns
    /// A `Result<Arc<Mutex<ArcAroraBlackboard>>, String>` containing the reference to the blackboard or an error
    fn get_blackboard(&self) -> Result<Arc<Mutex<ArcAroraBlackboard>>, String> {
        if let Some(ref weak_self) = self.self_ref {
            if let Some(arc_self) = weak_self.upgrade() {
                Ok(arc_self)
            } else {
                Err("Blackboard no longer exists".to_string())
            }
        } else {
            Err("No self reference available in blackboard".to_string())
        }
    }
}

/// Implementation of `ABBNodeTrait` for `Arc<Mutex<ArcAroraBlackboard>>`.
///
/// This implementation allows the Arc blackboard to be treated as a node in the
/// hierarchy, with the blackboard ID serving as its name and identifier.
impl ABBNodeTrait for Arc<Mutex<ArcAroraBlackboard>> {
    /// Not implemented for `Arc<Mutex<ArcAroraBlackboard>>`.
    ///
    /// # Returns
    /// An error indicating this operation is not supported
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        unimplemented!("ArcAroraBlackboard is wrapped in Arc<Mutex<>>, use get_id_copy() instead")
    }

    /// Determines if this node is a path node, otherwise it's an item node.
    ///
    /// For `ArcAroraBlackboard`, this always returns true since a blackboard
    /// is conceptually a root path node.
    ///
    /// # Returns
    /// A `Result` containing `true` for `ArcAroraBlackboard`
    fn is_path(&self) -> Result<bool, String> {
        Ok(true)
    }

    /// Returns a copy of the name of the blackboard.
    /// This is necessary because the blackboard is wrapped in an `Arc<Mutex<>>`,
    /// and we cannot return a reference directly.
    fn get_current_name_copy(&self) -> Result<String, String> {
        if let Ok(guard) = self.lock() {
            guard.get_current_name_copy()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns a copy of the ID of the blackboard.
    /// This is necessary because the blackboard is wrapped in an `Arc<Mutex<>>`,
    /// and we cannot return a reference directly.
    fn get_id_copy(&self) -> Result<Uuid, String> {
        if let Ok(guard) = self.lock() {
            guard.get_id_copy()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns the full path of the node as a string.
    /// This is necessary because the blackboard is wrapped in an `Arc<Mutex<>>`,
    /// and we cannot return a reference directly.
    fn get_full_path(&self) -> Result<String, String> {
        if let Ok(guard) = self.lock() {
            guard.get_full_path()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `ABBPathNodeTrait` for `Arc<Mutex<ArcAroraBlackboard>>`.
///
/// This implementation allows the Arc blackboard to be treated as a path node,
/// which is necessary for the namespaced structure of the blackboard.
impl ArcABBPathNodeTrait for Arc<Mutex<ArcAroraBlackboard>> {
    /// Checks if the given name exists in the root namespace of the blackboard.
    fn contains(&self, name: &String) -> Result<bool, String> {
        if let Ok(guard) = self.lock() {
            guard.contains(name)
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Inserts a new name-to-ID mapping in the root namespace of the blackboard.
    fn insert(&mut self, name: String, id: Uuid) -> Result<(), String> {
        if let Ok(mut guard) = self.lock() {
            guard.insert(name, id)
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Gets the ID associated with a name in the root namespace.
    fn get_name_id(&self, name: &String) -> Result<Option<Uuid>, String> {
        if let Ok(guard) = self.lock() {
            guard.get_name_id(name)
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Retrieves a node by its ID from the blackboard.
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String> {
        if let Ok(guard) = self.lock() {
            guard.get_node_by_id(id)
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }

    /// Returns a copy of all name-to-ID mappings in the root namespace.
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String> {
        if let Ok(guard) = self.lock() {
            guard.get_names_copy()
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `AroraBlackboardTrait` for `Arc<Mutex<ArcAroraBlackboard>>`.
///
/// This implementation provides methods to interact with the Arc blackboard,
/// such as printing items and setting items with values.
impl ArcAroraBlackboardTrait for Arc<Mutex<ArcAroraBlackboard>> {
    /// Sets an item into the blackboard with the given value, ID, and optional name.
    ///
    /// If the item already exists, its value is updated. If it doesn't exist,
    /// a new item is created with the given name.
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
        full_path: Option<&String>,
    ) -> Result<bool, String> {
        if let Ok(mut guard) = self.lock() {
            guard.set_bb_item(value, item_id, name, full_path)
        } else {
            Err("Failed to lock mutex".to_string())
        }
    }
}

/// Implementation of `NamespacedSetterTrait` for `Arc<Mutex<ArcAroraBlackboard>>`.
///
/// This trait allows the Arc blackboard to be used to set items in a namespaced manner.
impl ArcNamespacedSetterTrait for Arc<Mutex<ArcAroraBlackboard>> {
    /// Returns a pointer to the self so that we can pass it around.
    fn get_blackboard(&self) -> Result<Arc<Mutex<ArcAroraBlackboard>>, String> {
        Ok(self.clone())
    }
}

/// Implementation of `Display` trait for `ArcAroraBlackboard`.
///
/// This provides a formatted string representation of the blackboard and its contents.
impl Display for ArcAroraBlackboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            TreeFormattable::format_tree(self, false).trim_end()
        )
    }
}

/// Implementation of `TreeFormattable` trait for `ArcAroraBlackboard`.
impl TreeFormattable for ArcAroraBlackboard {
    fn format_tree(&self, show_ids: bool) -> String {
        let mut output = String::new();
        let name = self.get_current_name_copy();
        if name.is_err() {
            return format!("Error getting current name: {}", name.unwrap_err());
        }

        output.push_str(&format!("{:?}' Namespace Tree:\n", name.unwrap()));
        let names = self.get_names_copy();
        if names.is_err() {
            return format!("Error getting names: {}", names.unwrap_err());
        }
        for (name, ref_id) in names.unwrap() {
            self._format_tree_recursively(&name, &ref_id, 1, show_ids, &mut output);
        }
        output
    }
}

/// Implementation of `TreeFormattable` trait for `Arc<Mutex<ArcAroraBlackboard>>`.
impl TreeFormattable for Arc<Mutex<ArcAroraBlackboard>> {
    fn format_tree(&self, show_ids: bool) -> String {
        if let Ok(bb_guard) = self.lock() {
            TreeFormattable::format_tree(&*bb_guard, show_ids)
        } else {
            "Failed to lock blackboard mutex".to_string()
        }
    }
}

/// Implementation of `ItemsFormattable` trait for `ArcAroraBlackboard`.
impl ItemsFormattable for ArcAroraBlackboard {
    fn format_items(&self, show_ids: bool) -> String {
        let mut output = String::new();
        output.push_str(&format!("Blackboard Items for {}:\n", self.id));

        for (_id, node_arc) in &self.items {
            if let Ok(node_guard) = node_arc.lock() {
                match &*node_guard {
                    ArcABBNode::Path(path) => {
                        if let Ok(name) = path.get_current_name_copy() {
                            let id_ref = path.get_id_ref();
                            if id_ref.is_err() {
                                output.push_str(&format!(
                                    "Error getting ID for path node: {}\n",
                                    id_ref.unwrap_err()
                                ));
                                continue;
                            }

                            if show_ids {
                                output.push_str(&format!(
                                    "{} -> Path: {}\n",
                                    id_ref.unwrap(),
                                    name
                                ));
                            } else {
                                output.push_str(&format!("Path: {}\n", name));
                            }
                        }
                    }
                    ArcABBNode::Item(item) => {
                        if let Ok(name) = item.get_current_name_copy() {
                            let id_ref = item.get_id_ref();
                            if id_ref.is_err() {
                                output.push_str(&format!(
                                    "Error getting ID for path node: {}\n",
                                    id_ref.unwrap_err()
                                ));
                                continue;
                            }

                            if let Some(value) = item.get_value() {
                                if show_ids {
                                    output.push_str(&format!(
                                        "{} -> Item: {} : {} = {}\n",
                                        id_ref.unwrap(),
                                        name,
                                        item.get_value_type(),
                                        value
                                    ));
                                } else {
                                    output.push_str(&format!(
                                        "Item: {} : {} = {}\n",
                                        name,
                                        item.get_value_type(),
                                        value
                                    ));
                                }
                            } else {
                                if show_ids {
                                    output.push_str(&format!(
                                        "{} -> Item: {} : {} = (no value)\n",
                                        id_ref.unwrap(),
                                        name,
                                        item.get_value_type()
                                    ));
                                } else {
                                    output.push_str(&format!(
                                        "Item: {} : {} = (no value)\n",
                                        name,
                                        item.get_value_type()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
        output.trim_end().to_string()
    }
}

/// Implementation of `ItemsFormattable` trait for `Arc<Mutex<ArcAroraBlackboard>>`.
impl ItemsFormattable for Arc<Mutex<ArcAroraBlackboard>> {
    fn format_items(&self, show_ids: bool) -> String {
        if let Ok(bb_guard) = self.lock() {
            ItemsFormattable::format_items(&*bb_guard, show_ids)
        } else {
            "Failed to lock blackboard mutex".to_string()
        }
    }
}

/// Trait for types that can be serialized to JSON.
pub trait JsonSerializable {
    /// Converts the implementation to a JSON representation.
    ///
    /// # Returns
    /// A `Result<serde_json::Value, String>` containing the JSON representation.
    fn to_json(&self) -> Result<serde_json::Value, String>;
}

/// Implementation of JsonSerializable for `Arc<Mutex<ArcAroraBlackboard>>`.
impl JsonSerializable for Arc<Mutex<ArcAroraBlackboard>> {
    fn to_json(&self) -> Result<serde_json::Value, String> {
        match self.lock() {
            Ok(bb_guard) => bb_guard.to_json(),
            Err(_) => Err("Failed to lock blackboard mutex".to_string()),
        }
    }
}
