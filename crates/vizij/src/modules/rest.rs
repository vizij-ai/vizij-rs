//! The rest module: `reset`, which returns the device's keys to where they
//! rest, and `reset_keys`, which returns the keys it names.
//!
//! Where a key rests is what its meta says
//! ([`KeyMeta::default`](arora_types::data::KeyMeta::default)), so a device
//! states its rest pose once, in its store, and the module needs nothing
//! else: it reads the store it writes to. A key with no declared rest keeps
//! its value. The writes go through the store like any other, so every
//! bridge sees them.
//!
//! Nothing rests a key on its own: a halted run's outputs hold their last
//! values, and a client that wants them back at rest says so with
//! `reset_keys` — the keys it knows the run wrote.

use std::collections::HashSet;

use arora::HostModule;
use arora_types::data::{DataStore, Key, StateChange};
use uuid::{uuid, Uuid};

/// The rest contract: `reset` and `reset_keys`.
#[arora_module::contract(name = "rest")]
pub trait Rest {
    /// Return every key to the value its meta says it rests at.
    #[export(id = "654d13df-30a4-42ee-9664-15e6fa6f07e5")]
    fn reset(&mut self);

    /// Return each of `keys` to the value its meta says it rests at.
    #[export(id = "ce6e5dc9-8f5a-43c8-89b8-80ced7763a2c")]
    fn reset_keys(
        &mut self,
        #[param(id = "6aa219d3-ec55-4837-8bf1-bfdddeb1d8fa")] keys: Vec<String>,
    );
}

/// The id the rest module is registered under.
pub const MODULE_ID: Uuid = uuid!("0cd506ad-98e8-47ee-b55d-8b0d3d6f3d4c");

/// The rest module over `store`: the device's own store, or a sibling handle
/// onto it.
pub fn host_module(store: Box<dyn DataStore>) -> HostModule {
    HostModule::from_exports(MODULE_ID, rest::exports(AtRest { store }))
}

struct AtRest {
    store: Box<dyn DataStore>,
}

impl AtRest {
    /// Write each of `keys` that has a declared rest back to it.
    fn rest(&self, keys: Vec<Key>) {
        let meta = self.store.meta(&keys);
        let mut change = StateChange::new();
        for (key, meta) in keys.into_iter().zip(meta) {
            if let Some(rest) = meta.and_then(|meta| meta.default) {
                change.set.insert(key, Some(rest));
            }
        }
        if let Err(e) = self.store.write(change) {
            log::error!("reset: {e:?}");
        }
    }
}

impl Rest for AtRest {
    fn reset(&mut self) {
        // The keys the store describes and the keys it holds: a subtree's
        // meta can give a rest to a key that has no meta of its own.
        let mut keys: HashSet<Key> = self.store.all_meta().into_keys().collect();
        keys.extend(self.store.snapshot().storage.into_keys());
        self.rest(keys.into_iter().collect());
    }

    fn reset_keys(&mut self, keys: Vec<String>) {
        self.rest(keys.into_iter().map(Key::from).collect());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arora_types::data::KeyMeta;
    use vizij_api_core::value::float;
    use vizij_arora_store::BlackboardStore;

    fn read(store: &BlackboardStore, key: &str) -> Option<arora_types::value::Value> {
        store.read(&[Key::from(key)]).into_iter().next().flatten()
    }

    /// `reset_keys` rests the keys it names that declare a rest, and leaves
    /// every other key as it is.
    #[test]
    fn reset_keys_rests_the_named_keys_only() {
        let store = BlackboardStore::new();
        store
            .set_meta(
                [
                    (Key::from("a"), KeyMeta::new().resting_at(float(0.0))),
                    (Key::from("b"), KeyMeta::new().resting_at(float(0.0))),
                ]
                .into(),
            )
            .unwrap();
        let mut moved = StateChange::new();
        for key in ["a", "b", "c"] {
            moved.set.insert(Key::from(key), Some(float(1.0)));
        }
        store.write(moved).unwrap();

        let mut rest = AtRest {
            store: Box::new(store.clone()),
        };
        rest.reset_keys(vec!["a".to_string(), "c".to_string()]);
        assert_eq!(read(&store, "a"), Some(float(0.0)), "named, resting");
        assert_eq!(read(&store, "b"), Some(float(1.0)), "not named");
        assert_eq!(
            read(&store, "c"),
            Some(float(1.0)),
            "named, no rest declared"
        );
    }
}
