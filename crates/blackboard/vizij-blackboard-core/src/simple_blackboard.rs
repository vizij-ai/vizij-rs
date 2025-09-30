use arora_schema::value::Value;
use std::collections::HashMap;

/// A leaf node that contains a name and a ChannelValue
#[derive(Debug)]
pub struct BBItem {
    name: String,
    value: Value,
}

/// An enum that can represent either a KeyValueItem or a BBItem
#[derive(Debug)]
pub enum BBNode {
    Container(KeyValueItem),
    Leaf(BBItem),
}

/// A container that holds multiple BBNodes, mapped by name
#[derive(Debug)]
pub struct KeyValueItem {
    name: String,
    index: HashMap<String, BBNode>,
}

/// The root node of the blackboard structure
#[derive(Debug)]
pub struct SimpleBlackboard {
    root: KeyValueItem,
}

/// Trait for objects that can hold items
pub trait ItemHolder {
    fn add_item<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<(), String>;
    fn get_item<S: ToString + ?Sized>(&self, path: &S) -> Option<&Value>;
    fn remove_item<S: ToString + ?Sized>(&mut self, path: &S) -> Option<Value>;
}

impl BBItem {
    pub fn new(name: String, value: Value) -> Self {
        Self { name, value }
    }
}

impl BBNode {
    /// Convenience method to create a new leaf node
    pub fn new_leaf(name: String, value: Value) -> Self {
        BBNode::Leaf(BBItem::new(name, value))
    }

    /// Convenience method to create a new container node
    pub fn new_container(name: String) -> Self {
        BBNode::Container(KeyValueItem::new(name))
    }

    /// Get the name of this node
    pub fn name(&self) -> &str {
        match self {
            BBNode::Container(kv) => &kv.name,
            BBNode::Leaf(item) => &item.name,
        }
    }

    /// Get the value if this is a leaf node
    pub fn value(&self) -> Option<&Value> {
        match self {
            BBNode::Leaf(item) => Some(&item.value),
            _ => None,
        }
    }

    /// Get mutable value if this is a leaf node
    pub fn value_mut(&mut self) -> Option<&mut Value> {
        match self {
            BBNode::Leaf(item) => Some(&mut item.value),
            _ => None,
        }
    }

    /// Get as container if this is a container node
    pub fn as_container(&self) -> Option<&KeyValueItem> {
        match self {
            BBNode::Container(kv) => Some(kv),
            _ => None,
        }
    }

    /// Get as mutable container if this is a container node
    pub fn as_container_mut(&mut self) -> Option<&mut KeyValueItem> {
        match self {
            BBNode::Container(kv) => Some(kv),
            _ => None,
        }
    }
}

impl KeyValueItem {
    pub fn new(name: String) -> Self {
        Self {
            name,
            index: HashMap::new(),
        }
    }

    /// Get the name of this item
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Helper method to ensure a path exists, creating containers as needed
    ///
    /// # Arguments
    /// * `path_parts` - The path parts to navigate/create
    /// * `leaf` - The leaf node to add at the end of the path
    ///
    /// # Returns
    /// * `Ok(())` if the path was successfully created/traversed and the leaf was added
    /// * `Err(String)` if there was an error (empty path or path conflict)
    fn ensure_path_exists_and_add(
        &mut self,
        path_parts: &[&str],
        leaf: BBNode,
    ) -> Result<(), String> {
        if path_parts.is_empty() {
            return Err("Cannot ensure an empty path exists".to_string());
        }

        let part = path_parts[0];

        if path_parts.len() == 1 {
            self.index.insert(part.to_string(), leaf);
            return Ok(());
        } else if path_parts.len() > 1 {
            if !self.index.contains_key(part) {
                self.index
                    .insert(part.to_string(), BBNode::new_container(part.to_string()));
            }

            // If the path continues but we have a leaf node here, we return an error
            if let Some(node) = self.index.get(part) {
                if let BBNode::Leaf(_) = node {
                    return Err(format!(
                        "Path conflict: Expected container but found leaf node at '{}'",
                        part
                    ));
                }
            }

            // We need to handle the leaf vs. container case separately
            // Check if our node is a container
            if let Some(BBNode::Container(_)) = self.index.get(part) {
                // Guaranteed to be safe because we just checked above
                let container_node = self.index.get_mut(part).unwrap();
                if let BBNode::Container(ref mut container) = container_node {
                    // Recurse with the remaining path
                    return container.ensure_path_exists_and_add(&path_parts[1..], leaf);
                }
            }
        }

        Ok(())
    }
}

impl ItemHolder for KeyValueItem {
    /// Add an item to the blackboard at the specified path
    ///
    /// # Arguments
    /// * `path` - The dot-separated path where the item should be added
    /// * `value` - The value to add
    ///
    /// # Returns
    /// * `Ok(())` if the item was successfully added
    /// * `Err(String)` if there was an error (e.g., path conflict, invalid path)
    fn add_item<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<(), String> {
        let path = path.to_string();
        // Split the path by '.'
        let path_parts: Vec<&str> = path.split('.').collect();

        if path_parts.is_empty() {
            return Err("Cannot add item with empty path".to_string());
        }

        let leaf = BBNode::Leaf(BBItem::new(path_parts.last().unwrap().to_string(), value));
        // Ensure all parent paths exist and add the final leaf node
        self.ensure_path_exists_and_add(&path_parts, leaf)
    }

    fn get_item<S: ToString + ?Sized>(&self, path: &S) -> Option<&Value> {
        let path = path.to_string();
        // Split the path by '.'
        let path_parts: Vec<&str> = path.split('.').collect();

        if path_parts.is_empty() {
            return None; // Nothing to get for empty path
        }

        // Navigate the path
        let mut current = self;

        for (i, part) in path_parts.iter().enumerate() {
            // Get the node at this level
            if let Some(node) = current.index.get(*part) {
                if i == path_parts.len() - 1 {
                    // If we've reached the end of the path, return the value
                    return node.value();
                } else if let Some(container) = node.as_container() {
                    // If we need to keep going, update current
                    current = container;
                } else {
                    // Not a container but we need to go deeper
                    return None;
                }
            } else {
                // No node at this path
                return None;
            }
        }

        None // Should never reach here
    }

    fn remove_item<S: ToString + ?Sized>(&mut self, path: &S) -> Option<Value> {
        let path = path.to_string();
        // Split the path by '.'
        let path_parts: Vec<&str> = path.split('.').collect();

        if path_parts.is_empty() {
            return None; // Nothing to remove for empty path
        }

        // If only one path part, remove directly from this container
        if path_parts.len() == 1 {
            return match self.index.remove(path_parts[0]) {
                Some(BBNode::Leaf(item)) => Some(item.value),
                _ => None, // Removed a container or nothing
            };
        }

        // For deeper paths, navigate to the parent container
        let parent_path = &path_parts[..path_parts.len() - 1];
        let last_part = path_parts[path_parts.len() - 1];

        let mut current = self;

        // Navigate to the parent container
        for part in parent_path {
            if let Some(BBNode::Container(container)) = current.index.get_mut(*part) {
                current = container;
            } else {
                return None; // Path doesn't exist or isn't a container
            }
        }

        // Now we're at the parent container, remove the item
        match current.index.remove(last_part) {
            Some(BBNode::Leaf(item)) => Some(item.value),
            _ => None, // Removed a container or nothing
        }
    }
}

impl SimpleBlackboard {
    pub fn new<S: ToString>(name: S) -> Self {
        let root = KeyValueItem::new(name.to_string());

        Self { root }
    }

    pub fn name(&self) -> &str {
        self.root.name()
    }

    pub fn index(&self) -> &HashMap<String, BBNode> {
        &self.root.index
    }
}

impl ItemHolder for SimpleBlackboard {
    // Delegate to the root KeyValueItem
    fn add_item<S: ToString + ?Sized>(&mut self, path: &S, value: Value) -> Result<(), String> {
        self.root.add_item(path, value)
    }

    fn get_item<S: ToString + ?Sized>(&self, path: &S) -> Option<&Value> {
        self.root.get_item(path)
    }

    fn remove_item<S: ToString + ?Sized>(&mut self, path: &S) -> Option<Value> {
        self.root.remove_item(path)
    }
}
