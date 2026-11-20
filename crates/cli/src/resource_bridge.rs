// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The handles of a resource a component owns, as another store holds them,
//! the way runtimes that link components across stores on wasmtime keep
//! them (wasmCloud's `wash-runtime`, `engine/store/resource_bridge.rs` and
//! `relocate.rs`). The store that owns the resource keeps its handle in a
//! [`ResourceRegistry`], under an id; any other store holds a
//! [`ProxyResource`] with that id, one host resource type for every
//! resource it imports. A store never touches another's handles: a value
//! crossing is [`extract`]ed from one store into a [`Relocated`] value,
//! whose handles are ids, then [`inject`]ed into the other. An owned handle
//! passed back to its owner leaves the registry, a borrowed one stays, and
//! one its holder drops is staged in the registry and dropped in the
//! owner's store once that store is free ([`ResourceRegistry::stage_drop`]).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use wasmtime::component::types::Type;
use wasmtime::component::{Resource, ResourceAny, ResourceTable, ResourceType, Val};
use wasmtime::{AsContextMut, Result, StoreContextMut, bail, format_err};

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

/// A value on its way from one store to another: as it was, with each
/// resource's handle in it an id.
pub enum Relocated {
    Plain(Val),
    Resource { proxy_id: u64, owned: bool },
    List(Vec<Relocated>),
    FixedLengthList(Vec<Relocated>),
    Map(Vec<(Relocated, Relocated)>),
    Record(Vec<(String, Relocated)>),
    Tuple(Vec<Relocated>),
    Variant(String, Option<Box<Relocated>>),
    Option(Option<Box<Relocated>>),
    Result(std::result::Result<Option<Box<Relocated>>, Option<Box<Relocated>>>),
}

/// `value`, of type `ty`, taken out of `store`: each handle in it, at any
/// depth, an id ([`extract_resource`]).
pub fn extract<D: Bridged + 'static>(
    mut store: impl AsContextMut<Data = D>,
    value: &Val,
    ty: &Type,
) -> Result<Relocated> {
    let mut store = store.as_context_mut();
    let mut boxed = |value: &Val, ty: Option<Type>| -> Result<Relocated> {
        match ty {
            Some(ty) => extract(&mut store, value, &ty),
            None => Ok(Relocated::Plain(value.clone())),
        }
    };
    Ok(match (value, ty) {
        (Val::Resource(any), Type::Own(_)) => extract_resource(&mut store, *any, true)?,
        (Val::Resource(any), Type::Borrow(_)) => extract_resource(&mut store, *any, false)?,
        (Val::List(values), Type::List(list)) => {
            Relocated::List(all(&mut store, values, &list.ty())?)
        }
        (Val::FixedLengthList(values), Type::FixedLengthList(list)) => {
            Relocated::FixedLengthList(all(&mut store, values, &list.ty())?)
        }
        (Val::Map(entries), Type::Map(map)) => Relocated::Map(
            entries
                .iter()
                .map(|(key, value)| {
                    Ok((
                        extract(&mut store, key, &map.key())?,
                        extract(&mut store, value, &map.value())?,
                    ))
                })
                .collect::<Result<_>>()?,
        ),
        (Val::Record(fields), Type::Record(record)) => Relocated::Record(
            fields
                .iter()
                .zip(record.fields())
                .map(|((name, value), field)| {
                    Ok((name.clone(), extract(&mut store, value, &field.ty)?))
                })
                .collect::<Result<_>>()?,
        ),
        (Val::Tuple(values), Type::Tuple(tuple)) => Relocated::Tuple(
            values
                .iter()
                .zip(tuple.types())
                .map(|(value, ty)| extract(&mut store, value, &ty))
                .collect::<Result<_>>()?,
        ),
        (Val::Variant(case, Some(payload)), Type::Variant(variant)) => {
            let ty = variant.cases().find(|c| c.name == case).and_then(|c| c.ty);
            Relocated::Variant(case.clone(), Some(Box::new(boxed(payload, ty)?)))
        }
        (Val::Option(Some(payload)), Type::Option(option)) => {
            Relocated::Option(Some(Box::new(boxed(payload, Some(option.ty()))?)))
        }
        (Val::Result(Ok(Some(payload))), Type::Result(result)) => {
            Relocated::Result(Ok(Some(Box::new(boxed(payload, result.ok())?))))
        }
        (Val::Result(Err(Some(payload))), Type::Result(result)) => {
            Relocated::Result(Err(Some(Box::new(boxed(payload, result.err())?))))
        }
        _ => Relocated::Plain(value.clone()),
    })
}

/// Each of `values`, of type `ty`, taken out of `store` ([`extract`]).
fn all<D: Bridged + 'static>(
    store: &mut StoreContextMut<'_, D>,
    values: &[Val],
    ty: &Type,
) -> Result<Vec<Relocated>> {
    values
        .iter()
        .map(|value| extract(&mut *store, value, ty))
        .collect()
}

/// A handle taken out of `store`: a proxy of the store's is the id it
/// holds, given up when the handle is `owned`; a handle the store owns is
/// registered under a new id.
fn extract_resource<D: Bridged + 'static>(
    store: &mut StoreContextMut<'_, D>,
    any: ResourceAny,
    owned: bool,
) -> Result<Relocated> {
    let proxy_id = if any.ty() == proxy_resource_type() {
        let resource: Resource<ProxyResource> = any.try_into_resource(&mut *store)?;
        let table = store.data_mut().table();
        match owned {
            true => table.delete(resource)?.proxy_id,
            false => table.get(&resource)?.proxy_id,
        }
    } else if let Some(registry) = store.data_mut().resource_registry() {
        registry.register(any)
    } else {
        bail!("cross-store bridge: a handle of a resource no other store owns");
    };
    Ok(Relocated::Resource { proxy_id, owned })
}

/// `relocated` put into `store`: each id its handle there, the owner's
/// own handle in the store that registered it, a proxy in any other.
pub fn inject<D: Bridged + 'static>(
    mut store: impl AsContextMut<Data = D>,
    relocated: Relocated,
) -> Result<Val> {
    let mut store = store.as_context_mut();
    let mut boxed = |payload: Option<Box<Relocated>>| -> Result<Option<Box<Val>>> {
        payload
            .map(|payload| inject(&mut store, *payload).map(Box::new))
            .transpose()
    };
    Ok(match relocated {
        Relocated::Plain(value) => value,
        Relocated::Resource { proxy_id, owned } => inject_resource(&mut store, proxy_id, owned)?,
        Relocated::List(values) => Val::List(each(&mut store, values)?),
        Relocated::FixedLengthList(values) => Val::FixedLengthList(each(&mut store, values)?),
        Relocated::Tuple(values) => Val::Tuple(each(&mut store, values)?),
        Relocated::Map(entries) => Val::Map(
            entries
                .into_iter()
                .map(|(key, value)| Ok((inject(&mut store, key)?, inject(&mut store, value)?)))
                .collect::<Result<_>>()?,
        ),
        Relocated::Record(fields) => Val::Record(
            fields
                .into_iter()
                .map(|(name, value)| Ok((name, inject(&mut store, value)?)))
                .collect::<Result<_>>()?,
        ),
        Relocated::Variant(case, payload) => Val::Variant(case, boxed(payload)?),
        Relocated::Option(payload) => Val::Option(boxed(payload)?),
        Relocated::Result(Ok(payload)) => Val::Result(Ok(boxed(payload)?)),
        Relocated::Result(Err(payload)) => Val::Result(Err(boxed(payload)?)),
    })
}

/// Each of `values` put into `store` ([`inject`]).
fn each<D: Bridged + 'static>(
    store: &mut StoreContextMut<'_, D>,
    values: Vec<Relocated>,
) -> Result<Vec<Val>> {
    values
        .into_iter()
        .map(|value| inject(&mut *store, value))
        .collect()
}

/// The handle `proxy_id` names, put into `store`: the owner's own, taken
/// from its registry when `owned`, lent when borrowed; in any other store a
/// new proxy.
fn inject_resource<D: Bridged + 'static>(
    store: &mut StoreContextMut<'_, D>,
    proxy_id: u64,
    owned: bool,
) -> Result<Val> {
    if let Some(registry) = store.data_mut().resource_registry() {
        let real = match owned {
            true => registry.take(proxy_id),
            false => registry.get(proxy_id),
        };
        return real
            .map(Val::Resource)
            .ok_or_else(|| format_err!("cross-store bridge: unknown proxied resource {proxy_id}"));
    }
    let resource = store.data_mut().table().push(ProxyResource { proxy_id })?;
    Ok(Val::Resource(resource.try_into_resource_any(&mut *store)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::{Engine, Store};

    /// A store's data: its table, and a registry when it owns resources.
    #[derive(Default)]
    struct Data {
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
    fn registered() -> Result<(Store<Data>, u64)> {
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

    fn registry(store: &mut Store<Data>) -> &mut ResourceRegistry {
        store.data_mut().registry.as_mut().expect("the owner's")
    }

    /// A borrowed handle injected into its owner's store is lent: it stays
    /// registered. An owned one is given back: it leaves the registry, and
    /// its id names nothing after.
    #[test]
    fn an_owned_handle_leaves_the_registry_a_borrowed_one_stays() -> Result<()> {
        let (mut store, proxy_id) = registered()?;
        let lent = inject(
            &mut store,
            Relocated::Resource {
                proxy_id,
                owned: false,
            },
        )?;
        assert!(matches!(lent, Val::Resource(_)));
        assert!(registry(&mut store).get(proxy_id).is_some());
        let given = inject(
            &mut store,
            Relocated::Resource {
                proxy_id,
                owned: true,
            },
        )?;
        assert!(matches!(given, Val::Resource(_)));
        assert!(registry(&mut store).get(proxy_id).is_none());
        let again = inject(
            &mut store,
            Relocated::Resource {
                proxy_id,
                owned: true,
            },
        );
        assert!(again.is_err(), "an id the registry no longer has");
        Ok(())
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
