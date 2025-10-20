use std::collections::HashMap;

use uuid::Uuid;

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
pub trait BBNodeTrait {
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

pub trait BBPathNodeTrait: BBNodeTrait + TreeFormattable {
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

    /// Return a copy of the map of item names and IDs in this path.
    ///
    /// # Returns
    /// A `Result<HashMap<String, String>, String>` containing name-to-ID mappings, or an error message
    fn get_names_copy(&self) -> Result<HashMap<String, Uuid>, String>;

    fn _format_tree_recursively(
        &self,
        name: &str,
        id: &Uuid,
        depth: usize,
        show_ids: bool,
        output: &mut String,
    );

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
