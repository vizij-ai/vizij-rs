use anyhow::{anyhow, Result};
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use vizij_api_core::{json, Shape, TypedPath, Value, WriteBatch};

/// Single blackboard entry with provenance information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub value: Value,
    pub shape: Option<Shape>,
    pub epoch: u64,
    pub source: String,
    pub priority: u8,
}

impl BlackboardEntry {
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
    pub path: TypedPath,
    pub previous_value: Option<Value>,
    pub previous_shape: Option<Shape>,
    pub previous_epoch: Option<u64>,
    pub previous_source: Option<String>,

    pub new_value: Value,
    pub new_shape: Option<Shape>,
    pub new_epoch: u64,
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

    /// Set a value on the blackboard from JSON-like values.
    ///
    /// This convenience accepts JSON values (serde_json::Value) which are converted
    /// into the workspace `vizij_api_core::Value` and `Shape` types. The `path` is
    /// provided as a String and parsed into a `TypedPath`.
    pub fn set(
        &mut self,
        path: String,
        value_json: serde_json::Value,
        shape_json: Option<serde_json::Value>,
        epoch: u64,
        source: String,
    ) -> Result<()> {
        // Parse path
        let tp = TypedPath::parse(&path).map_err(|e| anyhow!("typedpath parse error: {}", e))?;

        // Convert JSON into Value
        let value: Value =
            json::parse_value(value_json).map_err(|e| anyhow!("value deserialize: {}", e))?;

        // Optional shape
        let shape: Option<Shape> = match shape_json {
            Some(sj) => {
                Some(serde_json::from_value(sj).map_err(|e| anyhow!("shape deserialize: {}", e))?)
            }
            None => None,
        };

        let entry = BlackboardEntry::new(value, shape, epoch, source, 0);
        self.inner.insert(tp, entry);
        Ok(())
    }

    /// Directly set a value using typed API types.
    pub fn set_entry(
        &mut self,
        path: TypedPath,
        entry: BlackboardEntry,
    ) -> Option<BlackboardEntry> {
        self.inner.insert(path, entry)
    }

    /// Get an entry by path string. Returns None if absent or parse fails.
    pub fn get(&self, path: &str) -> Option<&BlackboardEntry> {
        if let Ok(tp) = TypedPath::parse(path) {
            self.inner.get(&tp)
        } else {
            None
        }
    }

    /// Remove an entry by path string. Returns the removed entry if present.
    pub fn remove(&mut self, path: &str) -> Option<BlackboardEntry> {
        if let Ok(tp) = TypedPath::parse(path) {
            self.inner.remove(&tp)
        } else {
            None
        }
    }

    /// Iterate over all entries.
    pub fn iter(&self) -> impl Iterator<Item = (&TypedPath, &BlackboardEntry)> {
        self.inner.iter()
    }

    /// Apply a WriteBatch onto the blackboard using last-writer-wins semantics.
    /// Returns a Vec<ConflictLog> describing any overwrites that occurred.
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
    use vizij_api_core::{Value, WriteBatch, WriteOp};

    #[test]
    fn set_and_get_entry() {
        let mut bb = Blackboard::new();
        let path = TypedPath::parse("a/b.c").expect("parse path");
        let entry = BlackboardEntry::new(Value::Vec3([1.0, 2.0, 3.0]), None, 1, "test".into(), 0);
        bb.set_entry(path.clone(), entry);

        let got = bb.get("a/b.c").expect("entry missing");
        match &got.value {
            Value::Vec3(v) => {
                assert_eq!(v, &[1.0, 2.0, 3.0]);
            }
            _ => panic!("unexpected value variant"),
        }
        assert_eq!(got.epoch, 1);
        assert_eq!(got.source, "test");
    }

    #[test]
    fn apply_writebatch_conflict() {
        let mut bb = Blackboard::new();
        // initial
        let path = TypedPath::parse("x.y").unwrap();
        let entry = BlackboardEntry::new(Value::Float(0.5), None, 1, "init".into(), 0);
        bb.set_entry(path.clone(), entry);

        // incoming batch overwrites
        let mut batch = WriteBatch::new();
        batch.push(WriteOp::new(path.clone(), Value::Float(0.75)));

        let conflicts = bb.apply_writebatch(batch, 2, "anim".into());
        assert_eq!(conflicts.len(), 1);
        let c = &conflicts[0];
        assert_eq!(c.previous_epoch, Some(1));
        assert_eq!(c.previous_source.as_deref(), Some("init"));
        assert_eq!(c.new_epoch, 2);
        assert_eq!(c.new_source, "anim");
    }
}
