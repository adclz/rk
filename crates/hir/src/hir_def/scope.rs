use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;

use crate::Visibility;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::pous::generics::GenericParam;
use crate::hir_def::pous::interface::MethodPrototype;
use crate::hir_def::program::ProgramDecl;
use crate::hir_def::semantic_index::semantic_index;
use crate::hir_def::{
    namespace::NamespaceDecl,
    pous::{class::MethodDecl, pou::Pou, variable::VariableDecl},
    semantic_index::get_scope,
    using::Using,
};

#[salsa::tracked(debug)]
pub struct ScopeId<'db> {
    pub file: File,

    pub scope: usize,
}

impl<'db> ScopeId<'db> {
    pub fn global(db: &'db dyn WorkspaceDataBase, file: File) -> Self {
        ScopeId::new(db, file, usize::MAX)
    }

    pub fn is_global(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        self.scope(db) == usize::MAX
    }

    pub fn usings(&self, db: &'db dyn WorkspaceDataBase) -> &Vec<Using<'db>> {
        &get_scope(db, *self).usings
    }

    pub fn namespaces(&self, db: &'db dyn WorkspaceDataBase) -> Option<&Vec<NamespaceDecl<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Namespace(ns) => ns.namespaces(db),
            ScopeKind::Global => &semantic_index(db, self.file(db)).global_namespaces,
            _ => None?,
        })
    }

    pub fn pous(&self, db: &'db dyn WorkspaceDataBase) -> Option<&Vec<Pou<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Namespace(ns) => ns.pous(db),
            ScopeKind::Global => &semantic_index(db, self.file(db)).global_pous,
            _ => None?,
        })
    }

    pub fn return_type(&self, db: &'db dyn WorkspaceDataBase) -> Option<&'db Spec<'db>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.return_type(db)?,
                _ => None?,
            },
            ScopeKind::MethodDecl(m) => m.return_type(db)?,
            _ => None?,
        })
    }

    pub fn generics(&self, db: &'db dyn WorkspaceDataBase) -> Option<&'db Vec<GenericParam<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.generics(db),
                Pou::FunctionBlock(fb) => fb.generics(db),
                _ => None?,
            },
            _ => None?,
        })
    }

    pub fn method_declarations(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<&Vec<MethodDecl<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Class(cl) => cl.methods(db),
                Pou::FunctionBlock(fb) => fb.methods(db),
                _ => None?,
            },
            _ => None?,
        })
    }

    pub fn method_prototypes(
        &self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<&Vec<MethodPrototype<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Interface(it) => it.methods(db),
                _ => None?,
            },
            _ => None?,
        })
    }

    pub fn variables<'a, 'b>(
        &'a self,
        db: &'db dyn WorkspaceDataBase,
    ) -> Option<&'db Vec<VariableDecl<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => f.variables(db),
                Pou::FunctionBlock(fb) => fb.variables(db),
                Pou::Class(cl) => cl.variables(db),
                _ => None?,
            },
            ScopeKind::MethodDecl(m) => m.variables(db),
            ScopeKind::MethodProt(m) => m.variables(db),
            ScopeKind::Program(p) => p.variables(db),
            _ => None?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Scope<'db> {
    pub file: File,

    pub usings: Vec<Using<'db>>,

    pub kind: ScopeKind<'db>,

    pub id: ScopeId<'db>,

    // If None, this is the global scope
    pub parent: Option<ScopeId<'db>>,
}

impl<'db> Scope<'db> {
    pub fn new(
        file: File,
        kind: ScopeKind<'db>,
        usings: Vec<Using<'db>>,
        id: ScopeId<'db>,
        visibility: Visibility,
        parent: Option<ScopeId<'db>>,
    ) -> Self {
        Self {
            file,
            usings,
            kind,
            id,
            parent,
        }
    }

    pub fn is_global(&self) -> bool {
        matches!(self.kind, ScopeKind::Global)
    }

    pub fn is_program(&self) -> bool {
        matches!(self.kind, ScopeKind::Program(_))
    }

    pub fn is_namespace(&self) -> bool {
        matches!(self.kind, ScopeKind::Namespace(_))
    }

    pub fn is_pou(&self) -> bool {
        matches!(self.kind, ScopeKind::Pou(_))
    }

    pub fn is_method_decl(&self) -> bool {
        matches!(self.kind, ScopeKind::MethodDecl(_))
    }

    pub fn is_method_prot(&self) -> bool {
        matches!(self.kind, ScopeKind::MethodProt(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ScopeKind<'db> {
    Global,
    Program(ProgramDecl<'db>),
    Namespace(NamespaceDecl<'db>),
    Pou(Pou<'db>),
    MethodDecl(MethodDecl<'db>),
    MethodProt(MethodPrototype<'db>),
}
