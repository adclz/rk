// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The ST shape of a WIT value: the variables a parameter becomes, and what
//! a result comes back as. A scalar is one variable of its type; a record
//! or a tuple passed in is one variable per field, as the canonical ABI
//! flattens it; a record, a tuple, an option or a result that comes back is
//! one output per field, which rk lays out as the canonical ABI lays out
//! the value. Anything else is not generated yet, and says why.

use wit_parser::{Handle, Resolve, Type, TypeDefKind, TypeId, TypeOwner};

use crate::names;

/// The longest STRING a generated declaration takes, in bytes: a path as
/// long as Linux allows, and what `Std.Console` writes at once.
pub const STRING_LIMIT: u32 = 4096;

/// A variable of a generated POU.
#[derive(Debug, Clone)]
pub struct Var {
    pub name: String,
    pub ty: String,
    /// How many values the canonical ABI flattens it into.
    pub flat: usize,
}

/// What a function gives back.
pub enum Shape {
    /// Nothing.
    Nothing,
    /// One scalar, the FUNCTION's own value.
    Return(String),
    /// Outputs, one per field.
    Outputs(Vec<Var>),
}

/// The ST name of the type `id` defines, from anywhere in the generated
/// code: under `Wasi`, its package and its interface.
pub fn qualified(resolve: &Resolve, id: TypeId) -> String {
    let ty = &resolve.types[id];
    let name = names::upper(ty.name.as_deref().unwrap_or("anonymous"));
    match ty.owner {
        TypeOwner::Interface(interface) => {
            let interface = &resolve.interfaces[interface];
            let package = interface
                .package
                .map(|package| &resolve.packages[package].name);
            match (package, interface.name.as_deref()) {
                (Some(package), Some(interface)) => format!(
                    "{}.{}.{}.{name}",
                    names::namespace(&package.namespace),
                    names::namespace(&package.name),
                    names::namespace(interface)
                ),
                _ => name,
            }
        }
        _ => name,
    }
}

/// The type `ty` stands for, through the aliases a `use` makes, and the
/// alias's own definition otherwise: the alias the generated code names.
pub fn through_uses(resolve: &Resolve, ty: Type) -> Type {
    let mut ty = ty;
    while let Type::Id(id) = ty {
        let def = &resolve.types[id];
        match def.kind {
            TypeDefKind::Type(Type::Id(other)) if def.owner != resolve.types[other].owner => {
                ty = Type::Id(other);
            }
            _ => break,
        }
    }
    ty
}

/// The type `ty` names through every alias: a primitive, or a definition
/// that is no alias.
pub fn resolved(resolve: &Resolve, ty: Type) -> Type {
    let mut ty = ty;
    while let Type::Id(id) = ty {
        match resolve.types[id].kind {
            TypeDefKind::Type(inner) => ty = inner,
            _ => break,
        }
    }
    ty
}

/// The definition `ty` names, through every alias.
pub fn definition(resolve: &Resolve, ty: Type) -> Option<&TypeDefKind> {
    let mut ty = ty;
    loop {
        let Type::Id(id) = ty else { return None };
        match &resolve.types[id].kind {
            TypeDefKind::Type(inner) => ty = *inner,
            kind => return Some(kind),
        }
    }
}

/// What `ty` is, in words, when it has no scalar ST type.
pub fn what(resolve: &Resolve, ty: Type) -> &'static str {
    match resolved(resolve, ty) {
        Type::Char => "a char",
        Type::String => "a string",
        Type::ErrorContext => "an error context",
        Type::Id(_) => match definition(resolve, ty) {
            Some(TypeDefKind::Record(_)) => "a record",
            Some(TypeDefKind::Tuple(_)) => "a tuple",
            Some(TypeDefKind::Option(_)) => "an option",
            Some(TypeDefKind::Result(_)) => "a result",
            Some(TypeDefKind::Variant(_)) => "a variant",
            Some(TypeDefKind::Flags(_)) => "flags",
            Some(TypeDefKind::List(_)) => "a list",
            Some(TypeDefKind::Map(..)) => "a map",
            Some(TypeDefKind::FixedLengthList(..)) => "a fixed-length list",
            _ => "a type rk does not know",
        },
        _ => "a scalar",
    }
}

/// The ST type of a scalar `ty`: a number or a BOOL, an enum, an alias of
/// one, or a handle, which is a UDINT. `None` for anything else.
pub fn scalar(resolve: &Resolve, ty: Type, here: Option<TypeOwner>) -> Option<String> {
    let ty = through_uses(resolve, ty);
    let elementary = match ty {
        Type::Bool => "BOOL",
        Type::U8 => "USINT",
        Type::U16 => "UINT",
        Type::U32 => "UDINT",
        Type::U64 => "ULINT",
        Type::S8 => "SINT",
        Type::S16 => "INT",
        Type::S32 => "DINT",
        Type::S64 => "LINT",
        Type::F32 => "REAL",
        Type::F64 => "LREAL",
        Type::Char | Type::String | Type::ErrorContext => return None,
        Type::Id(id) => {
            let def = &resolve.types[id];
            let named = |resolve: &Resolve| match here == Some(def.owner) {
                true => names::upper(def.name.as_deref().unwrap_or("anonymous")),
                false => qualified(resolve, id),
            };
            return match &def.kind {
                TypeDefKind::Enum(_) => Some(named(resolve)),
                TypeDefKind::Type(inner) if def.name.is_some() => {
                    scalar(resolve, *inner, here).map(|_| named(resolve))
                }
                TypeDefKind::Type(inner) => scalar(resolve, *inner, here),
                TypeDefKind::Handle(Handle::Own(_) | Handle::Borrow(_))
                | TypeDefKind::Resource
                | TypeDefKind::Future(_)
                | TypeDefKind::Stream(_) => Some("UDINT".to_string()),
                _ => None,
            };
        }
    };
    Some(elementary.to_string())
}

/// The STRING a parameter takes.
fn string() -> String {
    format!("STRING[{STRING_LIMIT}]")
}

/// The variables a parameter `name` of type `ty` becomes: one, or one per
/// field of a record or a tuple, `when_seconds`. `Err` says what it is
/// that cannot be passed.
pub fn input(
    resolve: &Resolve,
    name: &str,
    ty: Type,
    here: Option<TypeOwner>,
) -> Result<Vec<Var>, &'static str> {
    let one = |name: String, ty: Type| -> Result<Var, &'static str> {
        match resolved(resolve, ty) {
            Type::String => Ok(Var {
                name,
                ty: string(),
                flat: 2,
            }),
            _ => scalar(resolve, ty, here)
                .map(|st| Var {
                    name,
                    ty: st,
                    flat: 1,
                })
                .ok_or_else(|| what(resolve, ty)),
        }
    };
    if let Ok(var) = one(names::lower(name), ty) {
        return Ok(vec![var]);
    }
    let fields: Vec<(String, Type)> = match definition(resolve, ty) {
        Some(TypeDefKind::Record(record)) => record
            .fields
            .iter()
            .map(|field| (format!("{name}-{}", field.name), field.ty))
            .collect(),
        Some(TypeDefKind::Tuple(tuple)) => tuple
            .types
            .iter()
            .enumerate()
            .map(|(at, ty)| (format!("{name}-{at}"), *ty))
            .collect(),
        _ => return Err(what(resolve, ty)),
    };
    fields
        .into_iter()
        .map(|(field, ty)| one(names::lower(&field), ty))
        .collect()
}

/// What a function returns, `ty`, as ST gives it back: a FUNCTION's own
/// value for a scalar, or outputs. A block has no value of its own:
/// `in_block` gives a scalar back as the output `value`. `Err` says what it
/// is that cannot come back.
pub fn result(
    resolve: &Resolve,
    ty: Option<Type>,
    in_block: bool,
    here: Option<TypeOwner>,
) -> Result<Shape, &'static str> {
    let Some(ty) = ty else {
        return Ok(Shape::Nothing);
    };
    let out = |name: &str, ty: Type| -> Result<Var, &'static str> {
        scalar(resolve, ty, here)
            .map(|st| Var {
                name: names::lower(name),
                ty: st,
                flat: 1,
            })
            .ok_or_else(|| what(resolve, ty))
    };
    if let Some(st) = scalar(resolve, ty, here) {
        return Ok(match in_block {
            true => Shape::Outputs(vec![Var {
                name: "value".to_string(),
                ty: st,
                flat: 1,
            }]),
            false => Shape::Return(st),
        });
    }
    let flag = |name: &str| Var {
        name: name.to_string(),
        ty: "BOOL".to_string(),
        flat: 1,
    };
    let outputs = match definition(resolve, ty) {
        Some(TypeDefKind::Record(record)) => record
            .fields
            .iter()
            .map(|field| out(&field.name, field.ty))
            .collect::<Result<Vec<_>, _>>()?,
        Some(TypeDefKind::Tuple(tuple)) => tuple
            .types
            .iter()
            .enumerate()
            .map(|(at, ty)| out(&format!("item{at}"), *ty))
            .collect::<Result<Vec<_>, _>>()?,
        Some(TypeDefKind::Option(inner)) => vec![flag("is_some"), out("value", *inner)?],
        // An ok value and an error share their place in memory, which two
        // outputs cannot.
        Some(TypeDefKind::Result(result)) => match (result.ok, result.err) {
            (None, None) => vec![flag("failed")],
            (Some(ok), None) => vec![flag("failed"), out("value", ok)?],
            (None, Some(err)) => vec![flag("failed"), out("error", err)?],
            (Some(_), Some(_)) => return Err("a result with a value and an error"),
        },
        _ => return Err(what(resolve, ty)),
    };
    Ok(Shape::Outputs(outputs))
}
