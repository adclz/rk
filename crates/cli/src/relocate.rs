// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A value on its way from one store to another, the way runtimes that link
//! components across stores on wasmtime carry it (wasmCloud's
//! `wash-runtime`, `engine/store/relocate.rs`): [`extract`]ed from the store
//! it is in into a [`Relocated`] value, whose resources' handles are ids of
//! the [`resource_bridge`](crate::resource_bridge) and whose streams and
//! futures are piped into channels ([`crate::stream_pump`]); then
//! [`inject`]ed into the other store, where each id is a handle and each
//! channel feeds a stream or a future of its own.
//!
//! wasmtime's host API reads a stream's items and a future's value as a Rust
//! type, so the types that cross are listed once, as wasmCloud lists them:
//! the numbers, `bool`, `char` and `string`. The build refuses the others.

use wasmtime::component::types::Type;
use wasmtime::component::{
    FutureAny, FutureReader, Lift, Lower, Resource, ResourceAny, StreamAny, StreamReader, Val,
};
use wasmtime::{AsContextMut, Result, StoreContextMut, bail, format_err};

use crate::resource_bridge::{Bridged, ProxyResource, proxy_resource_type};
use crate::stream_pump;

/// A value on its way from one store to another: as it was, with each
/// resource's handle in it an id, and each stream or future piped into a
/// channel. `D` is what each store holds, the same in both, as wasmCloud's
/// stores all hold its `SharedCtx`.
pub enum Relocated<D: 'static> {
    Plain(Val),
    Resource {
        proxy_id: u64,
        owned: bool,
    },
    /// A stream, carried as what builds it in the other store from the
    /// channel its items go into.
    Stream(ValFactory<D>),
    /// A future, carried the same way: what builds it from the channel its
    /// value comes through.
    Future(ValFactory<D>),
    List(Vec<Relocated<D>>),
    FixedLengthList(Vec<Relocated<D>>),
    Map(Vec<(Relocated<D>, Relocated<D>)>),
    Record(Vec<(String, Relocated<D>)>),
    Tuple(Vec<Relocated<D>>),
    Variant(String, Payload<D>),
    Option(Payload<D>),
    Result(std::result::Result<Payload<D>, Payload<D>>),
}

/// What a case of a variant, an option or a result holds, when it holds
/// something.
pub type Payload<D> = Option<Box<Relocated<D>>>;

/// What builds a stream or a future in the store a value goes to, fed by
/// the channel its other end was piped into.
pub type ValFactory<D> = Box<dyn FnOnce(StoreContextMut<'_, D>) -> Result<Val> + Send>;

/// `value`, of type `ty`, taken out of `store`: each handle in it, at any
/// depth, an id ([`extract_resource`]), and each stream or future piped out
/// of the store ([`stream_pump`]).
pub fn extract<D: Bridged + 'static>(
    mut store: impl AsContextMut<Data = D>,
    value: &Val,
    ty: &Type,
) -> Result<Relocated<D>> {
    let mut store = store.as_context_mut();
    let mut boxed = |value: &Val, ty: Option<Type>| -> Result<Relocated<D>> {
        match ty {
            Some(ty) => extract(&mut store, value, &ty),
            None => Ok(Relocated::Plain(value.clone())),
        }
    };
    Ok(match (value, ty) {
        (Val::Resource(any), Type::Own(_)) => extract_resource(&mut store, *any, true)?,
        (Val::Resource(any), Type::Borrow(_)) => extract_resource(&mut store, *any, false)?,
        (Val::Stream(any), Type::Stream(stream)) => {
            let Some(item) = stream.ty() else {
                bail!("cross-store bridge: a stream with no type of item");
            };
            Relocated::Stream(stream_factory(&mut store, any.clone(), &item)?)
        }
        (Val::Future(any), Type::Future(future)) => Relocated::Future(match future.ty() {
            Some(value) => future_factory(&mut store, any.clone(), &value)?,
            None => bridge_future::<D, ()>(&mut store, any.clone())?,
        }),
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
) -> Result<Vec<Relocated<D>>> {
    values
        .iter()
        .map(|value| extract(&mut *store, value, ty))
        .collect()
}

/// A stream of `T` taken out of `src`: its reader piped into a channel of
/// chunks ([`stream_pump::channel`]), and what builds the stream that the
/// channel feeds in the other store.
fn bridge_stream<D: 'static, T>(
    src: &mut StoreContextMut<'_, D>,
    any: StreamAny,
) -> Result<ValFactory<D>>
where
    T: Lift + Lower + Send + Sync + 'static,
{
    let reader = any.try_into_stream_reader::<T>()?;
    let (consumer, producer) = stream_pump::channel::<T>(stream_pump::DEFAULT_CAPACITY);
    reader.pipe(&mut *src, consumer)?;
    Ok(Box::new(move |mut dst| {
        let reader = StreamReader::new(&mut dst, producer)?;
        Ok(Val::Stream(StreamAny::try_from_stream_reader(
            &mut dst, reader,
        )?))
    }))
}

/// A future of `T` taken out of `src`: its reader piped into a oneshot
/// channel ([`stream_pump::future_channel`]), and what builds the future
/// that awaits the channel in the other store.
fn bridge_future<D: 'static, T>(
    src: &mut StoreContextMut<'_, D>,
    any: FutureAny,
) -> Result<ValFactory<D>>
where
    T: Lift + Lower + Send + Sync + 'static,
{
    let reader = any.try_into_future_reader::<T>()?;
    let (sink, rx) = stream_pump::future_channel::<T>();
    reader.pipe(&mut *src, sink)?;
    Ok(Box::new(move |mut dst| {
        let reader = FutureReader::new(&mut dst, async move {
            rx.await.map_err(|_| {
                format_err!("cross-store bridge: the future's writer left with no value")
            })
        })?;
        Ok(Val::Future(FutureAny::try_from_future_reader(
            &mut dst, reader,
        )?))
    }))
}

/// The types of a stream's items and of a future's value that cross between
/// stores, each with the Rust type wasmtime's host API reads it as, listed
/// once for both, as wasmCloud's `bridgeable_elements!` lists them.
macro_rules! bridgeable_elements {
    ($($variant:ident => $rust:ty),* $(,)?) => {
        /// A stream of `item` taken out of `src` ([`bridge_stream`]).
        fn stream_factory<D: 'static>(
            src: &mut StoreContextMut<'_, D>,
            any: StreamAny,
            item: &Type,
        ) -> Result<ValFactory<D>> {
            match item {
                $(Type::$variant => bridge_stream::<D, $rust>(src, any),)*
                other => bail!("cross-store bridge: a stream of {other:?} stays in its store"),
            }
        }

        /// A future of `value` taken out of `src` ([`bridge_future`]).
        fn future_factory<D: 'static>(
            src: &mut StoreContextMut<'_, D>,
            any: FutureAny,
            value: &Type,
        ) -> Result<ValFactory<D>> {
            match value {
                $(Type::$variant => bridge_future::<D, $rust>(src, any),)*
                other => bail!("cross-store bridge: a future of {other:?} stays in its store"),
            }
        }
    };
}

bridgeable_elements!(
    Bool => bool, S8 => i8, U8 => u8, S16 => i16, U16 => u16,
    S32 => i32, U32 => u32, S64 => i64, U64 => u64,
    Float32 => f32, Float64 => f64, Char => char, String => String,
);

/// A handle taken out of `store`: a proxy of the store's is the id it
/// holds, given up when the handle is `owned`; a handle the store owns is
/// registered under a new id.
fn extract_resource<D: Bridged + 'static>(
    store: &mut StoreContextMut<'_, D>,
    any: ResourceAny,
    owned: bool,
) -> Result<Relocated<D>> {
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
/// channel a new stream or future of the store's.
pub fn inject<D: Bridged + 'static>(
    mut store: impl AsContextMut<Data = D>,
    relocated: Relocated<D>,
) -> Result<Val> {
    let mut store = store.as_context_mut();
    let mut boxed = |payload: Payload<D>| -> Result<Option<Box<Val>>> {
        payload
            .map(|payload| inject(&mut store, *payload).map(Box::new))
            .transpose()
    };
    Ok(match relocated {
        Relocated::Plain(value) => value,
        Relocated::Resource { proxy_id, owned } => inject_resource(&mut store, proxy_id, owned)?,
        Relocated::Stream(build) | Relocated::Future(build) => build(store.as_context_mut())?,
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
    values: Vec<Relocated<D>>,
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
