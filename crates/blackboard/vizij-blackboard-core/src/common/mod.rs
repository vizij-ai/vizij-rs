pub mod itemnode;
pub mod traits;

pub use crate::common::itemnode::BBItemNode;

// Utility function to split a namespace path into components
pub fn split_path(path: &str, separator: char) -> Vec<String> {
    path.split(separator)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

pub static DEFAULT_PATH_SEPARATOR: char = '.';
