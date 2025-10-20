pub mod arc_abb_itemnode;
pub mod arc_abb_node;
pub mod arc_abb_pathnode;
pub mod arc_abb_traits;

pub use crate::arc_abb::arc_abb_itemnode::ABBItemNode;
pub use crate::arc_abb::arc_abb_node::ArcABBNode;
pub use crate::arc_abb::arc_abb_pathnode::ArcABBPathNode;
pub use crate::arc_abb::arc_abb_traits::{
    ArcABBPathNodeTrait, ArcAroraBlackboardTrait, ArcNamespacedSetterTrait,
};
pub use crate::blackboard_ref::{
    BlackboardInterface, BlackboardNodeAccess, BlackboardRef, BlackboardType,
};

// Utility function to split a namespace path into components
pub fn split_path(path: &str) -> Vec<String> {
    path.split('.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
