use std::sync::Arc;

use crate::{
    hir_def::interned::identifier::Ident,
    hir_ty::ty::{Ty, TyDecl, TyKind, ty_for_method_decl, ty_for_method_prot},
};
use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Methods<'db> {
    pub inherited_methods: FxHashMap<Ident, InheritedMethod<'db>>,
    pub declared_methods: FxHashMap<Ident, Ty<'db>>,

    pub inherited_duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
    pub declared_duplicates: Vec<(Ty<'db>, Ty<'db>)>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, salsa::Update)]
pub struct InheritedMethod<'db> {
    pub source: Ty<'db>,
    pub method: Ty<'db>,
}

impl<'db> InheritedMethod<'db> {
    fn new(source: Ty<'db>, method: Ty<'db>) -> Self {
        Self { source, method }
    }
}

#[salsa::tracked]
pub fn method_table<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>) -> Arc<Methods<'db>> {
    let mut inherited_methods = FxHashMap::default();
    let mut declared_methods = FxHashMap::default();

    let mut inherited_duplicates = vec![];
    let mut declared_duplicates = vec![];

    match ty.kind(db) {
        TyKind::Target(target) => return method_table(db, *target),
        TyKind::Class {
            extends,
            implements,
            methods,
            ..
        } => {
            // Inherit base
            if let Some(base) = extends {
                let table2 = method_table(db, *base);
                for (name, entry) in &method_table(db, *base).declared_methods {
                    let method = InheritedMethod::new(*base, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }

            // Inherit interfaces (abstract signatures only)
            for iface in implements {
                for (name, entry) in &method_table(db, *iface).declared_methods {
                    let method = InheritedMethod::new(*iface, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }

            // Add this class’s own methods
            for m in methods {
                let ty = ty_for_method_decl(db, *m);
                let target = Ty::new(db, TyDecl::Method(*m), ty.def(db), TyKind::Target(ty));

                if let Some(m) = declared_methods.insert(*m.name(db), target) {
                    declared_duplicates.push((m, target));
                }
            }
        }

        TyKind::Interface {
            implements,
            methods,
        } => {
            // Methods = abstract signatures
            for m in methods {
                let ty = ty_for_method_prot(db, *m);
                let target = Ty::new(db, TyDecl::MethodProt(*m), ty.def(db), TyKind::Target(ty));

                if let Some(m) = declared_methods.insert(*m.name(db), target) {
                    declared_duplicates.push((m, target));
                }
            }

            for iface in implements {
                for (name, entry) in &method_table(db, *iface).declared_methods {
                    let method = InheritedMethod::new(*iface, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }
        }

        TyKind::FunctionBlock {
            extends, methods, ..
        } => {
            for m in methods {
                let ty = ty_for_method_decl(db, *m);
                let target = Ty::new(db, TyDecl::Method(*m), ty.def(db), TyKind::Target(ty));

                if let Some(m) = declared_methods.insert(*m.name(db), target) {
                    declared_duplicates.push((m, target));
                }
            }

            if let Some(base) = extends {
                for (name, entry) in &method_table(db, *base).declared_methods {
                    let method = InheritedMethod::new(*base, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }
        }
        _ => {}
    }

    Arc::new(Methods {
        inherited_methods,
        declared_methods,
        inherited_duplicates,
        declared_duplicates,
    })
}
