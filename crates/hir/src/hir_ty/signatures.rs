use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    hir_def::{
        interned::identifier::Ident,
        pous::{
            class::MethodDecl,
            function::Function,
            function_block::FunctionBlock,
            interface::MethodPrototype,
            variable::{VariableDecl, VariableKind},
        },
    },
    hir_ty::ty::Ty,
};

pub trait HasSignature<'db>: Copy {
    fn signature(self, db: &'db dyn BaseDatabase) -> &'db IndexMap<Ident, Ty<'db>>;
}

#[salsa::tracked]
impl<'db> HasSignature<'db> for Function<'db> {
    #[salsa::tracked(returns(ref))]
    fn signature(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        fetch_variables(db, &self.variables(db))
    }
}

#[salsa::tracked]
impl<'db> HasSignature<'db> for FunctionBlock<'db> {
    #[salsa::tracked(returns(ref))]
    fn signature(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        fetch_variables(db, &self.variables(db))
    }
}

#[salsa::tracked]
impl<'db> HasSignature<'db> for MethodDecl<'db> {
    #[salsa::tracked(returns(ref))]
    fn signature(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        fetch_variables(db, &self.variables(db))
    }
}

#[salsa::tracked]
impl<'db> HasSignature<'db> for MethodPrototype<'db> {
    #[salsa::tracked(returns(ref))]

    fn signature(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, Ty<'db>> {
        fetch_variables(db, &self.variables(db))
    }
}

fn fetch_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> IndexMap<Ident, Ty<'db>> {
    let mut variables = IndexMap::default();
    for v in vars {
        match v.kind(db) {
            VariableKind::Input => {
                variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
            }
            VariableKind::Output => {
                variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
            }
            VariableKind::InOut => {
                variables.insert(*v.name(db), v.spec(db).spec_to_ty(db));
            }
            _ => continue,
        };
    }
    variables
}
