//! This module defines the node structure used in the Arora blackboard system.
//! Nodes can be either path nodes (containing other nodes) or item nodes (containing values).
//! This design allows for a hierarchical organization of data in the blackboard.

use uuid::Uuid;

use crate::{rc_abb::ABBPathNode, traits::BBNodeTrait, ABBItemNode};

/// An abstract node in the blackboard structure, which can be either a path or an item.
///
/// Path nodes act as containers for other nodes, creating a hierarchical structure.
/// Item nodes contain actual data values that can be accessed and modified.
#[derive(Debug)]
pub enum ABBNode {
    /// A path node that can contain other nodes
    Path(ABBPathNode),
    /// An item node that contains a data value
    Item(ABBItemNode),
}

impl ABBNode {
    /// Converts an `ABBNode` to a `PathNode` if it is a path, otherwise returns None.
    ///
    /// # Returns
    /// An `Option<&ABBPathNode>` containing a reference to the path node if this is a path node,
    /// or `None` if this is an item node.
    pub fn as_path(&self) -> Option<&ABBPathNode> {
        match self {
            ABBNode::Path(ns) => Some(ns),
            _ => None,
        }
    }

    pub fn as_path_mut(&mut self) -> Option<&mut ABBPathNode> {
        match self {
            ABBNode::Path(ns) => Some(ns),
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
            ABBNode::Item(item) => Some(item),
            _ => None,
        }
    }
}

/// Implementation of `ABBNodeTrait` for `ABBNode`.
///
/// This implementation allows `ABBNode` to be used in the blackboard hierarchy,
/// delegating to the appropriate variant (Path or Item).
impl BBNodeTrait for ABBNode {
    /// Returns a reference to the ID of this node.
    ///
    /// # Returns
    /// A `Result<&String, String>` containing a reference to the node's ID, or an error message
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        match self {
            ABBNode::Path(path) => path.get_id_ref(),
            ABBNode::Item(item) => item.get_id_ref(),
        }
    }

    /// Determines if this node is a path node.
    ///
    /// # Returns
    /// A `Result<bool, String>` indicating whether this is a path node, or an error message
    fn is_path(&self) -> Result<bool, String> {
        match self {
            ABBNode::Path(_) => Ok(true),
            ABBNode::Item(_) => Ok(false),
        }
    }

    /// Returns a copy of the name of this node.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing a copy of the node's name, or an error message
    fn get_current_name_copy(&self) -> Result<String, String> {
        match self {
            ABBNode::Path(path) => path.get_current_name_copy(),
            ABBNode::Item(item) => item.get_current_name_copy(),
        }
    }

    /// Returns a copy of the ID of this node.
    ///
    /// # Returns
    /// A `Result<String, String>` containing a copy of the node's ID, or an error message
    fn get_id_copy(&self) -> Result<Uuid, String> {
        match self {
            ABBNode::Path(path) => path.get_id_copy(),
            ABBNode::Item(item) => item.get_id_copy(),
        }
    }

    /// Returns the full path of the node as a string.
    ///
    /// # Returns
    /// A `Result<Option<String>, String>` containing the full path of the node, or an error message
    fn get_full_path(&self) -> Result<String, String> {
        match self {
            ABBNode::Path(path) => path.get_full_path(),
            ABBNode::Item(item) => item.get_full_path(),
        }
    }
}
