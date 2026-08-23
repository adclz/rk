//! Symbol naming: namespace-qualified POU names and the `$`-suffix
//! mangling.

use compact_str::CompactString;
use db::WorkspaceDataBase;
use hir::hir_def::interned::identifier::Ident;
use hir::hir_def::pous::function::Function;
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
    // What discriminates the symbol is resolution's decision.
    match hir::hir_ty::resolver::name::overload_discriminant(db, f) {
        Some(types) => {
            let parts: Vec<String> = types.iter().map(|t| type_mangle(db, t)).collect();
            let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
            mangle_generic_name(db, base, &refs)
        }
        None => base,
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




/// The namespace-qualified name of a POU (bare for a top-level one): the
/// canonical MIR identifier, so `NsA.foo` and `NsB.foo` stay distinct.
pub fn qualified_pou_ident<'db>(
    db: &'db dyn WorkspaceDataBase,
    ty: hir::hir_ty::ty::Type<'db>,
) -> Ident {
    use hir::HirNodeInfo;

    let (scope_id, bare) = match ty {
        hir::hir_ty::ty::Type::Function(f) => (f.get_scope_id(db), f.name(db)),
        hir::hir_ty::ty::Type::FunctionBlock(fb) => (fb.get_scope_id(db), fb.name(db)),
        hir::hir_ty::ty::Type::Class(c) => (c.get_scope_id(db), c.name(db)),
        hir::hir_ty::ty::Type::Interface(i) => (i.get_scope_id(db), i.name(db)),
        hir::hir_ty::ty::Type::DataType(dt) => (dt.get_scope_id(db), dt.name(db)),
        hir::hir_ty::ty::Type::Program(p) => (p.get_scope_id(db), p.name(db)),
        _ => return Ident::new(db, CompactString::from("")),
    };

    match hir::hir_ty::resolver::name::enclosing_namespace_path(db, scope_id) {
        Some(path) => Ident::new(
            db,
            CompactString::from(format!("{}.{}", path.to_string(db), bare.text(db))),
        ),
        None => bare,
    }
}
