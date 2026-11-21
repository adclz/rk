// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The handles of a resource a component owns, as another store holds them,
//! the way runtimes that link components across stores on wasmtime keep
//! them (wasmCloud's `wash-runtime`, `engine/store/resource_bridge.rs`). The
//! store that owns the resource keeps its handle in a [`ResourceRegistry`],
//! under an id; any other store holds a [`ProxyResource`] with that id, one
//! host resource type for every resource it imports. A store never touches
//! another's handles: a value crosses through [`crate::relocate`]. An owned
//! handle passed back to its owner leaves the registry, a borrowed one stays,
//! and one its holder drops is staged in the registry and dropped in the
//! owner's store ([`ResourceRegistry::stage_drop`]).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use wasmtime::component::{ResourceAny, ResourceTable, ResourceType};

/// The id the next handle a registry takes is known by: ids are never
/// reused.
static NEXT_PROXY_ID: AtomicU64 = AtomicU64::new(0);

/// What a store that does not own a resource holds for one of its handles:
/// the id the owner's registry knows it by.
pub struct ProxyResource {
    pub proxy_id: u64,
}

/// The host resource type every resource a store holds a proxy of is.
pub fn proxy_resource_type() -> ResourceType {
    ResourceType::host::<ProxyResource>()
}

/// The handles a store owns that another store holds, by id, and those the
/// holder let go of, to drop in the owner's store.
#[derive(Default)]
pub struct ResourceRegistry {
    reals: BTreeMap<u64, ResourceAny>,
    pending_drops: Vec<ResourceAny>,
}

impl ResourceRegistry {
    /// Keeps `real`, which another store is to hold, and gives its id.
    pub fn register(&mut self, real: ResourceAny) -> u64 {
        let proxy_id = NEXT_PROXY_ID.fetch_add(1, Ordering::Relaxed);
        self.reals.insert(proxy_id, real);
        proxy_id
    }

    /// The handle `proxy_id` names, lent: it stays registered.
    pub fn get(&self, proxy_id: u64) -> Option<ResourceAny> {
        self.reals.get(&proxy_id).copied()
    }

    /// The handle `proxy_id` names, given back: it leaves the registry.
    pub fn take(&mut self, proxy_id: u64) -> Option<ResourceAny> {
        self.reals.remove(&proxy_id)
    }

    /// The holder let go of `proxy_id`: its handle waits to be dropped in
    /// the owner's store. Whether the id was registered.
    pub fn stage_drop(&mut self, proxy_id: u64) -> bool {
        match self.reals.remove(&proxy_id) {
            Some(real) => {
                self.pending_drops.push(real);
                true
            }
            None => false,
        }
    }

    /// The handles waiting to be dropped.
    pub fn take_pending_drops(&mut self) -> Vec<ResourceAny> {
        std::mem::take(&mut self.pending_drops)
    }
}

/// What a store's data gives the bridge: its resource table, where its
/// proxies are, and its registry when it owns resources another store
/// holds.
pub trait Bridged {
    fn table(&mut self) -> &mut ResourceTable;
    fn resource_registry(&mut self) -> Option<&mut ResourceRegistry>;
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use wasmtime::{Engine, Result, Store};

    /// A store's data: its table, and a registry when it owns resources.
    #[derive(Default)]
    pub(crate) struct Data {
        table: ResourceTable,
        registry: Option<ResourceRegistry>,
    }

    impl Bridged for Data {
        fn table(&mut self) -> &mut ResourceTable {
            &mut self.table
        }

        fn resource_registry(&mut self) -> Option<&mut ResourceRegistry> {
            self.registry.as_mut()
        }
    }

    /// What the owner's store has a handle of.
    struct Thing;

    /// An owner's store and a handle it registered, by its id.
    pub(crate) fn registered() -> Result<(Store<Data>, u64)> {
        let mut store = Store::new(
            &Engine::default(),
            Data {
                registry: Some(ResourceRegistry::default()),
                ..Default::default()
            },
        );
        let resource = store.data_mut().table.push(Thing)?;
        let real = resource.try_into_resource_any(&mut store)?;
        let proxy_id = registry(&mut store).register(real);
        Ok((store, proxy_id))
    }

    pub(crate) fn registry(store: &mut Store<Data>) -> &mut ResourceRegistry {
        store.data_mut().registry.as_mut().expect("the owner's")
    }

    /// A drop the holder stages leaves the registry for the pending drops,
    /// taken once; an id the registry does not have stages nothing.
    #[test]
    fn a_staged_drop_waits_for_the_owner_s_store() -> Result<()> {
        let (mut store, proxy_id) = registered()?;
        assert!(registry(&mut store).stage_drop(proxy_id));
        assert!(registry(&mut store).get(proxy_id).is_none());
        assert_eq!(registry(&mut store).take_pending_drops().len(), 1);
        assert!(registry(&mut store).take_pending_drops().is_empty());
        assert!(!registry(&mut store).stage_drop(proxy_id));
        Ok(())
    }
}
