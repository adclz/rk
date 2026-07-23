//! Symbol naming: namespace-qualified POU names and the `$`-suffix
//! mangling.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;

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
