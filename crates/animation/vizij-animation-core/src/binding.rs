#![allow(dead_code)]
//! Binding table and resolver traits.
//!
//! v1 uses small string keys as TargetHandle. The table maps animation channel
//! identifiers (per-animation track) to resolved target handles, and each instance holds
//! a BindingSet referencing rows in the global table. Population happens in prebind().

/// Opaque target handle for v1 (small string key).
pub type TargetHandle = String;

use crate::ids::AnimId;

/// Channel key: (Animation, Track Index).
/// This uniquely identifies a channel within the entire engine.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct ChannelKey {
    pub anim: AnimId,
    pub track_idx: u32,
}

/// Trait for resolving canonical target paths to opaque handles.
/// Adapters (Bevy/WASM) implement this and pass into Engine::prebind().
pub trait TargetResolver {
    fn resolve(&mut self, path: &str) -> Option<TargetHandle>;
}

/// One row in the global binding table.
#[derive(Clone, Debug)]
pub struct BindingRow {
    pub channel: ChannelKey,
    pub handle: TargetHandle,
}

/// Global binding table shared across players/instances.
#[derive(Default, Debug)]
pub struct BindingTable {
    pub rows: Vec<BindingRow>,
}

impl BindingTable {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    /// Look up an existing row by channel key.
    pub fn get(&self, channel: ChannelKey) -> Option<&BindingRow> {
        self.rows.iter().find(|r| r.channel == channel)
    }

    /// Insert or update a binding row for a channel.
    pub fn upsert(&mut self, channel: ChannelKey, handle: TargetHandle) {
        if let Some(row) = self.rows.iter_mut().find(|r| r.channel == channel) {
            row.handle = handle;
        } else {
            self.rows.push(BindingRow { channel, handle });
        }
    }
}

/// Per-instance view over a set of bound channels (indices into BindingTable.rows).
#[derive(Clone, Debug, Default)]
pub struct BindingSet {
    pub channels: Vec<ChannelKey>,
}

impl BindingSet {
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }
}
