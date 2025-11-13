pub mod itemnode;
pub mod traits;

pub use crate::common::itemnode::BBItemNode;

// Utility function to split a namespace path into components
pub fn split_path(path: &str) -> Vec<String> {
    path.split(PATH_SEPARATOR)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

pub static PATH_SEPARATOR: char = '.';
