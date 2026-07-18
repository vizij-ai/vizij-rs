//! [`BlackboardStore`]: Vizij's `Blackboard` exposed as an Arora
//! [`DataStore`](arora_types::data::DataStore).
//!
//! Arora's runtime can be spawned with a custom memory (`Arc<dyn DataStore>`),
//! so this lets a Vizij `Blackboard` *be* that memory. Vizij and Arora share
//! one runtime value type ([`vizij_api_core::Value`] is
//! `arora_types::value::Value`), so the `DataStore` view reads and writes the
//! Blackboard's entries directly — the only translation is between string
//! [`Key`]s and [`TypedPath`]s. The Blackboard's richer provenance
//! (epoch/source/shape) is kept Vizij-side; the `DataStore` view exposes just
//! the values.
//!
//! Like `SimpleDataStore`, this is cheaply cloneable — clones share one store —
//! and change subscriptions are plain std channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, RwLock};

use arora_types::data::{DataError, DataStore, Key, Slot, State, StateChange, Subscription};
use vizij_api_core::blackboard::{Blackboard, BlackboardEntry};
use vizij_api_core::{TypedPath, Value};

/// Source label recorded on entries written through the Arora `DataStore` view.
const SOURCE: &str = "arora";

struct Inner {
    blackboard: RwLock<Blackboard>,
    epoch: AtomicU64,
    subscribers: Mutex<Vec<Sender<StateChange>>>,
}

impl Inner {
    fn next_epoch(&self) -> u64 {
        self.epoch.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn notify(&self, change: StateChange) {
        if change.is_empty() {
            return;
        }
        let mut subs = self.subscribers.lock().unwrap();
        subs.retain(|tx| tx.send(change.clone()).is_ok());
    }
}

/// A Vizij `Blackboard` presented as an Arora [`DataStore`]. Clone to share.
#[derive(Clone)]
pub struct BlackboardStore {
    inner: Arc<Inner>,
}

impl Default for BlackboardStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BlackboardStore {
    /// An empty store backed by a fresh `Blackboard`.
    pub fn new() -> Self {
        Self::from_blackboard(Blackboard::new())
    }

    /// Wrap an existing `Blackboard` (e.g. one an orchestrator already populated).
    pub fn from_blackboard(blackboard: Blackboard) -> Self {
        Self {
            inner: Arc::new(Inner {
                blackboard: RwLock::new(blackboard),
                epoch: AtomicU64::new(0),
                subscribers: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Read-only access to the underlying Vizij `Blackboard` (for Vizij-side
    /// consumers that want the richer entries, not just the value view).
    pub fn with_blackboard<R>(&self, f: impl FnOnce(&Blackboard) -> R) -> R {
        f(&self.inner.blackboard.read().unwrap())
    }
}

/// Read one entry's value (`None` if absent or the key is not a valid path).
fn read_one(blackboard: &Blackboard, key: &Key) -> Option<Value> {
    let tp = TypedPath::parse(&key.path).ok()?;
    Some(blackboard.get_tp(&tp)?.value.clone())
}

/// Apply one `(key, value)` to the blackboard at `epoch`. `None` clears the key.
fn apply_one(
    blackboard: &mut Blackboard,
    key: &Key,
    value: &Option<Value>,
    epoch: u64,
) -> Result<(), DataError> {
    match value {
        Some(v) => {
            let tp = TypedPath::parse(&key.path)
                .map_err(|e| DataError::Other(format!("{}: {e}", key.path)))?;
            blackboard.set_entry(
                tp,
                BlackboardEntry::new(v.clone(), None, epoch, SOURCE.to_string(), 0),
            );
        }
        None => {
            blackboard.remove(&key.path);
        }
    }
    Ok(())
}

impl DataStore for BlackboardStore {
    fn read(&self, keys: &[Key]) -> Vec<Option<Value>> {
        let blackboard = self.inner.blackboard.read().unwrap();
        keys.iter().map(|k| read_one(&blackboard, k)).collect()
    }

    fn write(&self, changes: StateChange) -> Result<(), DataError> {
        let epoch = self.inner.next_epoch();
        {
            let mut blackboard = self.inner.blackboard.write().unwrap();
            for (key, value) in &changes.set {
                apply_one(&mut blackboard, key, value, epoch)?;
            }
            for key in &changes.unset {
                blackboard.remove(&key.path);
            }
        }
        self.inner.notify(changes);
        Ok(())
    }

    fn snapshot(&self) -> State {
        let blackboard = self.inner.blackboard.read().unwrap();
        let storage = blackboard
            .iter()
            .map(|(tp, entry)| (Key::new(tp.to_string()), Some(entry.value.clone())))
            .collect();
        State { storage }
    }

    fn slot(&self, key: &Key) -> Box<dyn Slot> {
        Box::new(BlackboardSlot {
            key: key.clone(),
            inner: self.inner.clone(),
        })
    }

    fn subscribe(&self) -> Subscription {
        let (tx, rx) = channel();
        self.inner.subscribers.lock().unwrap().push(tx);
        Subscription::new(rx)
    }
}

/// A handle to one key. The Blackboard does not expose per-cell references, so
/// the slot re-resolves the path on each access (still hits the same storage).
struct BlackboardSlot {
    key: Key,
    inner: Arc<Inner>,
}

impl Slot for BlackboardSlot {
    fn get(&self) -> Option<Value> {
        let blackboard = self.inner.blackboard.read().unwrap();
        read_one(&blackboard, &self.key)
    }

    fn set(&self, value: Option<Value>) -> Result<(), DataError> {
        let epoch = self.inner.next_epoch();
        {
            let mut blackboard = self.inner.blackboard.write().unwrap();
            apply_one(&mut blackboard, &self.key, &value, epoch)?;
        }
        self.inner.notify(StateChange {
            set: HashMap::from([(self.key.clone(), value)]),
            unset: Default::default(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vizij_api_core::value::{as_vec3, bool_, float, vec3};

    #[test]
    fn write_then_read_primitive_and_composite() {
        let store = BlackboardStore::new();
        store
            .write(StateChange::set("rig/joint.angle", float(1.5)))
            .unwrap();
        assert_eq!(
            store.read(&[Key::from("rig/joint.angle")]),
            vec![Some(float(1.5))]
        );

        // A Vizij composite (a `Value::Structure` with a vizij type id),
        // round-tripped.
        let pos = vec3([1.0, 2.0, 3.0]);
        store
            .write(StateChange::set("rig/pos", pos.clone()))
            .unwrap();
        assert_eq!(store.read(&[Key::from("rig/pos")]), vec![Some(pos)]);

        // The Blackboard entry holds the same value with provenance.
        store.with_blackboard(|bb| {
            let entry = bb.get("rig/pos").expect("entry");
            assert_eq!(as_vec3(&entry.value), Some([1.0, 2.0, 3.0]));
            assert_eq!(entry.source, "arora");
        });

        assert_eq!(store.read(&[Key::from("absent")]), vec![None]);
    }

    #[test]
    fn unset_clears_the_key() {
        let store = BlackboardStore::new();
        store.write(StateChange::set("g", bool_(true))).unwrap();
        let mut change = StateChange::new();
        change.unset.insert(Key::from("g"));
        store.write(change).unwrap();
        assert_eq!(store.read(&[Key::from("g")]), vec![None]);
    }

    #[test]
    fn slot_and_store_coincide() {
        let store = BlackboardStore::new();
        let slot = store.slot(&Key::from("x"));
        slot.set(Some(float(2.0))).unwrap();
        assert_eq!(store.read(&[Key::from("x")]), vec![Some(float(2.0))]);
        store.write(StateChange::set("x", float(3.0))).unwrap();
        assert_eq!(slot.get(), Some(float(3.0)));
    }

    #[test]
    fn subscribe_delivers_changes() {
        let store = BlackboardStore::new();
        let sub = store.subscribe();
        store.write(StateChange::set("k", bool_(true))).unwrap();
        assert!(sub.try_recv().expect("change").contains(&Key::from("k")));
    }

    #[test]
    fn snapshot_returns_all() {
        let store = BlackboardStore::new();
        store.write(StateChange::set("a", float(1.0))).unwrap();
        store.write(StateChange::set("b", float(2.0))).unwrap();
        assert_eq!(store.snapshot().storage.len(), 2);
    }

    #[test]
    fn clones_share_storage() {
        let store = BlackboardStore::new();
        let other = store.clone();
        store
            .write(StateChange::set("shared", bool_(true)))
            .unwrap();
        assert_eq!(other.read(&[Key::from("shared")]), vec![Some(bool_(true))]);
    }
}
