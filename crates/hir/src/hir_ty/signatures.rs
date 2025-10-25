use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{
    hir_def::{
        interned::identifier::Ident,
        pous::{
            class::Class,
            function::Function,
            function_block::FunctionBlock,
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
    },
    hir_ty::inheritance_solver::MethodRef,
};

pub trait LocalVariables<'db>: Copy {
    fn local_variables(self, db: &'db dyn BaseDatabase) -> &'db IndexMap<Ident, VariableDecl<'db>>;
    
    fn named_parameter_match(&self, db: &'db dyn BaseDatabase, ident: Ident) -> Option<VariableDecl<'db>> {
        self.local_variables(db)
            .get(&ident)
            .copied()
    }

    fn indexed_parameter_match(&self, db: &'db dyn BaseDatabase, index: usize) -> Option<VariableDecl<'db>> {
        self.local_variables(db)
            .values()
            .nth(index)
            .copied()
    }
}

pub trait GlobalVariables<'db>: Copy + LocalVariables<'db> {
    fn global_variables(self, db: &'db dyn BaseDatabase) -> &'db IndexMap<Ident, VariableDecl<'db>>;
}

#[salsa::tracked]
impl<'db> GlobalVariables<'db> for  PouDecl<'db> {
    #[salsa::tracked(returns(ref))]
    fn global_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self.pou(db) {
            Pou::Function(f) => global_variables(db, f.variables(db)),
            Pou::FunctionBlock(fb) => global_variables(db, fb.variables(db)),
            Pou::Class(cl) => global_variables(db, cl.variables(db)),
            Pou::Interface(_) | Pou::DataType(_) => IndexMap::default(),
        }
    }
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for PouDecl<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self.pou(db) {
            Pou::Function(f) => local_variables(db, f.variables(db)),
            Pou::FunctionBlock(fb) => local_variables(db, fb.variables(db)),
            Pou::Class(cl) => local_variables(db, cl.variables(db)),
            Pou::Interface(_) | Pou::DataType(_) => IndexMap::default(),
        }
    }
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for Function<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        local_variables(db, self.variables(db))
    }
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for FunctionBlock<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        local_variables(db, self.variables(db))
    }
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for Class<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        local_variables(db, self.variables(db))
    }
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for MethodRef<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self {
            MethodRef::Prototype(p) => local_variables(db, p.variables(db)),
            MethodRef::Declared(d) => local_variables(db, d.variables(db)),
        }
    }
}

fn global_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> IndexMap<Ident, VariableDecl<'db>> {
    let mut variables = IndexMap::default();
    for v in vars {
        variables.insert(*v.name(db), *v);
    }
    variables
}

fn local_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> IndexMap<Ident, VariableDecl<'db>> {
    let mut variables = IndexMap::default();
    for v in vars {
        match v.kind(db) {
            VariableKind::Input => {
                variables.insert(*v.name(db), *v);
            }
            VariableKind::Output => {
                variables.insert(*v.name(db), *v);
            }
            VariableKind::InOut => {
                variables.insert(*v.name(db), *v);
            }
            _ => continue,
        };
    }
    variables
}
