//! TypedPath parsing and formatting.
//!
//! Grammar (simple, engine-agnostic):
//!   namespace/.../target.field.subfield[.index]
//! - '/' separates namespace segments
//! - The last '/'-separated segment contains the `target` and optional `.`-separated fields
//!   Examples:
//!   "robot1/Arm/Joint3.angle" -> namespaces=["robot1","Arm"], target="Joint3", fields=["angle"]
//!   "robot1/Camera0/Intrinsics.fx" -> namespaces=["robot1","Camera0"], target="Intrinsics", fields=["fx"]
//!   "root/node" -> namespaces=["root"], target="node", fields=[]
//!
//! TypedPath is intentionally simple and string-based; adapters (e.g., Bevy) may
//! parse and resolve it into engine-specific bindings.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// Canonical identifier for a value target.
///
/// The string form uses `/` to separate namespaces and `.` to address fields
/// within the target (for example `"robot/Arm/Joint.angle"`). Serialization
/// uses this string form so paths round-trip across Rust and wasm.
///
/// # Examples
///
/// ```rust
/// use vizij_api_core::TypedPath;
///
/// let path = TypedPath::parse("robot/Arm/Joint.angle")?;
/// assert_eq!(path.target_name(), "Joint");
/// # Ok::<(), String>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypedPath {
    /// Namespace segments preceding the target (may be empty)
    pub namespaces: Vec<String>,
    /// Target name (last segment before field selectors)
    pub target: String,
    /// Ordered field selectors on the target (may be empty)
    pub fields: Vec<String>,
}

impl TypedPath {
    /// Construct a typed path from components.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::TypedPath;
    ///
    /// let path = TypedPath::new(
    ///     vec!["robot".to_string(), "Arm".to_string()],
    ///     "Joint3",
    ///     vec!["angle".to_string()],
    /// );
    /// assert_eq!(path.to_string(), "robot/Arm/Joint3.angle");
    /// ```
    pub fn new(namespaces: Vec<String>, target: impl Into<String>, fields: Vec<String>) -> Self {
        Self {
            namespaces,
            target: target.into(),
            fields,
        }
    }

    /// Parse a path string according to the grammar described above.
    ///
    /// # Errors
    ///
    /// Returns an error string when the path is empty, has empty segments, or
    /// contains whitespace in any segment.
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.is_empty() {
            return Err("empty path".to_string());
        }
        // Split by '/'
        let mut parts: Vec<&str> = s.split('/').collect();
        if parts.is_empty() {
            return Err("invalid path".to_string());
        }
        if parts.iter().any(|seg| seg.is_empty()) {
            return Err("invalid typed path: empty namespace segment".to_string());
        }
        // The last segment contains target and optional '.' fields
        let last = parts.pop().unwrap();
        if last.is_empty() {
            return Err("path ends with '/'".to_string());
        }
        let mut last_parts: Vec<&str> = last.split('.').collect();
        if last_parts.is_empty() {
            return Err("invalid target segment".to_string());
        }
        let target = last_parts.remove(0);
        if target.is_empty() {
            return Err("invalid typed path: empty target name".to_string());
        }
        if target.chars().any(char::is_whitespace) {
            return Err("invalid typed path: target contains whitespace".to_string());
        }
        let fields: Vec<String> = last_parts.into_iter().map(|s| s.to_string()).collect();

        if parts.iter().any(|seg| seg.chars().any(char::is_whitespace)) {
            return Err("invalid typed path: namespace contains whitespace".to_string());
        }
        let namespaces = parts.into_iter().map(|s| s.to_string()).collect();

        if fields.iter().any(|seg| seg.is_empty()) {
            return Err("invalid typed path: empty field segment".to_string());
        }
        if fields
            .iter()
            .any(|seg| seg.chars().any(char::is_whitespace))
        {
            return Err("invalid typed path: field contains whitespace".to_string());
        }

        Ok(TypedPath {
            namespaces,
            target: target.to_string(),
            fields,
        })
    }

    /// Return a namespace segment by index, or `None` if out of bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::TypedPath;
    ///
    /// let path = TypedPath::parse("robot/Arm/Joint.angle")?;
    /// assert_eq!(path.namespace_segment(0), Some("robot"));
    /// assert_eq!(path.namespace_segment(2), None);
    /// # Ok::<(), String>(())
    /// ```
    pub fn namespace_segment(&self, index: usize) -> Option<&str> {
        self.namespaces.get(index).map(|s| s.as_str())
    }

    /// Iterate over all namespace segments.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::TypedPath;
    ///
    /// let path = TypedPath::parse("robot/Arm/Joint.angle")?;
    /// let parts: Vec<&str> = path.namespaces().collect();
    /// assert_eq!(parts, vec!["robot", "Arm"]);
    /// # Ok::<(), String>(())
    /// ```
    pub fn namespaces(&self) -> impl Iterator<Item = &str> {
        self.namespaces.iter().map(|s| s.as_str())
    }

    /// Return the target component of the path.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::TypedPath;
    ///
    /// let path = TypedPath::parse("robot/Arm/Joint.angle")?;
    /// assert_eq!(path.target_name(), "Joint");
    /// # Ok::<(), String>(())
    /// ```
    pub fn target_name(&self) -> &str {
        &self.target
    }

    /// Iterate over field selectors on the target.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use vizij_api_core::TypedPath;
    ///
    /// let path = TypedPath::parse("robot/Arm/Joint.angle")?;
    /// let fields: Vec<&str> = path.fields().collect();
    /// assert_eq!(fields, vec!["angle"]);
    /// # Ok::<(), String>(())
    /// ```
    pub fn fields(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(|s| s.as_str())
    }
}

impl fmt::Display for TypedPath {
    /// Internal helper for `fmt`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<String> = self.namespaces.clone();
        if self.fields.is_empty() {
            parts.push(self.target.clone());
        } else {
            let mut last = self.target.clone();
            last.push('.');
            last.push_str(&self.fields.join("."));
            parts.push(last);
        }
        f.write_str(&parts.join("/"))
    }
}

impl FromStr for TypedPath {
    type Err = String;
    /// Creates str (returns an error on invalid input).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TypedPath::parse(s)
    }
}

// Serde support: serialize as string, deserialize from string
impl Serialize for TypedPath {
    /// Internal helper for `serialize` (returns an error on invalid input).
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TypedPath {
    /// Internal helper for `deserialize` (returns an error on invalid input).
    fn deserialize<D>(deserializer: D) -> Result<TypedPath, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        TypedPath::parse(&s).map_err(de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    /// Parses simple.
    fn parse_simple() {
        let p = TypedPath::parse("robot1/Arm/Joint3.angle").unwrap();
        assert_eq!(p.namespaces, vec!["robot1".to_string(), "Arm".to_string()]);
        assert_eq!(p.target, "Joint3");
        assert_eq!(p.fields, vec!["angle".to_string()]);
        assert_eq!(p.to_string(), "robot1/Arm/Joint3.angle");
    }

    #[test]
    /// Parses no fields.
    fn parse_no_fields() {
        let p = TypedPath::parse("root/node").unwrap();
        assert_eq!(p.namespaces, vec!["root".to_string()]);
        assert_eq!(p.target, "node");
        assert!(p.fields.is_empty());
        assert_eq!(p.to_string(), "root/node");
    }

    #[test]
    /// Parses only target.
    fn parse_only_target() {
        let p = TypedPath::parse("node").unwrap();
        assert!(p.namespaces.is_empty());
        assert_eq!(p.target, "node");
        assert!(p.fields.is_empty());
        assert_eq!(p.to_string(), "node");
    }

    #[test]
    /// Parses rejects whitespace.
    fn parse_rejects_whitespace() {
        assert!(TypedPath::parse("invalid path").is_err());
        assert!(TypedPath::parse("robot /Arm/Joint").is_err());
        assert!(TypedPath::parse("robot/Arm/Joint with space").is_err());
        assert!(TypedPath::parse("robot/Arm/Joint.field with space").is_err());
    }
}
