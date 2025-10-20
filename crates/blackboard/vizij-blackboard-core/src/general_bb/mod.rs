pub mod traits;

// Utility function to split a namespace path into components
pub fn split_path(path: &str) -> Vec<String> {
    path.split('.')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}
