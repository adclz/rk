// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The ST type of a WIT type that has a name and holds values, as
//! wit-bindgen gives each one a type of the language: a record is a STRUCT
//! of its fields; a variant whose cases hold values is a STRUCT of its
//! case, `kind`, an enum named after it (`ERROR_CODE_KIND`), and of what
//! each case holds, under the case's name, each in a place of its own; a
//! tuple WIT names is a STRUCT of its items, `item0`, `item1`. A function
//! takes and gives such a value whole.
//!
//! What has no name of its own, an option, a result, a tuple, is laid out
//! in the members of the type that holds it, as a function's outputs are:
//! `timestamp_is_some` then `timestamp`. A case whose value ST has no type
//! for, a list, keeps its name alone; a record that holds one has no type.

use wit_parser::{Docs, Resolve, Type, TypeDefKind, TypeId, TypeOwner};

use crate::lift::{held_by_option, join};
use crate::names;
use crate::shapes;

/// How many bytes the STRING of a member holds.
pub const MEMBER_STRING_CAPACITY: u32 = 256;

/// The STRING a member holds a string in.
pub fn member_string() -> String {
    format!("STRING[{MEMBER_STRING_CAPACITY}]")
}

/// The type `ty` names when ST has a STRUCT for it, through the aliases a
/// `use` makes: a record, a variant whose cases hold values, a tuple WIT
/// names, or an alias of one.
pub fn structured(resolve: &Resolve, ty: Type) -> Option<TypeId> {
    let Type::Id(id) = shapes::through_uses(resolve, ty) else {
        return None;
    };
    let def = &resolve.types[id];
    def.name.as_ref()?;
    match &def.kind {
        TypeDefKind::Record(_) | TypeDefKind::Tuple(_) => Some(id),
        TypeDefKind::Variant(variant) if shapes::carries(variant) => Some(id),
        TypeDefKind::Type(inner) => structured(resolve, *inner).map(|_| id),
        _ => None,
    }
}

/// The definition the STRUCT of `id` lays out, through its aliases.
pub fn target(resolve: &Resolve, id: TypeId) -> TypeId {
    match resolve.types[id].kind {
        TypeDefKind::Type(Type::Id(inner)) => target(resolve, inner),
        _ => id,
    }
}

/// The enum of the cases of the variant `id`, as code in `here` names it.
pub fn kind_name(resolve: &Resolve, id: TypeId, here: Option<TypeOwner>) -> String {
    format!(
        "{}_KIND",
        shapes::type_name(resolve, target(resolve, id), here)
    )
}

/// A member of a STRUCT: its name, its ST type, and what WIT says of it.
pub struct Member {
    pub name: String,
    pub ty: String,
    pub docs: Option<String>,
}

/// The members of the STRUCT of `id`, in order, as code in `here` names
/// their types. `Err` says why it has no STRUCT: what in it ST has no type
/// for, or two members ST would take for one.
pub fn members(
    resolve: &Resolve,
    id: TypeId,
    here: Option<TypeOwner>,
) -> Result<Vec<Member>, String> {
    let mut out = Vec::new();
    let id = target(resolve, id);
    match &resolve.types[id].kind {
        TypeDefKind::Record(record) => {
            for field in &record.fields {
                let first = out.len();
                member(resolve, &field.name, field.ty, here, &mut out)?;
                if let Some(member) = out.get_mut(first) {
                    member.docs = docs(&field.docs);
                }
            }
        }
        TypeDefKind::Tuple(tuple) => {
            for (at, ty) in tuple.types.iter().enumerate() {
                member(resolve, &format!("item{at}"), *ty, here, &mut out)?;
            }
        }
        TypeDefKind::Variant(variant) => {
            out.push(Member {
                name: "kind".to_string(),
                ty: kind_name(resolve, id, here),
                docs: None,
            });
            // A case whose value ST has no type for keeps its name alone.
            for case in &variant.cases {
                let Some(ty) = case.ty else { continue };
                let first = out.len();
                if member(resolve, &case.name, ty, here, &mut out).is_err() {
                    out.truncate(first);
                    continue;
                }
                if let Some(member) = out.get_mut(first) {
                    member.docs = docs(&case.docs);
                }
            }
        }
        _ => return Err(holds(shapes::what(resolve, Type::Id(id)))),
    }
    for (at, member) in out.iter().enumerate() {
        if out[..at]
            .iter()
            .any(|earlier| earlier.name.eq_ignore_ascii_case(&member.name))
        {
            return Err(format!(
                "two of its members would be `{}`, which ST takes for one",
                member.name
            ));
        }
    }
    Ok(out)
}

/// Whether the type `id` has a STRUCT, which a function can take or give:
/// `Err` says what a function that does holds, and [`members`] why, where
/// the type is.
pub fn declared(
    resolve: &Resolve,
    id: TypeId,
    here: Option<TypeOwner>,
) -> Result<(), &'static str> {
    members(resolve, id, here)
        .map(|_| ())
        .map_err(|_| "a value whose STRUCT is left out")
}

fn docs(docs: &Docs) -> Option<String> {
    docs.contents.clone()
}

/// Why a type that holds `what` has no STRUCT.
fn holds(what: &str) -> String {
    format!("it holds {what}, which ST has no type for yet")
}

/// The members a value of `ty` named `name` is in a STRUCT: one for a
/// scalar, a string or a value ST has a STRUCT for; a flag and what it
/// decides for an option or a result; one per item of a tuple no name
/// holds, named as a function's outputs are. `Err` says why the type that
/// holds it has no STRUCT.
fn member(
    resolve: &Resolve,
    name: &str,
    ty: Type,
    here: Option<TypeOwner>,
    out: &mut Vec<Member>,
) -> Result<(), String> {
    let mut push = |name: &str, ty: String| {
        out.push(Member {
            name: names::lower(name),
            ty,
            docs: None,
        })
    };
    if let Type::String = shapes::resolved(resolve, ty) {
        push(name, member_string());
        return Ok(());
    }
    if let Some(st) = shapes::scalar(resolve, ty, here) {
        push(name, st);
        return Ok(());
    }
    if let Some(id) = structured(resolve, ty) {
        if declared(resolve, id, here).is_err() {
            return Err(format!(
                "it holds `{}`, which is left out",
                wit_name(resolve, id)
            ));
        }
        push(name, shapes::type_name(resolve, id, here));
        return Ok(());
    }
    match shapes::definition(resolve, ty) {
        Some(TypeDefKind::Option(some)) => {
            push(&join(name, "is-some"), "BOOL".to_string());
            member(
                resolve,
                &held_by_option(resolve, name, *some),
                *some,
                here,
                out,
            )
        }
        Some(TypeDefKind::Result(result)) => {
            push(&join(name, "failed"), "BOOL".to_string());
            if let Some(ok) = result.ok {
                member(resolve, &join(name, "value"), ok, here, out)?;
            }
            if let Some(err) = result.err {
                member(resolve, &join(name, "error"), err, here, out)?;
            }
            Ok(())
        }
        Some(TypeDefKind::Tuple(tuple)) => {
            for (at, item) in tuple.types.iter().enumerate() {
                member(resolve, &join(name, &at.to_string()), *item, here, out)?;
            }
            Ok(())
        }
        _ => Err(holds(shapes::what(resolve, ty))),
    }
}

/// The name WIT gives the definition the STRUCT of `id` lays out.
pub fn wit_name(resolve: &Resolve, id: TypeId) -> &str {
    resolve.types[target(resolve, id)]
        .name
        .as_deref()
        .expect("a type ST has a STRUCT for has a name")
}
