//! Symbol naming: namespace-qualified POU names and the `$`-suffix
//! mangling.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::interned::namespace::NamespacePath;
use hir::hir_def::pous::{function::Function, pou::Pou};
use hir::hir_ty::index_graphs::{namespace_pou_candidates, pou_candidates};

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

/// The MIR-level symbol for a FUNCTION, used for `function_indices`, call
/// routing, and the export-name fallback.
///
/// A non-overloaded name maps to its plain qualified name (`foo`, `Ns.foo`) — so
/// exports and existing symbols are unchanged. A FUNCTION that shares its name
/// with other overloads gets the arity discriminant appended (`foo$2`), giving
/// each overload a distinct symbol. Call sites resolve the specific overload in
/// HIR, so definition and call compute the same symbol here.
pub fn mir_function_symbol<'db>(db: &'db dyn WorkspaceDataBase, f: Function<'db>) -> Ident {
    let base = qualified_pou_ident(db, hir::hir_ty::ty::Type::Function(f));
    if function_is_overloaded(db, f) {
        let arity = f.param_count(db).to_string();
        mangle_generic_name(db, base, &[&arity])
    } else {
        base
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
