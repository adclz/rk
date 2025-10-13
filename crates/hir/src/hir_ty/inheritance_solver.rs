use std::sync::Arc;

use crate::{
    hir_def::interned::identifier::Ident,
    hir_ty::ty::{ty_for_method_decl, ty_for_method_prot, ty_for_pou, Ty, TyKind},
};
use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

#[derive(Default, Debug, Clone, PartialEq, Eq, salsa::Update)]
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

fn method_initial<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>) -> Arc<Methods<'db>> {
    Arc::new(Methods::default())
}

fn method_cycle<'db>(
    db: &'db dyn BaseDatabase,
    value: &Arc<Methods<'db>>,
    count: u32,
    ty: Ty<'db>,
) -> salsa::CycleRecoveryAction<Arc<Methods<'db>>> {
    salsa::CycleRecoveryAction::Iterate
}


#[salsa::tracked(cycle_initial = method_initial, cycle_fn=method_cycle)]
pub fn method_table<'db>(db: &'db dyn BaseDatabase, ty: Ty<'db>) -> Arc<Methods<'db>> {
    let mut inherited_methods = FxHashMap::default();
    let mut declared_methods = FxHashMap::default();

    let mut inherited_duplicates = vec![];
    let mut declared_duplicates = vec![];

    match ty.kind(db) {
        TyKind::Class {
            extends,
            implements,
            methods,
            ..
        } => {
            // Inherit base
            if let Some(base) = extends {
                let base = ty_for_pou(db, *base);
                let table2 = method_table(db, base);
                for (name, entry) in &method_table(db, base).declared_methods {
                    let method = InheritedMethod::new(base, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }

            // Inherit interfaces (abstract signatures only)
            for iface in implements {
                let iface = ty_for_pou(db, *iface);
                for (name, entry) in &method_table(db, iface).declared_methods {
                    let method = InheritedMethod::new(iface, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }

            // Add this class’s own methods
            for m in methods {
                let ty = ty_for_method_decl(db, *m);

                if let Some(m) = declared_methods.insert(*m.name(db), ty) {
                    declared_duplicates.push((m, ty));
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

                if let Some(m) = declared_methods.insert(*m.name(db), ty) {
                    declared_duplicates.push((m, ty));
                }
            }

            for iface in implements {
                let iface = ty_for_pou(db, *iface);
                for (name, entry) in &method_table(db, iface).declared_methods {
                    let method = InheritedMethod::new(iface, *entry);
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

                if let Some(m) = declared_methods.insert(*m.name(db), ty) {
                    declared_duplicates.push((m, ty));
                }
            }

            if let Some(base) = extends {
                let base: Ty<'_> = ty_for_pou(db, *base);
                for (name, entry) in &method_table(db, base).declared_methods {
                    let method = InheritedMethod::new(base, *entry);
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
