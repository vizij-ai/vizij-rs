//! Blackboard storage and conflict tracking for orchestrator frames.

use std::collections::HashMap;
use std::fmt;

/// Error from a blackboard JSON write: what failed to parse, as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlackboardError(pub String);

impl fmt::Display for BlackboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BlackboardError {}

type Result<T> = std::result::Result<T, BlackboardError>;

use serde::{Deserialize, Serialize};

use crate::{json, Shape, TypedPath, Value, WriteBatch};

/// Single blackboard entry with provenance information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    /// Current normalized value stored at the path.
    pub value: Value,
    /// Optional declared shape associated with [`Self::value`].
    pub shape: Option<Shape>,
    /// Epoch when this entry was last written.
    pub epoch: u64,
    /// Writer/controller label that last updated this entry.
    pub source: String,
    /// Reserved priority field for future conflict policies. Current writes still use last-writer-wins.
    pub priority: u8,
}

impl BlackboardEntry {
    /// Construct a blackboard entry with explicit provenance metadata.
    pub fn new(
        value: Value,
        shape: Option<Shape>,
        epoch: u64,
        source: String,
        priority: u8,
    ) -> Self {
        Self {
            value,
            shape,
            epoch,
            source,
            priority,
        }
    }
}

/// A conflict record produced when a write overwrote an existing entry.
/// Provides prior metadata for diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictLog {
    /// Path that was overwritten.
    pub path: TypedPath,
    /// Value present before the overwrite, if any.
    pub previous_value: Option<Value>,
    /// Shape present before the overwrite, if any.
    pub previous_shape: Option<Shape>,
    /// Epoch of the overwritten entry, if any.
    pub previous_epoch: Option<u64>,
    /// Source label of the overwritten entry, if any.
    pub previous_source: Option<String>,

    /// Replacement value written by the incoming operation.
    pub new_value: Value,
    /// Replacement shape written by the incoming operation.
    pub new_shape: Option<Shape>,
    /// Epoch assigned to the replacement write.
    pub new_epoch: u64,
    /// Source label assigned to the replacement write.
    pub new_source: String,
}

#[derive(Debug, Default)]
pub struct Blackboard {
    // Map from canonical TypedPath -> entry
    inner: HashMap<TypedPath, BlackboardEntry>,
}

impl Blackboard {
    /// Create a new empty blackboard.
    pub fn new() -> Self {
        Self {
            inner: HashMap::new(),
        }
    }

    /// Set a value at a path given as a string.
    ///
    /// The `path` is parsed into a [`TypedPath`]; existing entries at the same
    /// path are overwritten.
    pub fn set(
        &mut self,
        path: &str,
        value: Value,
        shape: Option<Shape>,
        epoch: u64,
        source: String,
    ) -> Result<()> {
        let tp = TypedPath::parse(path)
            .map_err(|e| BlackboardError(format!("typedpath parse error: {}", e)))?;
        let entry = BlackboardEntry::new(value, shape, epoch, source, 0);
        self.inner.insert(tp, entry);
        Ok(())
    }

    /// Set a value from JSON payloads: the value goes through the api-core
    /// normalizer ([`json::parse_value`], accepting every payload form vizij
    /// hosts have emitted) and the shape through plain serde. Existing entries
    /// at the same path are overwritten.
    pub fn set_json(
        &mut self,
        path: &str,
        value_json: serde_json::Value,
        shape_json: Option<serde_json::Value>,
        epoch: u64,
        source: String,
    ) -> Result<()> {
        let value: Value = json::parse_value(value_json)
            .map_err(|e| BlackboardError(format!("value deserialize: {}", e)))?;
        let shape: Option<Shape> = match shape_json {
            Some(sj) => Some(
                serde_json::from_value(sj)
                    .map_err(|e| BlackboardError(format!("shape deserialize: {}", e)))?,
            ),
            None => None,
        };
        self.set(path, value, shape, epoch, source)
    }

    /// Directly set a value using typed API types.
    ///
    /// Existing entries at the same path are overwritten and returned.
    pub fn set_entry(
        &mut self,
        path: TypedPath,
        entry: BlackboardEntry,
    ) -> Option<BlackboardEntry> {
        self.inner.insert(path, entry)
    }

    /// Get an entry by path string. Returns `None` if absent or if the path fails to parse.
    pub fn get(&self, path: &str) -> Option<&BlackboardEntry> {
        if let Ok(tp) = TypedPath::parse(path) {
            self.inner.get(&tp)
        } else {
            None
        }
    }

    /// Fetch an entry using a pre-parsed TypedPath (avoids re-parse per tick).
    pub fn get_tp(&self, path: &TypedPath) -> Option<&BlackboardEntry> {
        self.inner.get(path)
    }

    /// Remove an entry by path string. Returns `None` if absent or if the path fails to parse.
    pub fn remove(&mut self, path: &str) -> Option<BlackboardEntry> {
        if let Ok(tp) = TypedPath::parse(path) {
            self.inner.remove(&tp)
        } else {
            None
        }
    }

    /// Iterate over all entries in unspecified hash-map order.
    pub fn iter(&self) -> impl Iterator<Item = (&TypedPath, &BlackboardEntry)> {
        self.inner.iter()
    }

    /// Apply a [`WriteBatch`] onto the blackboard using last-writer-wins semantics.
    ///
    /// Batch order determines the final value when multiple ops target the same path. Returned
    /// conflict logs include both the overwritten entry metadata and the replacement metadata.
    pub fn apply_writebatch(
        &mut self,
        batch: WriteBatch,
        epoch: u64,
        source: String,
    ) -> Vec<ConflictLog> {
        let mut conflicts = Vec::new();

        for op in batch.into_vec() {
            let tp = op.path;
            let new_value = op.value;
            let new_shape = op.shape;
            let mut conflict = None;

            if let Some(prev) = self.inner.get(&tp) {
                // record conflict
                conflict = Some(ConflictLog {
                    path: tp.clone(),
                    previous_value: Some(prev.value.clone()),
                    previous_shape: prev.shape.clone(),
                    previous_epoch: Some(prev.epoch),
                    previous_source: Some(prev.source.clone()),
                    new_value: new_value.clone(),
                    new_shape: new_shape.clone(),
                    new_epoch: epoch,
                    new_source: source.clone(),
                });
            }

            // last-writer-wins: overwrite unconditionally
            let entry = BlackboardEntry::new(new_value, new_shape, epoch, source.clone(), 0);
            self.inner.insert(tp, entry);

            if let Some(c) = conflict {
                conflicts.push(c);
            }
        }

        conflicts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{as_vec3, float, vec3};
    use crate::{WriteBatch, WriteOp};

    #[test]
    fn set_and_get_entry() {
        let mut bb = Blackboard::new();
        let path = TypedPath::parse("a/b.c").expect("parse path");
        let entry = BlackboardEntry::new(vec3([1.0, 2.0, 3.0]), None, 1, "test".into(), 0);
        bb.set_entry(path.clone(), entry);

        let got = bb.get("a/b.c").expect("entry missing");
        assert_eq!(as_vec3(&got.value), Some([1.0, 2.0, 3.0]));
        assert_eq!(got.epoch, 1);
        assert_eq!(got.source, "test");
    }

    #[test]
    fn apply_writebatch_conflict() {
        let mut bb = Blackboard::new();
        // initial
        let path = TypedPath::parse("x.y").unwrap();
        let entry = BlackboardEntry::new(float(0.5), None, 1, "init".into(), 0);
        bb.set_entry(path.clone(), entry);

        // incoming batch overwrites
        let mut batch = WriteBatch::new();
        batch.push(WriteOp::new(path.clone(), float(0.75)));

        let conflicts = bb.apply_writebatch(batch, 2, "anim".into());
        assert_eq!(conflicts.len(), 1);
        let c = &conflicts[0];
        assert_eq!(c.previous_epoch, Some(1));
        assert_eq!(c.previous_source.as_deref(), Some("init"));
        assert_eq!(c.new_epoch, 2);
        assert_eq!(c.new_source, "anim");
    }
}
