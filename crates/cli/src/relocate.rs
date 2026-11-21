// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A value on its way from one store to another, the way runtimes that link
//! components across stores on wasmtime carry it (wasmCloud's
//! `wash-runtime`, `engine/store/relocate.rs`): [`extract`]ed from the store
//! it is in into a [`Relocated`] value, whose resources' handles are ids of
//! the [`resource_bridge`](crate::resource_bridge) and whose streams and
//! futures are the ends of pipes ([`crate::stream_pump`]); then [`inject`]ed
//! into the other store, where each id is a handle and each end a stream or
//! a future of its own.

use wasmtime::component::types::Type;
use wasmtime::component::{Resource, ResourceAny, Val};
use wasmtime::{AsContextMut, Result, StoreContextMut, bail, format_err};

use crate::resource_bridge::{Bridged, ProxyResource, proxy_resource_type};
use crate::stream_pump::{self, FutureEnd, StreamEnd};

/// A value on its way from one store to another: as it was, with each
/// resource's handle in it an id, and each stream or future the end of a
/// pipe.
pub enum Relocated {
    Plain(Val),
    Resource { proxy_id: u64, owned: bool },
    Stream(StreamEnd),
    Future(FutureEnd),
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
/// depth, an id ([`extract_resource`]), and each stream or future piped out
/// of the store ([`stream_pump`]).
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
        (Val::Stream(any), Type::Stream(stream)) => {
            Relocated::Stream(stream_pump::take_stream(&mut store, any.clone(), stream)?)
        }
        (Val::Future(any), Type::Future(future)) => {
            Relocated::Future(stream_pump::take_future(&mut store, any.clone(), future)?)
        }
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
/// own handle in the store that registered it, a proxy in any other; each
/// pipe's end a new stream or future of the store's.
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
        Relocated::Stream(end) => Val::Stream(stream_pump::give_stream(&mut store, end)?),
        Relocated::Future(end) => Val::Future(stream_pump::give_future(&mut store, end)?),
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
    use crate::resource_bridge::tests::{registered, registry};

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
}
