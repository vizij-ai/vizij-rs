//! This module defines the traits used in the Arora blackboard system.
//! These traits define the behavior of nodes, paths, and the blackboard itself,
//! and provide a common interface for interacting with the system regardless of
//! the specific implementation details.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use arora_schema::value::Value;
use arora_schema::{
    gen_bb_uuid,
    keyvalue::{KeyValue, KeyValueField},
};
use uuid::Uuid;

use crate::{adt, ArcAroraBlackboard};

use super::{split_path, ABBItemNode, ArcABBNode, ArcABBPathNode};

/// Result of validating whether a path exists and its type.
///
/// This enum is used by methods that check paths in the blackboard,
/// providing information about whether the path exists and what type it is.
pub enum CheckPathResult {
    /// The path exists and is an item node, containing the ID of the item
    IsItem(Uuid),
    /// The path exists and is a path node, containing the ID of the path
    IsPath(Uuid),
    /// The path does not exist
    None(),
}

/// Base trait for every node type used by the blackboard.
///
/// This trait defines the basic operations that all nodes must support,
/// regardless of whether they are path nodes or item nodes.
/// It also provides methods to get copies of the node's name and ID,
/// which is necessary when the node becomes wrapped in an `Arc<Mutex<>>`.
pub trait ABBNodeTrait {
    /// Get the ID of the node as a reference.
    ///
    /// # Returns
    /// A `Result<&String, String>` containing a reference to the node's ID, or an error message
    fn get_id_ref(&self) -> Result<&Uuid, String>;

    /// Determines if this node is a path node.
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating whether this is a path node, or an error message
    fn is_path(&self) -> Result<bool, String>;

    /// Get the current name of the node as a copy.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing a copy of the node's name, or an error message
    fn get_current_name_copy(&self) -> Result<String, String>;

    /// Get the ID of the node as a copy.
    ///
    /// # Returns
    /// A `Result<String, String>` containing a copy of the node's ID, or an error message
    fn get_id_copy(&self) -> Result<Uuid, String>;

    /// Get the full path of the node as a string.
    /// This is the full namespace path of the node,
    /// including all parent namespaces.
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String>;
}

/// This is the main trait for objects that can be used as paths in the blackboard.
///
/// It provides methods to insert, retrieve, and check items in the path,
/// as well as utility methods for navigating the hierarchy.
pub trait ArcABBPathNodeTrait: ABBNodeTrait + TreeFormattable {
    /// Check if the given name exists in this path.
    ///
    /// # Arguments
    /// * `name` - The name to check for
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating if an entry with the given name exists, or an error message
    fn contains(&self, name: &str) -> Result<bool, String>;

    /// Insert a new name-to-ID mapping in this path.
    ///
    /// # Arguments
    /// * `name` - The name to associate with the ID
    /// * `id` - The ID to map the name to
    ///
    /// # Returns
    /// A `Result<(), String>` indicating success or an error message
    fn insert(&mut self, name: String, id: Uuid) -> Result<(), String>;

    /// Get the ID associated with a name in this path.
    ///
    /// # Arguments
    /// * `name` - The name to look up
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the ID if found, or an error message
    fn get_name_id(&self, name: &str) -> Result<Option<Uuid>, String>;

    /// Retrieve a node by its ID from the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result<Option<Arc<Mutex<ABBNode>>>, String>` containing the node if found, or an error message
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String>;

    /// Return a copy of the map of item names and IDs in this path.
    ///
    /// # Returns
    /// A `Result<HashMap<String, String>, String>` containing name-to-ID mappings, or an error message
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String>;

    /// Helper to allow passing a full String namespace path (dot.separated) directly.
    ///
    /// # Arguments
    /// * `path` - The path as a dot-separated string
    ///
    /// # Returns
    /// A `Result<Option<Arc<Mutex<ABBNode>>>, String>` containing the node if found, or an error message
    fn get<S: ToString + ?Sized>(
        &self,
        path: &S,
    ) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String> {
        let names = split_path(&path.to_string());
        self.get_by_names(names)
    }

    /// Helper to return the ID of an Item node directly by its path
    fn get_path_id<S: ToString + ?Sized>(&self, path: &S) -> Result<Option<Uuid>, String> {
        if let Ok(Some(node_arc)) = self.get(path) {
            if let Ok(node_guard) = node_arc.lock() {
                match &*node_guard {
                    ArcABBNode::Path(path_node) => {
                        // Return the ID of the path node
                        return path_node.get_id_copy().map(Some);
                    }
                    ArcABBNode::Item(item_node) => {
                        // The node is an Item node, return its ID
                        return item_node.get_id_ref().map(|id_ref| Some(*id_ref));
                    }
                }
            } else {
                return Err("Failed to lock mutex".to_string());
            }
        }
        Ok(None)
    }

    /// Helper to return the value of an Item node directly by its path
    fn get_value<S: ToString + ?Sized>(&self, path: &S) -> Result<Option<Value>, String> {
        if let Ok(Some(node_arc)) = self.get(path) {
            if let Ok(node_guard) = node_arc.lock() {
                if let ArcABBNode::Item(item_node) = &*node_guard {
                    // Return the value of the item node
                    return Ok(item_node.get_value().cloned());
                } else {
                    // The node is not an Item node
                    return Ok(None);
                }
            } else {
                return Err("Failed to lock mutex".to_string());
            }
        }
        Ok(None)
    }

    /// Get a formatted string representation of the current namespace items tree
    fn format_tree(&self, show_ids: bool) -> String {
        let mut output = String::new();

        match self.get_current_name_copy() {
            Err(e) => {
                output.push_str(&format!("Failed to get current name: {}\n", e));
            }
            Ok(curr_name) => {
                output.push_str(&format!("{:?}' Namespace Tree:\n", curr_name));
                match self.get_names_copy() {
                    Err(e) => {
                        output.push_str(&format!("Failed to get names: {}\n", e));
                        return output;
                    }
                    Ok(names) => {
                        for (name, ref_id) in names {
                            self._format_tree_recursively(&name, &ref_id, 1, show_ids, &mut output);
                        }
                    }
                }
            }
        }
        output
    }

    /// Recursive helper function to format the item tree
    fn _format_tree_recursively(
        &self,
        name: &str,
        id: &Uuid,
        depth: usize,
        show_ids: bool,
        output: &mut String,
    ) {
        if let Ok(Some(node_arc)) = self.get_node_by_id(id) {
            // Lock the mutex to get access to the node inside
            if let Ok(node_guard) = node_arc.lock() {
                // Now we can match on the dereferenced guard
                match &*node_guard {
                    ArcABBNode::Path(path) => {
                        match path.get_full_path() {
                            Err(e) => output.push_str(&format!(
                                "{}Failed to get full path for '{}': {}\n",
                                " ".repeat(depth * 2),
                                name,
                                e
                            )),
                            Ok(fp) => {
                                if show_ids {
                                    match path.get_id_ref() {
                                        Err(e) => output.push_str(&format!(
                                            "{}Failed to get ID for path '{}': {}\n",
                                            " ".repeat(depth * 2),
                                            name,
                                            e
                                        )),
                                        Ok(id_ref) => output.push_str(&format!(
                                            "{}Namespace: {} (ID: {}, fullpath: {})\n",
                                            " ".repeat(depth * 2),
                                            name,
                                            id_ref,
                                            fp
                                        )),
                                    }
                                } else {
                                    output.push_str(&format!(
                                        "{}Namespace: {} (fullpath: {})\n",
                                        " ".repeat(depth * 2),
                                        name,
                                        fp
                                    ));
                                }
                            }
                        }

                        match path.get_names_copy() {
                            Err(e) => output.push_str(&format!(
                                "{}Failed to get names for path '{}': {}\n",
                                " ".repeat(depth * 2),
                                name,
                                e
                            )),
                            Ok(child_map) => {
                                for (child_name, ref_id) in child_map {
                                    self._format_tree_recursively(
                                        &child_name,
                                        &ref_id,
                                        depth + 1,
                                        show_ids,
                                        output,
                                    );
                                }
                            }
                        }
                    }
                    ArcABBNode::Item(item) => match item.get_current_name_copy() {
                        Err(e) => output.push_str(&format!(
                            "{}Failed to get name for item '{}': {}\n",
                            " ".repeat(depth * 2),
                            name,
                            e
                        )),
                        Ok(item_name) => match item.get_full_path() {
                            Err(e) => output.push_str(&format!(
                                "{}Failed to get full path for item '{}': {}\n",
                                " ".repeat(depth * 2),
                                name,
                                e
                            )),
                            Ok(fp) => {
                                if let Some(try_item) = item.get_value() {
                                    if show_ids {
                                        match item.get_id_ref() {
                                            Err(e) => output.push_str(&format!(
                                                "{}Failed to get ID for item '{}': {}\n",
                                                " ".repeat(depth * 2),
                                                name,
                                                e
                                            )),
                                            Ok(id_ref) => output.push_str(&format!(
                                                "{}Item: {:?} = {} (ID: {}, fullpath: {})\n",
                                                " ".repeat(depth * 2),
                                                item_name,
                                                try_item,
                                                id_ref,
                                                fp
                                            )),
                                        }
                                    } else {
                                        output.push_str(&format!(
                                            "{}Item: {:?} = {} (fullpath: {})\n",
                                            " ".repeat(depth * 2),
                                            item_name,
                                            try_item,
                                            fp
                                        ));
                                    }
                                } else if show_ids {
                                    match item.get_id_ref() {
                                        Err(e) => output.push_str(&format!(
                                            "{}Failed to get ID for item '{}': {}\n",
                                            " ".repeat(depth * 2),
                                            name,
                                            e
                                        )),
                                        Ok(id_ref) => output.push_str(&format!(
                                            "{}Item: {:?} (ID: {}, fullpath: {})\n",
                                            " ".repeat(depth * 2),
                                            item_name,
                                            id_ref,
                                            fp
                                        )),
                                    }
                                } else {
                                    output.push_str(&format!(
                                        "{}Item: {:?} (fullpath: {})\n",
                                        " ".repeat(depth * 2),
                                        item_name,
                                        fp
                                    ));
                                }
                            }
                        },
                    },
                }
            } else {
                output.push_str(&format!(
                    "{}Failed to lock node for '{}': {}\n",
                    " ".repeat(depth * 2),
                    name,
                    "Lock failed"
                ));
            }
        } else {
            output.push_str(&format!(
                "{}Node not found: {}\n",
                " ".repeat(depth * 2),
                id
            ));
        }
    }

    /// Get the node referenced by the given path parts, by traversing the namespace tree
    /// This is the main getter function used to retrieve nodes by path
    fn get_by_names(&self, names: Vec<String>) -> Result<Option<Arc<Mutex<ArcABBNode>>>, String> {
        if names.len() == 1 {
            // quick path in case we only have one name
            let name = &names[0];
            if self.contains(name)? {
                if let Ok(Some(id)) = self.get_name_id(name) {
                    return self.get_node_by_id(&id);
                }
            }
            return Ok(None);
        }
        let mut current_node_id: Option<Uuid> = None;
        for (i, name_part) in names.iter().enumerate() {
            let node_id = if current_node_id.is_none() {
                // Check in root
                self.get_name_id(name_part)?
            } else if let Some(path_id) = &current_node_id {
                // Check in current node
                if let Ok(Some(node_arc)) = self.get_node_by_id(path_id) {
                    if let Ok(node_guard) = node_arc.lock() {
                        if let ArcABBNode::Path(path) = &*node_guard {
                            path.get_name_id(name_part)?
                        } else {
                            // Current node is not a path node, but may be a KeyValue node
                            None
                        }
                    } else {
                        return Err("Failed to lock mutex".to_string());
                    }
                } else {
                    // Node not found
                    None
                }
            } else {
                // Component was not found in root
                None
            };

            if let Some(id) = node_id {
                if i == names.len() - 1 {
                    // Found the final component - return the Arc directly
                    return self.get_node_by_id(&id);
                } else {
                    // Continue traversing
                    current_node_id = Some(id);
                }
            } else {
                // Component not found
                return Ok(None);
            }
        }

        Ok(None)
    }
    /// Get the specified node by its path as a KeyValue object.
    ///
    /// # Arguments
    /// * `path` - The path as a string
    /// # Returns
    /// A `Result<Option<KeyValue>, String>` containing the KeyValue if found, or an error message
    fn get_keyvalue<S: ToString + ?Sized>(&self, path: &S) -> Result<Option<KeyValue>, String> {
        // Call the internal method with by_id set to false
        let node = self.get(path)?;

        // If the node doesn't exist, return None
        let node_arc = match node {
            Some(node) => node,
            None => return Ok(None),
        };

        self.get_keyvalue_from_node_arc(node_arc)
    }
    fn get_keyvalue_by_id(&self, id: &Uuid) -> Result<Option<KeyValue>, String> {
        // First check if the path exists
        let node = self.get_node_by_id(id)?;

        // If the node doesn't exist, return None
        let node_arc = match node {
            Some(node) => node,
            None => return Ok(None),
        };

        self.get_keyvalue_from_node_arc(node_arc)
    }

    fn get_keyvalue_from_node_arc(
        &self,
        node_arc: Arc<Mutex<ArcABBNode>>,
    ) -> Result<Option<KeyValue>, String> {
        let id: Uuid = node_arc.get_id_copy()?;
        let is_path = node_arc.is_path()?;
        let mut fields = HashMap::new();
        if is_path {
            // This is a namespace node, we need to recursively build a KeyValue structure

            // Get all the items in this path
            let names: HashMap<String, Uuid> = {
                let node_guard = node_arc
                    .lock()
                    .map_err(|_| "Failed to lock node for names access")?;
                if let Some(path_node) = node_guard.as_path() {
                    path_node.get_names_copy()?
                } else {
                    return Err("Failed to get path node".to_string());
                }
            };

            // For each child, recursively build the KeyValue structure
            for (name, child_id) in names {
                let child_node = self.get_node_by_id(&child_id)?;

                let child_node_arc = match child_node {
                    Some(node) => node,
                    None => continue, // Skip if the child node doesn't exist
                };

                // Check if this child is an item or path
                if child_node_arc.is_path()? {
                    // It's a path, so recursively get its KeyValue
                    if let Some(child_kv) = self.get_keyvalue_by_id(&child_id)? {
                        // Add this to our fields
                        fields.insert(
                            name.clone(),
                            KeyValueField::new_with_id(name, child_id, Value::KeyValue(child_kv)),
                        );
                    }
                } else {
                    let child_node_guard = child_node_arc
                        .lock()
                        .map_err(|_| "Failed to lock child node for item access")?;
                    // It's an item, get its value
                    let item_node = match child_node_guard.as_item() {
                        Some(item) => item,
                        None => return Err("Node is not an item".to_string()),
                    };

                    // Add this item to our fields
                    fields.insert(
                        name.clone(),
                        KeyValueField::new_with_id_and_option(
                            name,
                            child_id,
                            item_node.get_value().cloned(),
                        ),
                    );
                }
            }
        } else {
            let node_guard = node_arc
                .lock()
                .map_err(|_| "Failed to lock node for names access")?;
            let item_node = match node_guard.as_item() {
                Some(item) => item,
                None => return Err("Node is not an item".to_string()),
            };

            let node_name = item_node.get_current_name_copy()?;

            // Create a simple KeyValue with just this value
            fields.insert(
                node_name.clone(),
                KeyValueField::new_with_id_and_option(
                    node_name,
                    item_node.get_id_copy()?,
                    item_node.get_value().cloned(),
                ),
            );
        }

        let kv = KeyValue { id, fields };
        Ok(Some(kv))
    }

    fn as_keyvalue(&self) -> Result<Option<KeyValue>, String> {
        // This method is used to convert the node into a KeyValue structure
        // It will return None if the node is not a path or item
        if let Ok(Some(node_arc)) = self.get_node_by_id(&self.get_id_copy()?) {
            self.get_keyvalue_from_node_arc(node_arc)
        } else {
            Ok(None)
        }
    }
}

/// Define a trait for blackboard item manipulation.
///
/// This allows both the BB main object and also path nodes to act as a blackboard manipulator.
/// The difference is that depending on where we are, the item path will be different:
/// - Manipulating items on the BB object means manipulating at the root path
/// - Manipulating items on a path node means manipulating at the current path
pub trait ArcAroraBlackboardTrait: ArcABBPathNodeTrait + ItemsFormattable {
    /// Sets an item into the blackboard, given a Value and an ID.
    ///
    /// The item ID is a string that will be used as the item hash for fast retrieval.
    /// This will create a new ABBItemNode object and insert it into the blackboard associated with the id.
    /// The name is necessary in case the item does not exist yet, because we need it for the ABBItemNode.
    ///
    /// # Arguments
    /// * `value` - The value to set
    /// * `item_id` - The ID for the item
    /// * `name` - Optional name for the item (required when creating a new item)
    ///
    /// # Returns
    /// `Result<bool, String>` indicating success or an error message
    fn set_bb_item(
        &mut self,
        value: Value,
        item_id: &Uuid,
        name: Option<String>,
        full_path: Option<&str>,
    ) -> Result<bool, String>;

    /// Syntactic sugar to set an existing item into the blackboard given we don't have to provide a name.
    ///
    /// # Arguments
    /// * `value` - The value to set
    /// * `item_id` - The ID for the existing item
    ///
    /// # Returns
    /// `Result<bool, String>` indicating success or an error message
    fn set_existing_bb_item(&mut self, value: Value, item_id: &Uuid) -> Result<bool, String> {
        self.set_bb_item(value, item_id, None, None)
    }
}

/// Define a trait for setting items in a namespaced manner.
///
/// This trait is used to set items in the blackboard, either in the root or in a path node.
/// It provides methods for navigating the namespace hierarchy and setting values at specific paths.
pub trait ArcNamespacedSetterTrait: ArcAroraBlackboardTrait {
    /// Get a reference to the blackboard.
    ///
    /// Because this trait can be implemented by both the blackboard and path nodes,
    /// we need to define a method to get the blackboard reference.
    /// This will return a pointer to the BB used by the system, whether we're in the root or in a path node.
    ///
    /// # Returns
    /// A `Result<Arc<Mutex<ArcAroraBlackboard>>, String>` containing a reference to the blackboard, or an error message
    fn get_blackboard(&self) -> Result<Arc<Mutex<ArcAroraBlackboard>>, String>;

    /// Check if the given namespace path exists and return its type.
    ///
    /// The path is a dot-separated string, e.g. "namespace1.namespace2.item".
    /// The function will traverse the namespace tree and return the type of the last component.
    ///
    /// # Arguments
    /// * `path` - The namespace path to check
    ///
    /// # Returns
    /// A `Result<CheckPathResult, String>` indicating whether the path exists and what type it is, or an error message
    fn check_path(&self, path: &str) -> Result<CheckPathResult, String> {
        let name_parts = split_path(path);
        if name_parts.is_empty() {
            return Ok(CheckPathResult::None());
        }

        let mut current_node_id: Option<Uuid> = None;
        for (i, name_part) in name_parts.iter().enumerate() {
            // Key fix: Separate block for each iteration to ensure locks are released
            let node_id = {
                if current_node_id.is_none() {
                    // Check in root
                    self.get_name_id(name_part)?
                } else if let Some(path_id) = &current_node_id {
                    // Check in current node
                    let next_id = {
                        if let Ok(Some(node_arc)) = self.get_node_by_id(path_id) {
                            // Create a separate scope for the lock so it's released immediately after use
                            let name_id = {
                                if let Ok(node_guard) = node_arc.lock() {
                                    if let ArcABBNode::Path(path) = &*node_guard {
                                        path.get_name_id(name_part)?
                                    } else {
                                        None
                                    }
                                } else {
                                    return Err("Failed to lock mutex".to_string());
                                }
                            }; // Lock is released here when node_guard goes out of scope
                            name_id
                        } else {
                            // Current node is not a path node
                            None
                        }
                    };
                    next_id
                } else {
                    // Node not found
                    None
                }
            }; // This closure ensures all locks are released before next iteration

            if let Some(id) = node_id {
                if i == name_parts.len() - 1 {
                    // Found the final component - check its type
                    let result = {
                        if let Ok(Some(node_arc)) = self.get_node_by_id(&id) {
                            let node_type = {
                                if let Ok(node_guard) = node_arc.lock() {
                                    match &*node_guard {
                                        ArcABBNode::Item(item_node) => {
                                            let item_id = item_node.get_id_copy()?;
                                            Some(CheckPathResult::IsItem(item_id))
                                        }
                                        ArcABBNode::Path(path_node) => {
                                            let path_id = path_node.get_id_copy()?;
                                            Some(CheckPathResult::IsPath(path_id))
                                        }
                                    }
                                } else {
                                    return Err("Failed to lock mutex".to_string());
                                }
                            }; // Lock is released here when node_guard goes out of scope
                            node_type
                        } else {
                            None
                        }
                    };

                    if let Some(result) = result {
                        return Ok(result);
                    }
                } else {
                    // Continue traversing
                    current_node_id = Some(id);
                }
            } else {
                // Component not found
                return Ok(CheckPathResult::None());
            }
        }

        // If we get here, it means the path is empty or traversal failed
        Ok(CheckPathResult::None())
    }

    /// Set an item in the blackboard at the specified path with a Value or KeyValue block.
    /// If it does not exist, it will create the necessary path nodes.
    /// If it exists, it will update the existing item.
    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Uuid, String> {
        self.set_with_id(path, value, None)
    }

    /// Set method that can handle either Value or a KeyValue block
    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        item_id: Option<Uuid>,
    ) -> Result<Uuid, String> {
        self.set_value_with_compatibility_check(&path.to_string(), value, item_id, true)
    }

    fn set_value_with_compatibility_check(
        &mut self,
        path: &str,
        value: Value,
        item_id: Option<Uuid>,
        check_compatibility: bool,
    ) -> Result<Uuid, String> {
        let path_parts: Vec<String> = split_path(path);
        if path_parts.is_empty() {
            return Err("Path cannot be empty when setting an item to the blackboard".to_string());
        }

        let res = self.check_path(path)?;
        let mut ret_id: Uuid;

        if let Value::KeyValue(kv) = value {
            ret_id = if kv.id.is_nil() { gen_bb_uuid() } else { kv.id };

            if let CheckPathResult::IsItem(existing_id) = res {
                return Err(format!(
                "Path {} already exists as an Item with id {}, cannot set KeyValue structure here",
                    path, existing_id
                ));
            } else if let CheckPathResult::IsPath(current_path_id) = res {
                // If the path exists as a node, and the provided value is a KeyValue,
                // check compatibility first and then set recursively
                // We only want to check compatoibility on the first cycle of the recursion
                self._set_keyvalue_into_existing_field(
                    path,
                    item_id,
                    check_compatibility,
                    &mut ret_id,
                    current_path_id,
                    &kv,
                )?;
            } else {
                self._set_keyvalue_into_new_field(path, item_id, &path_parts, &mut ret_id, kv)?;
            }
        } else {
            // not keyvalue
            if let CheckPathResult::IsPath(_) = res {
                // BBPath exists but contains a Path node
                return Err(format!(
                    "Path {} already exists as a BBPath node, cannot set it with a Value",
                    path
                ));
            } else {
                ret_id = self._set_item(path, value, item_id, path_parts, res)?;
            }
        }
        Ok(ret_id)
    }

    fn _set_keyvalue_into_existing_field(
        &mut self,
        path: &str,
        item_id: Option<Uuid>,
        check_compatibility: bool,
        ret_id: &mut Uuid,
        current_path_id: Uuid,
        kv: &KeyValue,
    ) -> Result<(), String> {
        let existing_path_arc = self.get_node_by_id(&current_path_id)?.ok_or_else(|| {
            format!(
                "Failed to get existing path node for {} when setting KeyValue structure",
                path
            )
        })?;

        if check_compatibility {
            if let Some(res) = self.check_path_compatible_with_kv(&current_path_id, kv)? {
                return Err(format!(
                    "Incompatible Value structure for existing path {}: {}",
                    current_path_id,
                    res.unwrap_err()
                ));
            }
        }

        *ret_id = if kv.id.is_nil() { gen_bb_uuid() } else { kv.id };

        for (field_name, field) in &kv.fields {
            let existing_field_id = {
                let existing_node = existing_path_arc
                    .lock()
                    .map_err(|_| "Failed to lock existing node mutex")?;
                if let ArcABBNode::Path(existing_path) = &*existing_node {
                    existing_path.get_name_id(field_name)?
                } else {
                    return Err(format!(
                        "Existing node is not a BBPath node for {}",
                        current_path_id
                    ));
                }
            };
            self.assign_kv_field(
                path,
                item_id,
                field_name,
                field,
                existing_field_id,
                current_path_id,
            )?;
        }
        Ok(())
    }

    fn _set_keyvalue_into_new_field(
        &mut self,
        path: &str,
        item_id: Option<Uuid>,
        path_parts: &[String],
        ret_id: &mut Uuid,
        kv: KeyValue,
    ) -> Result<(), String> {
        *ret_id = if kv.id.is_nil() { gen_bb_uuid() } else { kv.id };

        let new_node_name = path_parts.last().unwrap().clone();
        let new_path_node = ArcABBPathNode::new_with_full_path(
            new_node_name.clone(),
            *ret_id,
            self.get_blackboard()?,
            path,
        );
        let new_path = ArcABBNode::Path(new_path_node);
        let new_path_arc = Arc::new(Mutex::new(new_path));

        {
            let bb_ref = self.get_blackboard()?;
            let mut bb = match bb_ref.lock() {
                Ok(bb) => bb,
                Err(_) => return Err("Failed to lock blackboard mutex".to_string()),
            };
            bb.insert_item_in_hash(*ret_id, new_path_arc.clone());
        }
        //println!("Inserting new path with name: {}", new_node_name);
        //self.insert(new_node_name, *ret_id)?;

        match self.create_path_to(path, ret_id) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        self._set_keyvalue_into_existing_field(path, item_id, false, ret_id, *ret_id, &kv)
    }

    fn _set_item(
        &mut self,
        path: &str,
        value: Value,
        item_id: Option<Uuid>,
        path_parts: Vec<String>,
        res: CheckPathResult,
    ) -> Result<Uuid, String> {
        Ok(if let CheckPathResult::IsItem(existing_id) = res {
            // BBPath exists and contains an Item node
            let use_id = if let Some(ref provided_id) = item_id {
                if !provided_id.is_nil() {
                    *provided_id
                } else {
                    existing_id
                }
            } else {
                // If no item_id is provided, use the existing one
                existing_id
            };
            self.set_existing_bb_item(value, &use_id)?;
            use_id
        } else {
            // BBPath does not exist, so create it
            // Generate a unique ID for the item if not provided
            let new_item_id = if let Some(id) = item_id {
                if id.is_nil() {
                    gen_bb_uuid()
                } else {
                    id
                }
            } else {
                gen_bb_uuid()
            };

            self.create_path_to(path, &new_item_id)?;
            self.set_bb_item(
                value,
                &new_item_id,
                Some(path_parts.last().unwrap().clone()),
                Some(path),
            )?;
            new_item_id
        })
    }

    fn assign_kv_field(
        &mut self,
        path: &str,
        _item_id: Option<Uuid>,
        field_name: &String,
        field: &KeyValueField,
        existing_field_id: Option<Uuid>,
        current_path_id: Uuid,
    ) -> Result<(), String> {
        let src_field_id = if field.id.is_nil() {
            gen_bb_uuid()
        } else {
            field.id
        };

        if let Some(field_value) = field.value.clone() {
            // We have a field value, so we can proceed to set it

            if let Value::KeyValue(sub_kv) = *field_value {
                // The content is a KeyValue (nested)
                if let Some(existing_ref_id) = existing_field_id {
                    // Field name exists, so we will perform a recursive assignment of the KeyValue structure into the existing path
                    // First check if the existing node is a BBPath node
                    let proceed =
                        if let Some(existing_node_arc) = self.get_node_by_id(&existing_ref_id)? {
                            if let Ok(existing_node_guard) = existing_node_arc.lock() {
                                if let ArcABBNode::Path(_) = &*existing_node_guard {
                                    // ok it's a BBPath, we can continue - note we are not checking compatibility again at each level of the recursion
                                    true
                                } else {
                                    return Err(format!(
                                        "Existing node is not a ABBPathNode for {}",
                                        existing_ref_id
                                    ));
                                }
                            } else {
                                return Err(format!(
                                    "Failed to lock existing node mutex for {}",
                                    existing_ref_id
                                ));
                            }
                        } else {
                            return Err(format!(
                                "Failed to find node for field by ID for {}",
                                existing_ref_id
                            ));
                        };
                    if proceed {
                        for (sub_field_name, sub_field) in &sub_kv.fields {
                            if let Err(e) = self.assign_kv_field(
                                path,
                                _item_id,
                                sub_field_name,
                                sub_field,
                                None,
                                existing_ref_id,
                            ) {
                                return Err(format!(
                                    "Failed to set KeyValue structure into existing path {}: {}",
                                    sub_field_name, e
                                ));
                            }
                        }
                    }
                } else {
                    // Field name does not exist, so create it as a new BBPath node and set the KeyValue structure into it
                    // Generate a unique ID for the new path node
                    // Create new ABBPathNode
                    let new_path = ArcABBPathNode::new_with_full_path(
                        field_name.clone(),
                        src_field_id,
                        self.get_blackboard()?,
                        &format!("{}.{}", path, field_name),
                    );
                    let new_node = ArcABBNode::Path(new_path);

                    {
                        if let Some(existing_path_arc) = self.get_node_by_id(&current_path_id)? {
                            let mut existing_node = existing_path_arc
                                .lock()
                                .map_err(|_| "Failed to lock existing node mutex")?;
                            if let ArcABBNode::Path(ref mut existing_path_node) = *existing_node {
                                // Add the new path to the blackboard items and as a field in the existing path
                                {
                                    let bb_ref = self.get_blackboard()?;
                                    let mut bb = match bb_ref.lock() {
                                        Ok(bb) => bb,
                                        Err(_) => {
                                            return Err(
                                                "Failed to lock blackboard mutex".to_string()
                                            )
                                        }
                                    };
                                    bb.insert_item_in_hash(
                                        src_field_id,
                                        Arc::new(Mutex::new(new_node)),
                                    );
                                }
                                existing_path_node.insert(field_name.clone(), src_field_id)?;
                            } else {
                                return Err(format!(
                                    "Existing node is not a ABBPathNode for {}",
                                    current_path_id
                                ));
                            };
                        } else {
                            return Err(format!(
                                "Failed to find node by ID for {}",
                                current_path_id
                            ));
                        }
                    }

                    if let Err(e) = self.assign_kv_field(
                        &format!("{}.{}", path, field_name),
                        _item_id,
                        field_name,
                        &KeyValueField::new_with_id_and_option(
                            field_name.clone(),
                            src_field_id,
                            Some(Value::KeyValue(sub_kv)),
                        ),
                        Some(src_field_id),
                        current_path_id,
                    ) {
                        return Err(format!(
                            "Failed to set KeyValue structure into new path {}: {}",
                            field_name, e
                        ));
                    }
                }
            } else {
                // The content is a Value, set it as an Item node
                let value = *field_value;
                // Check if the field id retrieved from the existing path is None or not - meaning whether it already exists or not
                if let Some(existing_ref_id) = existing_field_id {
                    // Set the value of the existing Item node directly given we already checked for compatibility
                    self.set_existing_bb_item(value, &existing_ref_id)?;
                } else {
                    // Field name does not exist in the path, so create it as a new Item node
                    if let Some(existing_path_arc) = self.get_node_by_id(&current_path_id)? {
                        let mut existing_path_node = existing_path_arc
                            .lock()
                            .map_err(|_| "Failed to lock existing node mutex")?;
                        if let ArcABBNode::Path(ref mut existing_path) = *existing_path_node {
                            // Create new Item node and store it in the blackboard items
                            let new_item = ABBItemNode::from_value(
                                field_name,
                                value,
                                src_field_id,
                                &format!("{}.{}", path, field_name).to_string(),
                            )
                            .map_err(|e| format!("Failed to create ABBItemNode: {}", e))?
                            .ok_or_else(|| {
                                "Failed to create ABBItemNode: returned None".to_string()
                            })?;
                            let new_node = ArcABBNode::Item(new_item);
                            {
                                let bb_ref = self.get_blackboard()?;
                                let mut bb = match bb_ref.lock() {
                                    Ok(bb) => bb,
                                    Err(_) => {
                                        return Err("Failed to lock blackboard mutex".to_string())
                                    }
                                };
                                bb.insert_item_in_hash(
                                    src_field_id,
                                    Arc::new(Mutex::new(new_node)),
                                );
                            }
                            existing_path.insert(field_name.clone(), src_field_id)?;
                        } else {
                            return Err(format!(
                                "Existing node is not a BBPath node for {}",
                                current_path_id
                            ));
                        };
                    }
                }
            }
        }
        Ok(())
    }

    /// Create all intermediate path nodes and point the last component to the provided ID
    fn create_path_to(&mut self, path: &str, target_id: &Uuid) -> Result<(), String> {
        let name_parts = split_path(path);
        if name_parts.is_empty() {
            return Err("Path cannot be empty when adding an item to the blackboard".to_string());
        }

        // current_path will be used for error reporting
        let mut intermediate_path = String::new();

        // Get the ID of the current node
        let mut current_node_id = self.get_id_copy()?;
        let bb_ref = self.get_blackboard()?;
        let bb = bb_ref.clone();

        // For each part of the path, check if it exists and create it if not
        for (_i, part) in name_parts.iter().enumerate().take(name_parts.len()) {
            // Build the current path for error reporting
            if !intermediate_path.is_empty() {
                intermediate_path.push('.');
            }
            intermediate_path.push_str(part);

            if let Ok(mut bb_guard) = bb_ref.lock() {
                if let Some(current_node_arc) = bb_guard.get_node_by_id(&current_node_id)? {
                    // navigating an existing node
                    if let Ok(mut current_node_guard) = current_node_arc.lock() {
                        if let ArcABBNode::Path(ref mut current_path) = *current_node_guard {
                            // Get the next path node
                            current_node_id = if current_path.contains(part)? {
                                // If the path already exists, get its ID
                                let node_id = current_path.get_name_id(part)?.unwrap();

                                // Make sure it's a path node
                                if let Some(node) = bb_guard.get_node_by_id(&node_id)? {
                                    if let Ok(node_guard) = node.lock() {
                                        if let ArcABBNode::Item(_) = &*node_guard {
                                            return Err(format!("Path component '{}' in '{}' is an Item, expected a BBPath",
                                                part, intermediate_path));
                                        }
                                    }
                                }
                                node_id
                            } else {
                                // if second last node, then use target id, otherwise create a new path node ID
                                let new_node_id = if _i >= name_parts.len() - 1 {
                                    *target_id
                                } else {
                                    let new_id = gen_bb_uuid();
                                    let new_path = ArcABBPathNode::new_with_full_path(
                                        part.clone(),
                                        new_id,
                                        bb.clone(),
                                        &intermediate_path,
                                    );
                                    let new_node = ArcABBNode::Path(new_path);
                                    bb_guard.insert_item_in_hash(
                                        new_id,
                                        Arc::new(Mutex::new(new_node)),
                                    );
                                    new_id
                                };

                                current_path.insert(part.clone(), new_node_id)?;
                                new_node_id
                            };
                        } else {
                            return Err(format!(
                                "Current node is not a BBPath node for {}",
                                current_node_id
                            ));
                        }
                    } else {
                        return Err(format!(
                            "Failed to lock current node mutex for {}",
                            current_node_id
                        ));
                    }
                } else {
                    return Err(format!("Failed to find node by ID for {}", current_node_id));
                }
            } else {
                return Err("Failed to lock blackboard mutex".to_string());
            }
        }

        Ok(())
    }

    /// Method to check if path is compatible with KeyValue and can merge
    fn check_path_compatible_with_kv(
        &self,
        path_id: &Uuid,
        kv: &KeyValue,
    ) -> Result<Option<Result<String, String>>, String> {
        if let Some(node_arc) = self.get_node_by_id(path_id)? {
            // Provided path ID exists, so check if it is a BBPath node

            if let Ok(node_guard) = node_arc.lock() {
                if let ArcABBNode::Path(path) = &*node_guard {
                    // It's a path, check compatibility of the KeyValue structure with the existing path
                    for (field_name, field) in &kv.fields {
                        // Check if the field name already exists in the existing path
                        if let Some(existing_ref_id) = path.get_name_id(field_name)? {
                            // Field name exists, so check IDS first

                            // Check if the field is a KeyValue or a Value and then check for compatibility
                            if let Some(field_value) = field.value.clone() {
                                if let Value::KeyValue(sub_kv) = *field_value {
                                    // The KV content is another KeyValue (nested), check compatibility recursively
                                    return self
                                        .check_path_compatible_with_kv(&existing_ref_id, &sub_kv);
                                } else {
                                    // The KV content is a Value, check if the existing node is an Item node

                                    if let Some(existing_node_arc) =
                                        self.get_node_by_id(&existing_ref_id)?
                                    {
                                        if let Ok(existing_node_guard) = existing_node_arc.lock() {
                                            if let ArcABBNode::Item(existing_item) =
                                                &*existing_node_guard
                                            {
                                                // Check if the value types are compatible
                                                if !adt::utils::is_compatible_type(
                                                    &field_value,
                                                    existing_item.get_value_type(),
                                                ) {
                                                    return Ok(Some(Err(format!("Incompatible value type for field {}. Existing type: {:?}, New value: {:?}",
                                                        field_name, existing_item.get_value_type(), field_value))));
                                                }
                                            } else {
                                                return Ok(Some(Err(format!(
                                                    "Existing node is not an Item node for {}",
                                                    existing_ref_id
                                                ))));
                                            }
                                        } else {
                                            return Ok(Some(Err(format!(
                                                "Failed to lock existing node mutex for {}",
                                                existing_ref_id
                                            ))));
                                        }
                                    } else {
                                        return Ok(Some(Err(format!(
                                            "Failed to find node by ID for {}",
                                            existing_ref_id
                                        ))));
                                    }
                                }
                            }
                        } else {
                            // Field name does not exist, so it is compatible (will be created)
                        }
                    }
                    // All fields are compatible, return None
                    Ok(None)
                } else {
                    Ok(Some(Err(format!(
                        "Existing node is not a BBPath node for {}",
                        path_id
                    ))))
                }
            } else {
                Ok(Some(Err(format!(
                    "Failed to lock existing node mutex for {}",
                    path_id
                ))))
            }
        } else {
            Ok(Some(Err(format!(
                "Failed to find node by ID for {}",
                path_id
            ))))
        }
    }
}

/// Trait for objects that can be formatted as tree structures.
///
/// This trait provides methods to format the tree structure in different ways,
/// suitable for logging and debugging purposes.
pub trait TreeFormattable {
    /// Format the tree structure without showing IDs
    fn format_tree_simple(&self) -> String {
        self.format_tree(false)
    }

    /// Format the tree structure showing IDs
    fn format_tree_with_ids(&self) -> String {
        self.format_tree(true)
    }

    /// Format the tree structure with optional ID display
    fn format_tree(&self, show_ids: bool) -> String;
}

/// Trait for objects that can format their items as a formatted list.
///
/// This trait provides methods to format blackboard items in different ways,
/// suitable for logging and debugging purposes.
pub trait ItemsFormattable {
    /// Format the items list without showing IDs
    fn format_items_simple(&self) -> String {
        self.format_items(false)
    }

    /// Format the items list showing IDs  
    fn format_items_with_ids(&self) -> String {
        self.format_items(true)
    }

    /// Format the items list with optional ID display
    fn format_items(&self, show_ids: bool) -> String;
}
