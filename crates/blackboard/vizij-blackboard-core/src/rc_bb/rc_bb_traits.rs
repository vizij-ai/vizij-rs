//! This module defines the traits used in the Arora blackboard system.
//! These traits define the behavior of nodes, paths, and the blackboard itself,
//! and provide a common interface for interacting with the system regardless of
//! the specific implementation details.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use arora_schema::value::Value;
use arora_schema::{
    gen_bb_uuid,
    keyvalue::{KeyValue, KeyValueField},
};
use uuid::Uuid;

use crate::{adt, PATH_SEPARATOR};
use crate::{
    split_path,
    traits::{BBNodeTrait, BBPathNodeTrait, BlackboardTrait, CheckPathResult},
    BBItemNode,
};

use super::{RcBBNode, RcBBPathNode, RcBlackboard};

/// This is the main trait for objects that can be used as paths in the blackboard.
///
/// It provides methods to insert, retrieve, and check items in the path,
/// as well as utility methods for navigating the hierarchy.
pub trait RcBBPathNodeTrait: BBPathNodeTrait {
    /// Retrieve a node by its ID from the blackboard.
    ///
    /// # Arguments
    /// * `id` - The ID of the node to retrieve
    ///
    /// # Returns
    /// A `Result<Option<Rc<RefCell<BBNode>>>, String>` containing the node if found, or an error message
    fn get_node_by_id(&self, id: &Uuid) -> Result<Option<Rc<RefCell<RcBBNode>>>, String>;

    /// Helper to allow passing a full String namespace path (dot.separated) directly.
    ///
    /// # Arguments
    /// * `path` - The path as a dot-separated string
    ///
    /// # Returns
    /// A `Result<Option<Rc<RefCell<BBNode>>>, String>` containing the node if found, or an error message
    fn get<S: ToString + ?Sized>(&self, path: &S) -> Result<Option<Rc<RefCell<RcBBNode>>>, String> {
        let names = split_path(&path.to_string());
        self.get_by_names(names)
    }

    /// Helper to return the ID of an Item node directly by its path
    fn get_path_id<S: ToString + ?Sized>(&self, path: &S) -> Result<Option<Uuid>, String> {
        if let Some(node_ref) = self.get(path)? {
            match &*node_ref.borrow() {
                RcBBNode::Path(path_node) => {
                    // Return the ID of the path node
                    return path_node.get_id_copy().map(Some);
                }
                RcBBNode::Item(item_node) => {
                    // The node is an Item node, return its ID
                    return item_node.get_id_ref().map(|id_ref| Some(*id_ref));
                }
            }
        }
        Ok(None)
    }

    /// Helper to return the value of an Item node directly by its path
    fn get_value<S: ToString + ?Sized>(&self, path: &S) -> Result<Option<Value>, String> {
        if let Some(node_ref) = self.get(path)? {
            if let RcBBNode::Item(item_node) = &*node_ref.borrow() {
                // Return the value of the item node
                return Ok(item_node.get_value().cloned());
            } else {
                // The node is not an Item node
                return Ok(None);
            }
        }
        Ok(None)
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
        if let Some(node_ref) = self.get_node_by_id(id).unwrap() {
            match &*node_ref.borrow() {
                RcBBNode::Path(path) => {
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
                                BBPathNodeTrait::_format_tree_recursively(
                                    self,
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
                RcBBNode::Item(item) => match item.get_current_name_copy() {
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
                "{}Node not found: {}\n",
                " ".repeat(depth * 2),
                id
            ));
        }
    }

    /// Get the node referenced by the given path parts, by traversing the namespace tree
    /// This is the main getter function used to retrieve nodes by path
    fn get_by_names(&self, names: Vec<String>) -> Result<Option<Rc<RefCell<RcBBNode>>>, String> {
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
                if let Some(node_ref) = self.get_node_by_id(path_id)? {
                    if let RcBBNode::Path(path) = &*node_ref.borrow() {
                        path.get_name_id(name_part)?
                    } else {
                        // Current node is not a path node, but may be a KeyValue node
                        None
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
                    // Found the final component - return the node directly
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
        let node_opt = self.get(path)?;

        // If the node doesn't exist, return None
        let node_ref = match node_opt {
            Some(node) => node,
            None => return Ok(None),
        };

        self.get_keyvalue_from_node_ref(node_ref)
    }

    fn get_keyvalue_by_id(&self, id: &Uuid) -> Result<Option<KeyValue>, String> {
        // First check if the path exists
        let node_opt = self.get_node_by_id(id)?;

        // If the node doesn't exist, return None
        let node_ref = match node_opt {
            Some(node) => node,
            None => return Ok(None),
        };

        self.get_keyvalue_from_node_ref(node_ref)
    }

    fn get_keyvalue_from_node_ref(
        &self,
        node_ref: Rc<RefCell<RcBBNode>>,
    ) -> Result<Option<KeyValue>, String> {
        let is_path = node_ref.borrow().is_path()?;
        let mut fields = HashMap::new();
        if is_path {
            // This is a namespace node, we need to recursively build a KeyValue structure

            // Get all the items in this path
            let names: HashMap<String, Uuid> = {
                if let Some(path_node) = node_ref.borrow().as_path() {
                    path_node.get_names_copy()?
                } else {
                    return Err("Failed to get path node".to_string());
                }
            };

            // For each child, recursively build the KeyValue structure
            for (name, child_id) in names {
                let child_node_opt = self.get_node_by_id(&child_id)?;

                let child_node_ref = match child_node_opt {
                    Some(opt_node) => opt_node,
                    None => continue, // Skip if the child node doesn't exist
                };
                let child_node = child_node_ref.borrow();

                // Check if this child is an item or path
                if child_node.is_path()? {
                    // It's a path, so recursively get its KeyValue
                    if let Some(child_kv) = self.get_keyvalue_by_id(&child_id)? {
                        // Add this to our fields
                        fields.insert(
                            name.clone(),
                            KeyValueField::new_with_id(name, child_id, Value::KeyValue(child_kv)),
                        );
                    }
                } else {
                    // It's an item, get its value
                    let item_node = match child_node.as_item() {
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
            let node = node_ref.borrow();
            let item_node = match node.as_item() {
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

        let id: Uuid = node_ref.borrow().get_id_copy()?;
        let kv = KeyValue { id, fields };
        Ok(Some(kv))
    }

    fn as_keyvalue(&self) -> Result<Option<KeyValue>, String> {
        // This method is used to convert the node into a KeyValue structure
        // It will return None if the node is not a path or item
        if let Ok(Some(node_ref)) = self.get_node_by_id(&self.get_id_copy()?) {
            self.get_keyvalue_from_node_ref(node_ref)
        } else {
            Ok(None)
        }
    }
}

/// Define a trait for setting items in a namespaced manner.
///
/// This trait is used to set items in the blackboard, either in the root or in a path node.
/// It provides methods for navigating the namespace hierarchy and setting values at specific paths.
pub trait NamespacedSetterTrait: BlackboardTrait + RcBBPathNodeTrait {
    /// Get a reference to the blackboard.
    ///
    /// Because this trait can be implemented by both the blackboard and path nodes,
    /// we need to define a method to get the blackboard reference.
    /// This will return a pointer to the BB used by the system, whether we're in the root or in a path node.
    ///
    /// # Returns
    /// A `Result<Rc<RefCell<RcBlackboard>>, String>` containing the blackboard reference, or an error message
    fn get_blackboard(&self) -> Result<Weak<RefCell<RcBlackboard>>, String>;

    /// Insert an entry into the blackboard's item hash.
    /// # Arguments
    /// * `id` - The unique identifier for the node
    /// * `item` - The node to insert
    ///     
    /// # Note
    /// This method is intended to be used internally by the blackboard system
    fn _insert_entry(&mut self, id: Uuid, item: Rc<RefCell<RcBBNode>>);

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
            let node_id = {
                if current_node_id.is_none() {
                    // Check in root
                    self.get_name_id(name_part)?
                } else if let Some(path_id) = &current_node_id {
                    // Check in current node
                    let next_id = {
                        if let Some(node_ref) = self.get_node_by_id(path_id)? {
                            let name_id = {
                                if let RcBBNode::Path(path) = &*node_ref.borrow() {
                                    path.get_name_id(name_part)?
                                } else {
                                    None
                                }
                            };
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
            };

            if let Some(id) = node_id {
                if i == name_parts.len() - 1 {
                    // Found the final component - check its type
                    let result = {
                        if let Some(node_ref) = self.get_node_by_id(&id)? {
                            let node_type = {
                                match &*node_ref.borrow() {
                                    RcBBNode::Item(item_node) => {
                                        let item_id = item_node.get_id_copy()?;
                                        Some(CheckPathResult::IsItem(item_id))
                                    }
                                    RcBBNode::Path(path_node) => {
                                        let path_id = path_node.get_id_copy()?;
                                        Some(CheckPathResult::IsPath(path_id))
                                    }
                                }
                            };
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
    fn set<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<Vec<Uuid>, String> {
        self.set_with_id(path, value, None)
    }

    /// Set method that can handle either Value or a KeyValue block
    fn set_with_id<S: ToString + ?Sized>(
        &mut self,
        path: &S,
        value: Value,
        item_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>, String> {
        self.set_value_with_compatibility_check(&path.to_string(), value, item_id, true)
    }

    fn set_value_with_compatibility_check(
        &mut self,
        path: &str,
        value: Value,
        item_id: Option<Uuid>,
        check_compatibility: bool,
    ) -> Result<Vec<Uuid>, String> {
        let path_parts: Vec<String> = split_path(path);
        if path_parts.is_empty() {
            return Err("Path cannot be empty when setting an item to the blackboard".to_string());
        }

        let res = self.check_path(path)?;
        let mut ret_id: Uuid;
        let mut all_ids: Vec<Uuid> = Vec::new();

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
                    &mut all_ids,
                )?;
            } else {
                self._set_keyvalue_into_new_field(
                    path,
                    item_id,
                    &path_parts,
                    &mut ret_id,
                    kv,
                    &mut all_ids,
                )?;
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
        all_ids.push(ret_id);
        Ok(all_ids)
    }

    #[allow(clippy::too_many_arguments)]
    fn _set_keyvalue_into_existing_field(
        &mut self,
        path: &str,
        item_id: Option<Uuid>,
        check_compatibility: bool,
        ret_id: &mut Uuid,
        current_path_id: Uuid,
        kv: &KeyValue,
        all_ids: &mut Vec<Uuid>,
    ) -> Result<(), String> {
        let existing_path_ref = self.get_node_by_id(&current_path_id)?.ok_or_else(|| {
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
                if let RcBBNode::Path(existing_path) = &*existing_path_ref.borrow() {
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
                all_ids,
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
        all_ids: &mut Vec<Uuid>,
    ) -> Result<(), String> {
        *ret_id = if kv.id.is_nil() { gen_bb_uuid() } else { kv.id };

        let new_node_name = path_parts.last().unwrap().clone();
        let new_path_node = RcBBPathNode::new_with_full_path(
            new_node_name.clone(),
            *ret_id,
            self.get_blackboard()?,
            path,
        );
        let new_path = RcBBNode::Path(new_path_node);
        let new_path_ref = Rc::new(RefCell::new(new_path));

        {
            let bb_ref = self.get_blackboard()?;
            bb_ref
                .upgrade()
                .ok_or_else(|| "Blackboard is not longer available".to_string())?
                .borrow_mut()
                ._insert_entry(*ret_id, new_path_ref.clone());
        }
        //println!("Inserting new path with name: {}", new_node_name);
        //self.insert(new_node_name, *ret_id)?;

        match self.create_path_to(path, ret_id) {
            Ok(_) => {}
            Err(e) => return Err(e),
        }

        self._set_keyvalue_into_existing_field(path, item_id, false, ret_id, *ret_id, &kv, all_ids)
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

    #[allow(clippy::too_many_arguments)]
    fn assign_kv_field(
        &mut self,
        path: &str,
        _item_id: Option<Uuid>,
        field_name: &String,
        field: &KeyValueField,
        existing_field_id: Option<Uuid>,
        current_path_id: Uuid,
        all_ids: &mut Vec<Uuid>,
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
                        if let Some(existing_node_ref) = self.get_node_by_id(&existing_ref_id)? {
                            if let RcBBNode::Path(_) = &*existing_node_ref.borrow() {
                                // ok it's a BBPath, we can continue - note we are not checking compatibility again at each level of the recursion
                                true
                            } else {
                                return Err(format!(
                                    "Existing node is not a BBPathNode for {}",
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
                        all_ids.push(existing_ref_id); // Add the path node ID
                        for (sub_field_name, sub_field) in &sub_kv.fields {
                            if let Err(e) = self.assign_kv_field(
                                path,
                                _item_id,
                                sub_field_name,
                                sub_field,
                                None,
                                existing_ref_id,
                                all_ids,
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
                    // Create new BBPathNode
                    let new_path = RcBBPathNode::new_with_full_path(
                        field_name.clone(),
                        src_field_id,
                        self.get_blackboard()?,
                        &format!("{}.{}", path, field_name),
                    );
                    let new_node = RcBBNode::Path(new_path);

                    {
                        if let Some(existing_path_ref) = self.get_node_by_id(&current_path_id)? {
                            if let RcBBNode::Path(ref mut existing_path_node) =
                                *existing_path_ref.borrow_mut()
                            {
                                // Add the new path to the blackboard items and as a field in the existing path
                                {
                                    let bb_ref = self.get_blackboard()?;
                                    bb_ref
                                        .upgrade()
                                        .ok_or_else(|| {
                                            "Blackboard is not longer available".to_string()
                                        })?
                                        .borrow_mut()
                                        ._insert_entry(
                                            src_field_id,
                                            Rc::new(RefCell::new(new_node)),
                                        );
                                }
                                existing_path_node.insert(field_name.clone(), src_field_id)?;
                                all_ids.push(src_field_id); // Add the new path node ID
                            } else {
                                return Err(format!(
                                    "Existing node is not a BBPathNode for {}",
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
                        all_ids,
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
                    all_ids.push(existing_ref_id); // Add the existing item ID
                } else {
                    // Field name does not exist in the path, so create it as a new Item node
                    if let Some(existing_path_ref) = self.get_node_by_id(&current_path_id)? {
                        if let RcBBNode::Path(ref mut existing_path) =
                            *existing_path_ref.borrow_mut()
                        {
                            // Create new Item node and store it in the blackboard items
                            let new_item = BBItemNode::from_value(
                                field_name,
                                value,
                                src_field_id,
                                &format!("{}.{}", path, field_name).to_string(),
                            )
                            .map_err(|e| format!("Failed to create ABBItemNode: {}", e))?
                            .ok_or_else(|| {
                                "Failed to create ABBItemNode: returned None".to_string()
                            })?;
                            let new_node = RcBBNode::Item(new_item);
                            {
                                let bb_ref = self.get_blackboard()?;
                                bb_ref
                                    .upgrade()
                                    .ok_or_else(|| {
                                        "Blackboard is not longer available".to_string()
                                    })?
                                    .borrow_mut()
                                    ._insert_entry(src_field_id, Rc::new(RefCell::new(new_node)));
                            }
                            existing_path.insert(field_name.clone(), src_field_id)?;
                            all_ids.push(src_field_id); // Add the new item ID
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

        // For each part of the path, check if it exists and create it if not
        for (_i, part) in name_parts.iter().enumerate().take(name_parts.len()) {
            // Build the current path for error reporting
            if !intermediate_path.is_empty() {
                intermediate_path.push(PATH_SEPARATOR);
            }
            intermediate_path.push_str(part);

            let current_node_ref_opt = { self.get_node_by_id(&current_node_id)? };

            if let Some(current_node_ref) = current_node_ref_opt {
                // navigating an existing node
                if let RcBBNode::Path(ref mut current_path) = *current_node_ref.borrow_mut() {
                    // Get the next path node
                    current_node_id = if current_path.contains(part)? {
                        // If the path already exists, get its ID
                        let node_id = current_path.get_name_id(part)?.unwrap();

                        // Make sure it's a path node
                        if let Some(node_ref) = self.get_node_by_id(&node_id)? {
                            if let RcBBNode::Item(_) = &*node_ref.borrow() {
                                return Err(format!(
                                    "Path component '{}' in '{}' is an Item, expected a BBPath",
                                    part, intermediate_path
                                ));
                            }
                        }
                        node_id
                    } else {
                        // if second last node, then use target id, otherwise create a new path node ID
                        let new_node_id = if _i >= name_parts.len() - 1 {
                            *target_id
                        } else {
                            let new_id = gen_bb_uuid();
                            let new_path = RcBBPathNode::new_with_full_path(
                                part.clone(),
                                new_id,
                                self.get_blackboard()?.clone(),
                                &intermediate_path,
                            );
                            let new_node = RcBBNode::Path(new_path);
                            self._insert_entry(new_id, Rc::new(RefCell::new(new_node)));
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
                return Err(format!("Failed to find node by ID for {}", current_node_id));
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
        if let Some(node_ref) = self.get_node_by_id(path_id)? {
            // Provided path ID exists, so check if it is a BBPath node
            if let RcBBNode::Path(path) = &*node_ref.borrow() {
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

                                if let Some(existing_node_ref) =
                                    self.get_node_by_id(&existing_ref_id)?
                                {
                                    if let RcBBNode::Item(existing_item) =
                                        &*existing_node_ref.borrow()
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
                "Failed to find node by ID for {}",
                path_id
            ))))
        }
    }
}
