//! The rest module: `reset`, which returns the device's keys to where they rest.
//!
//! Where a key rests is what its meta says
//! ([`KeyMeta::default`](arora_types::data::KeyMeta::default)), so a device
//! states its rest pose once, in its store, and `reset` needs nothing else: it
//! reads the store it writes to. The writes go through the store like any
//! other, so every bridge sees them.

use std::collections::HashSet;

use arora::HostModule;
use arora_types::data::{DataStore, Key, StateChange};
use uuid::{uuid, Uuid};

/// The rest contract: one function, `reset`.
#[arora_module::contract(name = "rest")]
pub trait Rest {
    /// Return every key to the value its meta says it rests at.
    #[export(id = "654d13df-30a4-42ee-9664-15e6fa6f07e5")]
    fn reset(&mut self);
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

impl Rest for AtRest {
    fn reset(&mut self) {
        // The keys the store describes and the keys it holds: a subtree's
        // meta can give a rest to a key that has no meta of its own.
        let mut keys: HashSet<Key> = self.store.all_meta().into_keys().collect();
        keys.extend(self.store.snapshot().storage.into_keys());
        let keys: Vec<Key> = keys.into_iter().collect();
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
