// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Symbol naming: namespace-qualified POU names and the `$`-suffix
//! mangling.
//!
//! Every function in a module has one symbol, and two functions never share
//! one: `lower_module` refuses a second (`LowerTypeError::DuplicateSymbol`)
//! rather than let a call reach the wrong body. The spellings:
//!
//! - a FUNCTION: its qualified name (`NsA.Scale`); when the name is
//!   overloaded, one `$<type>` per `VAR_INPUT`/`VAR_IN_OUT`, then `$:<type>`
//!   for the return of a RETURN-directed set (`F$INT$:REAL`, which
//!   `F(INT, REAL)` cannot spell);
//! - its specializations: `$@<implementer>` per interface parameter, by
//!   parameter name, and `$<count>` per variadic arity;
//! - a METHOD: `<owner>#<name>`, specialized the same way. The owner is the
//!   instance type the body is emitted for: an inherited method is emitted
//!   on each inheritor, so `THIS` inside it is that inheritor. A base's
//!   method it overrides, reached through `SUPER.m()`, is
//!   `<owner>#<base>.<name>`;
//! - bodies: `<owner>$__body__`, a base's reached through `SUPER()`
//!   `<owner>$<base>.__body__`, and `<instance>$__scan__`.
//!
//! A `<type>` fragment is the IEC name of an elementary type, the qualified
//! name of a named one (`Motion.Axis`, never folded to `Motion_Axis`, which
//! is another type's name), or for an unnamed one its structure in brackets
//! no identifier can contain (`ARRAY[0..4](INT)`, `REF_TO(Int5)`). Everything
//! before the first `$` or `#` is the POU, which rk-runtime's debugger reads
//! back.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::function::Function;
use hir::hir_ty::infer::Infer;
use hir::hir_ty::infer::const_eval::{array_dimensions, enum_ordinals, subrange_bounds};
use hir::hir_ty::ty::Type;

/// `base$Part1$Part2…` from sorted concrete part names, or `base` when
/// there are none.
pub fn mangle_generic_name(db: &dyn WorkspaceDataBase, base: Ident, parts: &[&str]) -> Ident {
    if parts.is_empty() {
        return base;
    }
    let mut s = base.text(db).to_string();
    for p in parts {
        s.push('$');
        s.push_str(p);
    }
    Ident::new(db, CompactString::from(s))
}

/// The MIR symbol of a FUNCTION: its qualified name, with the signature
/// appended as discriminant when it is overloaded (`SHL$BYTE`). Definition
/// and call compute it the same way.
pub fn mir_function_symbol<'db>(db: &'db dyn WorkspaceDataBase, f: Function<'db>) -> Ident {
    let base = qualified_pou_ident(db, Type::Function(f));
    // What discriminates the symbol is resolution's decision.
    match hir::hir_ty::resolver::name::overload_discriminant(db, f) {
        Some(discriminant) => {
            let mut parts: Vec<String> = discriminant
                .params
                .iter()
                .map(|t| type_fragment(db, *t))
                .collect();
            if let Some(ret) = discriminant.ret {
                parts.push(format!(":{}", type_fragment(db, ret)));
            }
            let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
            mangle_generic_name(db, base, &refs)
        }
        None => base,
    }
}

/// The symbol of `method` emitted for the instance type `owner`: `Owner#m`
/// when it is the method `owner` answers to by that name (its own, or the one
/// it inherits), `Owner#Base.m` for a base's method it overrides, which only
/// `SUPER.m()` reaches.
pub fn method_copy_symbol<'db>(
    db: &'db dyn WorkspaceDataBase,
    owner: hir::hir_def::pous::pou::Pou<'db>,
    method: hir::hir_def::pous::class::MethodDecl<'db>,
) -> Ident {
    let owner_q = qualified_pou_ident(db, Type::new_pou(db, owner));
    let name = method.name_with_case(db);
    let answers =
        hir::hir_ty::oop::class_members(db, owner).implementation(&method.name(db)) == Some(method);
    let text = match declaring_owner(db, method) {
        Some(declared) if !answers => format!(
            "{}#{}.{}",
            owner_q.text(db),
            qualified_pou_ident(db, Type::new_pou(db, declared)).text(db),
            name.text(db)
        ),
        _ => format!("{}#{}", owner_q.text(db), name.text(db)),
    };
    Ident::new(db, CompactString::from(text))
}

/// The symbol of `body_of`'s body emitted for the instance type `owner`:
/// `Owner$__body__` for its own, `Owner$Base.__body__` for a base's that
/// `SUPER()` reaches.
pub fn body_symbol<'db>(
    db: &'db dyn WorkspaceDataBase,
    owner: hir::hir_def::pous::pou::Pou<'db>,
    body_of: hir::hir_def::pous::pou::Pou<'db>,
) -> Ident {
    let owner_q = qualified_pou_ident(db, Type::new_pou(db, owner));
    let text = match owner == body_of {
        true => format!("{}$__body__", owner_q.text(db)),
        false => format!(
            "{}${}.__body__",
            owner_q.text(db),
            qualified_pou_ident(db, Type::new_pou(db, body_of)).text(db)
        ),
    };
    Ident::new(db, CompactString::from(text))
}

/// The FUNCTION_BLOCK or CLASS that declares `method`.
pub fn declaring_owner<'db>(
    db: &'db dyn WorkspaceDataBase,
    method: hir::hir_def::pous::class::MethodDecl<'db>,
) -> Option<hir::hir_def::pous::pou::Pou<'db>> {
    use hir::hir_def::scope::ScopeKind;
    use hir::hir_def::semantic_index::get_scope;
    let parent = get_scope(db, method.scope_id(db)).parent?;
    match get_scope(db, parent).kind {
        ScopeKind::Pou(pou) => Some(pou),
        _ => None,
    }
}

/// The fragment for an interface specialization's implementer: `@` and its
/// qualified name, which no parameter fragment starts with.
pub fn implementer_fragment(db: &dyn WorkspaceDataBase, implementer: Ident) -> String {
    format!("@{}", implementer.text(db))
}

/// A type as a symbol fragment. Distinct types give distinct fragments,
/// except two unnamed types of the same structure, which resolution cannot
/// tell apart either.
fn type_fragment<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> String {
    match ty {
        Type::Elementary(e) => e.type_name().to_string(),
        Type::DataType(_) | Type::FunctionBlock(_) | Type::Class(_) | Type::Interface(_) => {
            qualified_pou_ident(db, ty).text(db).to_string()
        }
        Type::RefTo(target) => format!("REF_TO({})", type_fragment(db, target.infer(db))),
        Type::Array(array) => {
            let dims: Vec<String> = array_dimensions(db, array)
                .iter()
                .map(|(lo, hi)| format!("{}..{}", bound(*lo), bound(*hi)))
                .collect();
            let of = type_fragment(db, array.of_type(db).infer(db));
            format!("ARRAY[{}]({of})", dims.join(","))
        }
        Type::ArrayConformand(of) => format!("ARRAY[*]({})", type_fragment(db, of.infer(db))),
        Type::SubRange(subrange) => {
            let (lo, hi) = subrange_bounds(db, subrange);
            let base = type_fragment(db, subrange._type(db).infer(db));
            format!("{base}({}..{})", bound(lo), bound(hi))
        }
        Type::Enum(enm) => {
            let base = enm
                .typ(db)
                .map(|spec| type_fragment(db, spec.infer(db)))
                .unwrap_or_default();
            let variants: Vec<String> = enum_ordinals(db, enm)
                .iter()
                .map(|(variant, value)| {
                    format!("{}={}", variant.name.with_case.text(db), bound(*value))
                })
                .collect();
            format!("{base}({})", variants.join(","))
        }
        Type::Struct(s) => {
            let fields: Vec<String> = s
                .elements(db)
                .iter()
                .map(|e| {
                    format!(
                        "{}:{}",
                        e.name_with_case(db).text(db),
                        type_fragment(db, e.spec(db).infer(db))
                    )
                })
                .collect();
            format!("STRUCT({})", fields.join(","))
        }
        // Nothing else types a parameter of a function that passed rk check.
        other => other.kind().to_string(),
    }
}

fn bound(value: Option<i64>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

/// The namespace-qualified name of a POU (bare for a top-level one): the
/// canonical MIR identifier, so `NsA.foo` and `NsB.foo` stay distinct.
pub fn qualified_pou_ident<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: hir::hir_ty::ty::Type<'db>,
) -> Ident {
    use hir::HirNodeInfo;

    // Spelled as declared: a symbol is what exports, test reports and
    // backtraces show, and definition and call both name the declaration.
    let (scope_id, bare) = match ty {
        hir::hir_ty::ty::Type::Function(f) => (f.get_scope_id(db), f.name_with_case(db)),
        hir::hir_ty::ty::Type::FunctionBlock(fb) => (fb.get_scope_id(db), fb.name_with_case(db)),
        hir::hir_ty::ty::Type::Class(c) => (c.get_scope_id(db), c.name_with_case(db)),
        hir::hir_ty::ty::Type::Interface(i) => (i.get_scope_id(db), i.name_with_case(db)),
        hir::hir_ty::ty::Type::DataType(dt) => (dt.get_scope_id(db), dt.name_with_case(db)),
        hir::hir_ty::ty::Type::Program(p) => (p.get_scope_id(db), p.name_with_case(db)),
        _ => return Ident::new(db, CompactString::from("")),
    };

    match hir::hir_ty::resolver::name::enclosing_namespace(db, scope_id) {
        Some(ns) => Ident::new(
            db,
            CompactString::from(format!(
                "{}.{}",
                ns.path_with_case(db).to_string(db),
                bare.text(db)
            )),
        ),
        None => bare,
    }
}
