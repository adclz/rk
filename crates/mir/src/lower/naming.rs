//! Symbol naming: namespace-qualified POU names and the `$`-suffix
//! mangling.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::interned::namespace::NamespacePath;
use hir::hir_def::pous::{function::Function, pou::Pou};
use hir::hir_ty::head::signature::function_signature;
use hir::hir_ty::index_graphs::{namespace_pou_candidates, pou_candidates};
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
/// appended as discriminant when it is overloaded (`SHL$Byte`). Definition
/// and call compute it the same way.
pub fn mir_function_symbol<'db>(db: &'db dyn WorkspaceDataBase, f: Function<'db>) -> Ident {
    let base = qualified_pou_ident(db, Type::Function(f));
    if function_is_overloaded(db, f) {
        let parts: Vec<String> = function_signature(db, f)
            .iter()
            .map(|t| type_mangle(db, t))
            .collect();
        let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        mangle_generic_name(db, base, &refs)
    } else {
        base
    }
}

/// A parameter type as a short symbol fragment: elementary types by IEC
/// name, named types by qualified name, unnamed composites `T`.
fn type_mangle<'db>(db: &'db dyn WorkspaceDataBase, ty: &Type<'db>) -> String {
    match ty {
        Type::Elementary(e) => e.type_name().to_string(),
        other => {
            let q = qualified_pou_ident(db, *other);
            let s = q.text(db);
            if s.is_empty() {
                "T".to_string()
            } else {
                s.replace('.', "_")
            }
        }
    }
}

/// Whether `f`'s name is shared by more than one FUNCTION in its declaring
/// scope (i.e. `f` is part of an overload set).
fn function_is_overloaded<'db>(db: &'db dyn WorkspaceDataBase, f: Function<'db>) -> bool {
    let name = f.name(db);
    let candidates = match function_namespace_path(db, f) {
        Some(path) => namespace_pou_candidates(db, path, name),
        None => pou_candidates(db, name),
    };
    candidates
        .iter()
        .filter(|p| matches!(p, Pou::Function(_)))
        .count()
        > 1
}

/// The namespace path a function is declared in, or `None` for a top-level
/// (global) declaration — mirrors the namespace walk in [`qualified_pou_ident`].
fn function_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    f: Function<'db>,
) -> Option<NamespacePath> {
    use hir::hir_def::scope::ScopeKind;
    use hir::hir_def::semantic_index::semantic_index;

    let scope_id = f.scope_id(db);
    if scope_id.is_global(db) {
        return None;
    }
    for scope in semantic_index(db, scope_id.file(db)).scope_iterator(db, scope_id) {
        if let ScopeKind::Namespace(ns) = scope.kind {
            return Some(*ns.path(db));
        }
    }
    None
}

/// The namespace-qualified name of a POU (bare for a top-level one): the
/// canonical MIR identifier, so `NsA.foo` and `NsB.foo` stay distinct.
pub fn qualified_pou_ident<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: hir::hir_ty::ty::Type<'db>,
) -> Ident {
    use hir::HirNodeInfo;
    use hir::hir_def::scope::ScopeKind;
    use hir::hir_def::semantic_index::semantic_index;

    let (scope_id, bare) = match ty {
        hir::hir_ty::ty::Type::Function(f) => (f.get_scope_id(db), f.name(db)),
        hir::hir_ty::ty::Type::FunctionBlock(fb) => (fb.get_scope_id(db), fb.name(db)),
        hir::hir_ty::ty::Type::Class(c) => (c.get_scope_id(db), c.name(db)),
        hir::hir_ty::ty::Type::Interface(i) => (i.get_scope_id(db), i.name(db)),
        hir::hir_ty::ty::Type::DataType(dt) => (dt.get_scope_id(db), dt.name(db)),
        hir::hir_ty::ty::Type::Program(p) => (p.get_scope_id(db), p.name(db)),
        _ => return Ident::new(db, CompactString::from("")),
    };

    if scope_id.is_global(db) {
        return bare;
    }

    let sema = semantic_index(db, scope_id.file(db));
    for scope in sema.scope_iterator(db, scope_id) {
        if let ScopeKind::Namespace(ns) = scope.kind {
            let ns_str = ns.path(db).to_string(db);
            return Ident::new(
                db,
                CompactString::from(format!("{}.{}", ns_str, bare.text(db))),
            );
        }
    }
    bare
}
