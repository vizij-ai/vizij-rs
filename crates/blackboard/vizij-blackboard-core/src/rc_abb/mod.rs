pub mod abb_node;
pub mod abb_pathnode;
pub mod abb_traits;

pub use crate::rc_abb::abb_node::ABBNode;
pub use crate::rc_abb::abb_pathnode::ABBPathNode;
pub use crate::rc_abb::abb_traits::{ABBPathNodeTrait, NamespacedSetterTrait};
//pub use crate::arc_bb::blackboard_ref::{
//    BlackboardInterface, BlackboardNodeAccess, BlackboardRef, BlackboardType,
//};

// Utility function to split a namespace path into components
pub fn split_path(path: &str) -> Vec<String> {
    path.split('.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
