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
//! What a key *is* — its type, range, rest value, whether a remote writer may
//! change it — is kept beside the Blackboard ([`DataStore::meta`]): a key is
//! describable before it holds a value, and the Blackboard's entries are values.
//!
//! Like `SimpleDataStore`, this is cheaply cloneable — clones share one store —
//! and change subscriptions are plain std channels.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, RwLock};

use arora_types::data::{
    prefix_covers, DataError, DataStore, Key, KeyMeta, Slot, State, StateChange, Subscription,
};
use vizij_api_core::blackboard::{Blackboard, BlackboardEntry};
use vizij_api_core::{TypedPath, Value};

/// Source label recorded on entries written through the Arora `DataStore` view.
const SOURCE: &str = "arora";

struct Inner {
    blackboard: RwLock<Blackboard>,
    /// What each described key is.
    meta: RwLock<HashMap<Key, KeyMeta>>,
    /// What every key under a prefix is, until a key's own meta says otherwise.
    prefix_meta: RwLock<HashMap<String, KeyMeta>>,
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

    /// Wrap an existing `Blackboard` (e.g. one pre-populated elsewhere).
    pub fn from_blackboard(blackboard: Blackboard) -> Self {
        Self {
            inner: Arc::new(Inner {
                blackboard: RwLock::new(blackboard),
                meta: RwLock::new(HashMap::new()),
                prefix_meta: RwLock::new(HashMap::new()),
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

    fn meta(&self, keys: &[Key]) -> Vec<Option<KeyMeta>> {
        let meta = self.inner.meta.read().unwrap();
        let prefixes = self.inner.prefix_meta.read().unwrap();
        keys.iter()
            .map(|key| {
                // The most specific statement: the key's own, else the deepest
                // subtree covering it.
                meta.get(key).cloned().or_else(|| {
                    prefixes
                        .iter()
                        .filter(|(prefix, _)| prefix_covers(prefix, &key.path))
                        .max_by_key(|(prefix, _)| prefix.len())
                        .map(|(_, meta)| meta.clone())
                })
            })
            .collect()
    }

    fn all_meta(&self) -> HashMap<Key, KeyMeta> {
        self.inner.meta.read().unwrap().clone()
    }

    fn set_meta(&self, meta: HashMap<Key, KeyMeta>) -> Result<(), DataError> {
        self.inner.meta.write().unwrap().extend(meta);
        Ok(())
    }

    fn set_prefix_meta(&self, meta: HashMap<String, KeyMeta>) -> Result<(), DataError> {
        self.inner.prefix_meta.write().unwrap().extend(meta);
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
        // The current state, as the subscription's first change: a subscriber
        // starts from the full picture and stays current from what follows.
        // Taken while holding the subscriber list so a concurrent write lands
        // either in this opening state or in a later change, never in neither.
        let mut subscribers = self.inner.subscribers.lock().unwrap();
        let mut initial = StateChange::new();
        for (key, value) in self.snapshot().storage {
            initial.set.insert(key, value);
        }
        let _ = tx.send(initial);
        subscribers.push(tx);
        Subscription::new(rx)
    }

    fn clone_box(&self) -> Box<dyn DataStore> {
        Box::new(self.clone())
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
        sub.try_recv().expect("opening state");
        store.write(StateChange::set("k", bool_(true))).unwrap();
        assert!(sub.try_recv().expect("change").contains(&Key::from("k")));
    }

    /// A subscription opens on everything the store already holds, so a
    /// subscriber never has to read a snapshot separately and race the
    /// changes that follow.
    #[test]
    fn subscribe_opens_on_the_current_state() {
        let store = BlackboardStore::new();
        store
            .write(StateChange::set("already", bool_(true)))
            .unwrap();

        let sub = store.subscribe();
        let opening = sub.try_recv().expect("opening state");
        assert!(opening.contains(&Key::from("already")));

        store
            .write(StateChange::set("later", bool_(false)))
            .unwrap();
        assert!(sub
            .try_recv()
            .expect("change")
            .contains(&Key::from("later")));
    }

    #[test]
    fn clone_box_is_a_sibling_handle() {
        let store = BlackboardStore::new();
        let sibling = store.clone_box();
        store.write(StateChange::set("k", float(1.0))).unwrap();
        assert_eq!(sibling.read(&[Key::from("k")]), vec![Some(float(1.0))]);
    }

    #[test]
    fn snapshot_returns_all() {
        let store = BlackboardStore::new();
        store.write(StateChange::set("a", float(1.0))).unwrap();
        store.write(StateChange::set("b", float(2.0))).unwrap();
        assert_eq!(store.snapshot().storage.len(), 2);
    }

    /// A key's own meta wins over its subtree's, the deepest subtree over a
    /// shallower one, and a key nothing describes has none — whether or not it
    /// holds a value.
    #[test]
    fn meta_resolves_to_the_most_specific_statement() {
        let store = BlackboardStore::new();
        store
            .write(StateChange::set("face/mouth", float(0.0)))
            .unwrap();
        store
            .set_prefix_meta(HashMap::from([
                (String::new(), KeyMeta::new().described("anything")),
                ("face".to_string(), KeyMeta::new().editable()),
            ]))
            .unwrap();
        store
            .set_meta(HashMap::from([(
                Key::from("face/eyes"),
                KeyMeta::new().range(0.0, 1.0),
            )]))
            .unwrap();
        assert_eq!(
            store.meta(&[
                Key::from("face/eyes"),
                Key::from("face/mouth"),
                Key::from("faceplate"),
            ]),
            vec![
                Some(KeyMeta::new().range(0.0, 1.0)),
                Some(KeyMeta::new().editable()),
                Some(KeyMeta::new().described("anything")),
            ]
        );
        assert_eq!(
            store.all_meta().into_keys().collect::<Vec<_>>(),
            vec![Key::from("face/eyes")],
            "a key is described before it holds a value; a subtree is no key"
        );
        assert_eq!(
            BlackboardStore::new().meta(&[Key::from("face/mouth")]),
            vec![None]
        );
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
