//! This module defines a blackboard structure that can hold various types of data and allows for dynamic updates.
//! The blackboard is designed to be thread-safe and can be shared across multiple threads.
//! It uses a unique ID for each item to ensure that items can be referenced and updated without ambiguity.
//! It includes a namespace path feature which allows to organize, set and access items in a hierarchical manner.
//! While all storage is done in a single HashMap, the namespace path allows to create a tree-like structure where its leafs reference the actual data.
//!
//! The code below defines various types identifiable by the prefix ABB which stands for AroraBlackBoard.
//! In particular, it contains the following main types:
//! - RcBlackboard: The main struct representing the blackboard, which contains a collection of nodes indexed by their IDs
//! - BBNode: An abstract node in the blackboard structure
//! - Each BB item is encapsulated in an BBNode, which can be either a path or an item
//! - If it is a path, it can contain other nodes, allowing for a hierarchical namespaced structure
//! - If it is an item, it contains a value and its type
//! - RcBBPathNode: A node that represents a path in the blackboard structure
//! - BBItemNode: A node that represents a value item in the blackboard structure

use arora_schema::{gen_bb_uuid, value::Value};
use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Display},
    rc::{Rc, Weak},
};
use uuid::Uuid;

use super::{RcBBNode, RcBBPathNode};
use crate::{
    traits::{
        BBNodeTrait, BBPathNodeTrait, BlackboardTrait, ItemsFormattable, JsonSerializable,
        TreeFormattable,
    },
    BBItemNode,
};

use crate::{
    adt,
    rc_bb::{NamespacedSetterTrait, RcBBPathNodeTrait},
};

/// A struct representing the blackboard, which contains a collection of nodes indexed by their IDs.
/// This struct is designed to be thread-safe and can be shared across threads.
/// The blackboard is initialized with a path node with the same ID as the blackboard itself, which serves as the root namespace.
/// This allows for a hierarchical structure where nodes can be added or removed dynamically.
#[derive(Debug)]
pub struct RcBlackboard {
    /// The ID of the blackboard, which is used as the root path node.
    /// Allows to distinguish between multiple BBs in a system.
    id: Uuid,
    /// This HashMap is the owner of all nodes.
    items: HashMap<Uuid, Rc<RefCell<RcBBNode>>>,
    /// A self-reference that can be passed around without cloning.
    self_ref: Option<Weak<RefCell<RcBlackboard>>>,
}

impl RcBlackboard {
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
    /// An `Arc<Mutex<RcBlackboard>>` containing the newly created blackboard
    pub fn new<S: ToString>(name: S) -> Rc<RefCell<Self>> {
        let id = gen_bb_uuid();
        // Step 1: Create a path node with a temporary bb placeholder
        let path_node = RcBBPathNode::create_root(&name.to_string());

        // Step 2: Create a struct with the wrapped path node
        let mut bb: RcBlackboard = Self {
            id,
            items: HashMap::new(),
            self_ref: None,
        };

        // Initialize the items HashMap with the path node that will act as the root, using the BB id as key
        bb.items
            .insert(id, Rc::new(RefCell::new(RcBBNode::Path(path_node))));

        // Create Rc<RefCell<>> wrapper
        let rc_bb = Rc::new(RefCell::new(bb));

        // Create a Weak reference to copy over to the path node
        let weak_bb = Rc::downgrade(&rc_bb);

        // Now update the bb field with the weak reference
        {
            let mut bb_mut = rc_bb.borrow_mut();
            if let Some(node_ref) = bb_mut.items.get_mut(&id) {
                if let RcBBNode::Path(ref mut path) = *node_ref.borrow_mut() {
                    // Set the weak reference directly
                    path.set_bb(weak_bb.clone());
                } else {
                    panic!("Base node is not a RcBBPathNode");
                }
            }
            bb_mut.self_ref = Some(weak_bb);
        }

        rc_bb
    }

    /// Removes an item from the blackboard by its path.
    ///
    /// This method removes the item from both the parent path node's name mapping
    /// and from the blackboard's items HashMap. Returns a vector of all removed IDs.
    ///
    /// # Arguments
    /// * `path` - The path to the item to remove
    ///
    /// # Returns
    /// A `Result<Vec<Uuid>, String>` containing all removed IDs or an error message
    pub fn remove_item<S: ToString + ?Sized>(&mut self, path: &S) -> Result<Vec<Uuid>, String> {
        // First, get the node to find its ID
        let node_opt = self.get(path)?;
        if let Some(node_ref) = node_opt {
            let id = node_ref.borrow().get_id_copy()?;
            self.remove_item_by_id(&id)
        } else {
            Err(format!("Item '{}' not found", path.to_string()))
        }
    }

    /// Removes an item from the blackboard by its ID.
    ///
    /// This method removes the item from both the parent path node's name mapping
    /// and from the blackboard's items HashMap. If the item is a path node,
    /// it recursively removes all children and returns all removed IDs.
    ///
    /// # Arguments
    /// * `id` - The ID of the item to remove
    ///
    /// # Returns
    /// A `Result<Vec<Uuid>, String>` containing all removed IDs or an error message
    pub fn remove_item_by_id(&mut self, id: &Uuid) -> Result<Vec<Uuid>, String> {
        // Check if the node exists
        if !self.items.contains_key(id) {
            return Err(format!("Item with ID '{}' not found", id));
        }

        // Vector to accumulate all removed IDs
        let mut removed_ids = Vec::new();

        // Get the node to find its name, parent path, and children (if it's a path node)
        let (full_path, name, child_ids) = {
            let node_ref = self.items.get(id).cloned();
            if let Some(node) = node_ref {
                let node_borrow = node.borrow();
                let full_path = node_borrow.get_full_path()?;
                let name = node_borrow.get_current_name_copy()?;
                let is_path = node_borrow.is_path()?;

                // If this is a path node, collect all child IDs
                let child_ids = if is_path {
                    if let RcBBNode::Path(ref path_node) = *node_borrow {
                        path_node.get_names_copy()?.values().cloned().collect()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };

                (full_path, name, child_ids)
            } else {
                return Err(format!("Failed to get node with ID '{}'", id));
            }
        };

        // Now recursively remove all children (borrow has been dropped)
        for child_id in child_ids {
            let child_removed_ids = self.remove_item_by_id(&child_id)?;
            removed_ids.extend(child_removed_ids);
        }

        // If this is not the root node, remove it from its parent's names mapping
        if *id != self.id {
            // Parse the path to find the parent
            let path_parts: Vec<&str> = full_path.split('.').collect();
            if path_parts.len() > 1 {
                // Get parent path
                let parent_path = path_parts[..path_parts.len() - 1].join(".");
                if let Some(parent_node_ref) = self.get(&parent_path)? {
                    if let RcBBNode::Path(ref mut parent_path_node) = *parent_node_ref.borrow_mut()
                    {
                        parent_path_node.remove_name(&name)?;
                    }
                }
            } else {
                // This is a direct child of root
                if let Some(root_node_ref) = self.items.get(&self.id) {
                    if let RcBBNode::Path(ref mut root_path_node) = *root_node_ref.borrow_mut() {
                        root_path_node.remove_name(&name)?;
                    }
                }
            }
        }

        // Remove the item from the items HashMap and add to removed list
        self.items.remove(id);
        removed_ids.push(*id);

        Ok(removed_ids)
    }
}

/// Implementation of JsonSerializable for RcBlackboard.
impl JsonSerializable for RcBlackboard {
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

        for (id, node_ref) in &self.items {
            match &*node_ref.borrow() {
                RcBBNode::Path(path) => {
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
                    node_data.push((id.to_string(), true, name, None, None, names, graph_node_id));
                }
                RcBBNode::Item(item) => {
                    if let Ok(name) = item.get_current_name_copy() {
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

/// Implementation of `BBNodeTrait` for `RcBlackboard`
///
/// This implementation allows the blackboard itself to be treated as a node in the
/// hierarchy, with the blackboard ID serving as its name and identifier.
impl BBNodeTrait for RcBlackboard {
    /// Returns a reference to the ID of the blackboard.
    ///
    /// # Returns
    /// A `Result` containing a reference to the blackboard's ID
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        Ok(&self.id)
    }

    /// Determines if this node is a path node, otherwise it's an item node.
    ///
    /// For `RcBlackboard`, this always returns true since a blackboard
    /// is conceptually a root path node.
    ///
    /// # Returns
    /// A `Result` containing `true` for `RcBlackboard`
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
            if let RcBBNode::Path(path) = &*base_node.borrow() {
                return path.get_full_path();
            }
            return Err("Base node is not a RcBBPathNode".to_string());
        }
        Err(format!(
            "Base node for blackboard {} not found in items",
            self.id
        ))
    }
}

/// Implementation of `BBPathNodeTrait` for `RcBlackboard`
///
/// This implementation allows the blackboard to be treated as a path node.
impl BBPathNodeTrait for RcBlackboard {
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
    fn contains(&self, name: &str) -> Result<bool, String> {
        if let Some(base_node) = self.items.get(&self.id) {
            if let RcBBNode::Path(path) = &*base_node.borrow() {
                return path.contains(name);
            }
        }
        // Return false if the base node isn't found or isn't a RcBBPathNode
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
            if let RcBBNode::Path(path) = &mut *base_node.borrow_mut() {
                return path.insert(name, id);
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
    fn get_name_id(&self, name: &str) -> Result<Option<Uuid>, String> {
        if let Some(base_node) = self.items.get(&self.id) {
            if let RcBBNode::Path(path) = &*base_node.borrow() {
                return path.get_name_id(name);
            }
        }
        // Return None if the base node isn't found or isn't a RcBBPathNode
        Ok(None)
    }

    /// Returns a copy of all name-to-ID mappings in the root namespace.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap<String, String>` with name-to-ID mappings
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String> {
        if let Some(base_node) = self.items.get(&self.id) {
            if let RcBBNode::Path(path) = &*base_node.borrow() {
                return path.get_names_copy();
            }
        }
        // Return empty HashMap if the base node isn't found or isn't a RcBBPathNode
        Ok(HashMap::new())
    }

    fn _format_tree_recursively(
        &self,
        name: &str,
        id: &Uuid,
        depth: usize,
        show_ids: bool,
        out: &mut String,
    ) {
        // Get the base node and delegate to its implementation
        if let Some(base_node) = self.items.get(&self.id) {
            if let RcBBNode::Path(path) = &*base_node.borrow() {
                RcBBPathNodeTrait::_format_tree_recursively(path, name, id, depth, show_ids, out);
                return;
            }
        }
        out.push_str("Failed to format tree for blackboard\n");
    }
}

/// Implementation of `RcBBPathNodeTrait` for `RcBlackboard`.
/// This implementation allows the blackboard to be treated as an arc path node.
/// This is useful for traversing the blackboard structure.
impl RcBBPathNodeTrait for RcBlackboard {
    /// Retrieves a node by its ID from the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result` containing an `Option<Arc<Mutex<BBNode>>>` with the node if found, or `None` if not found
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Rc<RefCell<RcBBNode>>>, String> {
        Ok(self.items.get(id).cloned())
    }
}

/// Implementation of `RcBlackboardTrait` for `RcBlackboard`.
///
/// This implementation provides methods to interact with the blackboard,
/// such as printing items and setting items with values.
impl BlackboardTrait for RcBlackboard {
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
        full_path: Option<&str>,
    ) -> Result<bool, String> {
        if self.items.contains_key(item_id) {
            if let Some(node_ref) = self.items.get(item_id) {
                // Items exists, so get its existing item and update its value

                // Check that the node is an Item node
                if let RcBBNode::Item(ref mut item) = *node_ref.borrow_mut() {
                    // Confirm item_id matches the existing item
                    if item_id != item.get_id_ref()? {
                        return Err(format!("Item ID does not match existing item ID, existing item: {}, new item: {}",
                                item.get_id_ref()?, item_id));
                    }

                    let value =
                        adt::utils::quick_convert_to_type(&value, item.get_value_type(), false);

                    // Check if the value is Some
                    if item.get_value().is_some() {
                        if adt::utils::is_compatible_type(&value, item.get_value_type()) {
                            // Set the new value
                            item.set_value(value);
                        } else {
                            return Err(format!("Incompatible value type for existing item {}: expected {:?}, got {:?}",
                                    item_id, item.get_value_type(), value));
                        }
                        Ok(true)
                    } else {
                        Err(format!("Existing item {} has no value to set", item_id))
                    }
                } else {
                    Err(format!(
                        "Tried to set an existing id {} with a new value but it was not an Item",
                        item_id
                    ))
                }
            } else {
                Err(format!(
                    "Tried to set an existing id {} with a new value but it was not found",
                    item_id
                ))
            }
        } else {
            if name.as_ref().is_none() || name.as_ref().unwrap().is_empty() {
                return Err(
                    "Name cannot be empty when adding a new item to the blackboard".to_string(),
                );
            }

            let item_result = BBItemNode::from_value(
                name.as_ref().unwrap(),
                value,
                *item_id,
                full_path.unwrap_or(name.as_ref().unwrap()),
            );

            match item_result {
                Ok(Some(item)) => {
                    // Create the node and insert it into items HashMap (as owner)
                    let node: RcBBNode = RcBBNode::Item(item);
                    let node_ref = Rc::new(RefCell::new(node));
                    self.items.insert(*item_id, node_ref);
                    Ok(true)
                }
                Ok(None) => Err("Failed to create ABBItemNode: returned None".to_string()),
                Err(e) => Err(format!("Failed to create ABBItemNode: {}", e)),
            }
        }
    }
}

/// Implementation of `NamespacedSetterTrait` for `RcBlackboard`.
///
/// This trait allows the blackboard to be used to set items in a namespaced manner.
impl NamespacedSetterTrait for RcBlackboard {
    /// Returns a reference to the blackboard itself.
    ///
    /// # Returns
    /// A `Result<Arc<Mutex<RcBlackboard>>, String>` containing the reference to the blackboard or an error
    fn get_blackboard(&self) -> Result<Weak<RefCell<RcBlackboard>>, String> {
        if let Some(ref weak_self) = self.self_ref {
            Ok(weak_self.clone())
        } else {
            Err("No self reference available in blackboard".to_string())
        }
    }

    /// Insert an entry into the blackboard's item hash.
    /// # Arguments
    /// * `id` - The unique identifier for the node
    /// * `item` - The node to insert
    ///
    /// # Note
    /// This method is intended to be used internally by the blackboard system
    fn _insert_entry(&mut self, id: Uuid, item: Rc<RefCell<RcBBNode>>) {
        // Insert the item into the items HashMap
        self.items.insert(id, item);
    }
}

/// Implementation of `Display` trait for `RcBlackboard`.
///
/// This provides a formatted string representation of the blackboard and its contents.
impl Display for RcBlackboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            TreeFormattable::format_tree(self, false).trim_end()
        )
    }
}

/// Implementation of `TreeFormattable` trait for `RcBlackboard`.
impl TreeFormattable for RcBlackboard {
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
            RcBBPathNodeTrait::_format_tree_recursively(
                self,
                &name,
                &ref_id,
                1,
                show_ids,
                &mut output,
            );
        }
        output
    }
}

/// Implementation of `ItemsFormattable` trait for `RcBlackboard`.
impl ItemsFormattable for RcBlackboard {
    fn format_items(&self, show_ids: bool) -> String {
        let mut output = String::new();
        output.push_str(&format!("Blackboard Items for {}:\n", self.id));

        for node_ref in self.items.values() {
            match &*node_ref.borrow() {
                RcBBNode::Path(path) => {
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
                            output.push_str(&format!("{} -> Path: {}\n", id_ref.unwrap(), name));
                        } else {
                            output.push_str(&format!("Path: {}\n", name));
                        }
                    }
                }
                RcBBNode::Item(item) => {
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
                        } else if show_ids {
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
        output.trim_end().to_string()
    }
}
