//! [`BlackboardStore`]: Vizij's `Blackboard` exposed as an Arora
//! [`DataStore`](arora_types::data::DataStore).
//!
//! Arora's runtime can be spawned with a custom memory (`Arc<dyn DataStore>`),
//! so this lets a Vizij `Blackboard` *be* that memory. The `DataStore` interface
//! speaks the Arora `Value` vocabulary and string [`Key`]s; internally we store
//! Vizij values keyed by `TypedPath`, bridging each way through
//! [`vizij_arora`]. The Blackboard's richer provenance (epoch/source/shape) is
//! kept Vizij-side; the `DataStore` view exposes just the values.
//!
//! Like `SimpleDataStore`, this is cheaply cloneable — clones share one store —
//! and change subscriptions are plain std channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, RwLock};

use arora_types::data::{DataError, DataStore, Key, Slot, State, StateChange, Subscription};
use arora_types::value::Value as AValue;
use vizij_api_core::TypedPath;
use vizij_arora::{from_arora, to_arora};
use vizij_orchestrator::blackboard::{Blackboard, BlackboardEntry};

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
    /// consumers that want the richer entries, not just the Arora value view).
    pub fn with_blackboard<R>(&self, f: impl FnOnce(&Blackboard) -> R) -> R {
        f(&self.inner.blackboard.read().unwrap())
    }
}

/// Read one Vizij entry as an Arora value (`None` if absent, unparsable, or
/// not convertible).
fn read_one(blackboard: &Blackboard, key: &Key) -> Option<AValue> {
    let tp = TypedPath::parse(&key.path).ok()?;
    let entry = blackboard.get_tp(&tp)?;
    to_arora(&entry.value).ok()
}

/// Apply one `(key, value)` to the blackboard at `epoch`. `None` clears the key.
fn apply_one(
    blackboard: &mut Blackboard,
    key: &Key,
    value: &Option<AValue>,
    epoch: u64,
) -> Result<(), DataError> {
    match value {
        Some(av) => {
            let tp = TypedPath::parse(&key.path)
                .map_err(|e| DataError::Other(format!("{}: {e}", key.path)))?;
            let vv = from_arora(av).map_err(|e| DataError::Other(format!("{}: {e}", key.path)))?;
            blackboard.set_entry(
                tp,
                BlackboardEntry::new(vv, None, epoch, SOURCE.to_string(), 0),
            );
        }
        None => {
            blackboard.remove(&key.path);
        }
    }
    Ok(())
}

impl DataStore for BlackboardStore {
    fn read(&self, keys: &[Key]) -> Vec<Option<AValue>> {
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
            .filter_map(|(tp, entry)| {
                let value = to_arora(&entry.value).ok()?;
                Some((Key::new(tp.to_string()), Some(value)))
            })
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
    fn get(&self) -> Option<AValue> {
        let blackboard = self.inner.blackboard.read().unwrap();
        read_one(&blackboard, &self.key)
    }

    fn set(&self, value: Option<AValue>) -> Result<(), DataError> {
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
    use vizij_api_core::Value as VValue;

    fn av(v: VValue) -> AValue {
        to_arora(&v).unwrap()
    }

    #[test]
    fn write_then_read_primitive_and_composite() {
        let store = BlackboardStore::new();
        store
            .write(StateChange::set("rig/joint.angle", av(VValue::Float(1.5))))
            .unwrap();
        assert_eq!(
            store.read(&[Key::from("rig/joint.angle")]),
            vec![Some(AValue::F32(1.5))]
        );

        // A Vizij composite stored as a Value::Structure, round-tripped.
        let vec3 = av(VValue::Vec3([1.0, 2.0, 3.0]));
        store
            .write(StateChange::set("rig/pos", vec3.clone()))
            .unwrap();
        assert_eq!(store.read(&[Key::from("rig/pos")]), vec![Some(vec3)]);

        // Internally it is a Vizij Vec3 (not an opaque blob).
        store.with_blackboard(|bb| {
            let entry = bb.get("rig/pos").expect("entry");
            assert!(matches!(entry.value, VValue::Vec3(_)));
            assert_eq!(entry.source, "arora");
        });

        assert_eq!(store.read(&[Key::from("absent")]), vec![None]);
    }

    #[test]
    fn unset_clears_the_key() {
        let store = BlackboardStore::new();
        store
            .write(StateChange::set("g", av(VValue::Bool(true))))
            .unwrap();
        let mut change = StateChange::new();
        change.unset.insert(Key::from("g"));
        store.write(change).unwrap();
        assert_eq!(store.read(&[Key::from("g")]), vec![None]);
    }

    #[test]
    fn slot_and_store_coincide() {
        let store = BlackboardStore::new();
        let slot = store.slot(&Key::from("x"));
        slot.set(Some(av(VValue::Float(2.0)))).unwrap();
        assert_eq!(store.read(&[Key::from("x")]), vec![Some(AValue::F32(2.0))]);
        store
            .write(StateChange::set("x", av(VValue::Float(3.0))))
            .unwrap();
        assert_eq!(slot.get(), Some(AValue::F32(3.0)));
    }

    #[test]
    fn subscribe_delivers_changes() {
        let store = BlackboardStore::new();
        let sub = store.subscribe();
        store
            .write(StateChange::set("k", av(VValue::Bool(true))))
            .unwrap();
        assert!(sub.try_recv().expect("change").contains(&Key::from("k")));
    }

    #[test]
    fn snapshot_returns_all() {
        let store = BlackboardStore::new();
        store
            .write(StateChange::set("a", av(VValue::Float(1.0))))
            .unwrap();
        store
            .write(StateChange::set("b", av(VValue::Float(2.0))))
            .unwrap();
        assert_eq!(store.snapshot().storage.len(), 2);
    }

    #[test]
    fn clones_share_storage() {
        let store = BlackboardStore::new();
        let other = store.clone();
        store
            .write(StateChange::set("shared", av(VValue::Bool(true))))
            .unwrap();
        assert_eq!(
            other.read(&[Key::from("shared")]),
            vec![Some(AValue::Boolean(true))]
        );
    }
}
