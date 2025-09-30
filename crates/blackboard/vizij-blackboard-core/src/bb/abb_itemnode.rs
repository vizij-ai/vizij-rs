use crate::{adt, bb::ABBNodeTrait};
use arora_schema::{
    gen_bb_uuid,
    value::{Type, Value},
};
use uuid::Uuid;

/// A node that represents a value item in the blackboard structure
#[derive(Debug, Clone)]
pub struct ABBItemNode {
    /// The name of the item
    name: String,
    /// The type of the value
    value_type: Type,
    /// The unique ID of the item
    id: Uuid,
    /// The value of the item, if any
    value: Option<Value>,
    // Full path for this node
    full_path: String,
}

impl ABBItemNode {
    /// Constructor from just a name and type
    pub fn new_with_full_path(
        name: String,
        value_type: Type,
        ref_id: Option<Uuid>,
        full_path: &String,
    ) -> Option<Self> {
        if name.is_empty() {
            // We don't want to create nodes with empty names
            return None;
        }

        // Generate a unique ID for the value if not provided
        let ref_id = if let Some(id) = ref_id {
            id
        } else {
            gen_bb_uuid()
        };

        Some(Self {
            name,
            value_type,
            id: ref_id,
            value: None,
            full_path: full_path.clone(),
        })
    }

    /// Constructor that accepts a Value and infers the type
    pub fn from_value(
        name_ref: &String,
        value: Value,
        id: Uuid,
        full_path: &String,
    ) -> Result<Option<Self>, String> {
        if name_ref.is_empty() {
            return Err("Name cannot be empty".to_string());
        }

        let value_type = adt::utils::get_type_for_value(&value);
        let name = name_ref.clone();

        Ok(Some(Self {
            name,
            value_type,
            id,
            value: Some(value),
            full_path: full_path.clone(),
        }))
    }

    /// Adds a method to set the value stored in this item node
    pub fn set_value(&mut self, value: Value) -> bool {
        // Ensure value matches the type
        if !adt::utils::is_compatible(
            &value,
            &adt::utils::default_value_for_type(&self.value_type),
        ) {
            return false;
        }

        self.value = Some(value);
        true
    }

    /// Adds a method to get the value stored in this item node
    pub fn get_value(&self) -> Option<&Value> {
        self.value.as_ref()
    }

    /// Returns the type of the value stored in this item node
    pub fn get_value_type(&self) -> &Type {
        &self.value_type
    }
}

/// Implementation of `ABBNodeTrait` for `ABBItemNode`
///
/// This trait provides methods to access the name, ID, and path status of the node.
impl ABBNodeTrait for ABBItemNode {
    /// Returns the unique ID of the item
    fn get_id_ref(&self) -> Result<&Uuid, String> {
        Ok(&self.id)
    }

    /// Returns whether this node is a path
    /// (always false for item nodes)
    fn is_path(&self) -> Result<bool, String> {
        Ok(false)
    }

    /// Returns the name of the node
    fn get_current_name_copy(&self) -> Result<String, String> {
        Ok(self.name.clone())
    }

    /// Returns a copy of the unique ID of the item
    fn get_id_copy(&self) -> Result<Uuid, String> {
        Ok(self.id.clone())
    }

    /// Returns the full path of the node as a string
    fn get_full_path(&self) -> Result<String, String> {
        // For item nodes, the full path is just the name
        Ok(self.full_path.clone())
    }
}
