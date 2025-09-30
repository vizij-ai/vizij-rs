pub mod abb_itemnode;
pub mod abb_node;
pub mod abb_pathnode;
pub mod abb_traits;
pub mod blackboard_ref;

pub use crate::bb::abb_itemnode::ABBItemNode;
pub use crate::bb::abb_node::ArcABBNode;
pub use crate::bb::abb_pathnode::ArcABBPathNode;
pub use crate::bb::abb_traits::{
    ABBNodeTrait, ArcABBPathNodeTrait, ArcAroraBlackboardTrait, ArcNamespacedSetterTrait,
    ItemsFormattable, TreeFormattable,
};

// Utility function to split a namespace path into components
pub fn split_path(path: &String) -> Vec<String> {
    path.split('.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
