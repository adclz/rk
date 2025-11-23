use auto_lsp::default::db::{BaseDatabase, file::File};

use crate::hir_def::{
    namespace::NamespaceDecl, pous::{class::MethodDecl, pou::{Pou, PouDecl}, variable::VariableDecl}, semantic_index::get_scope, using::Using, visibility::Visibility
};

#[salsa::tracked(debug)]
pub struct ScopeId<'db> {
    pub file: File,

    pub scope: usize,
}

impl<'db> From<(&'db dyn BaseDatabase, File, usize)> for ScopeId<'db> {
    fn from(data: (&'db dyn BaseDatabase, File, usize)) -> Self {
        ScopeId::new(data.0, data.1, data.2)
    }
}

impl<'db> ScopeId<'db> {
    pub fn global(db: &'db dyn BaseDatabase, file: File) -> Self {
        ScopeId::new(db, file, usize::MAX)
    }

    pub fn is_global(&self, db: &'db dyn BaseDatabase) -> bool {
        self.scope(db) == usize::MAX
    }

        pub fn pous(&self, db: &'db dyn BaseDatabase) -> Option<&Vec<PouDecl<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Namespace(ns) => ns.pous(db),
            _ => None?,
        })
    }

    pub fn methods(&self, db: &'db dyn BaseDatabase) -> Option<&Vec<MethodDecl<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou.pou(db) {
                Pou::Class(cl) => cl.methods(db),
                Pou::FunctionBlock(fb) => fb.methods(db),
                _ => None?,
            },
            _ => None?,
        })
    }

    pub fn variables(&self, db: &'db dyn BaseDatabase) -> Option<&Vec<VariableDecl<'db>>> {
        Some(match get_scope(db, *self).kind {
            ScopeKind::Pou(pou) => match pou.pou(db) {
                Pou::Function(f) => f.variables(db),
                Pou::FunctionBlock(fb) => fb.variables(db),
                Pou::Class(cl) => cl.variables(db),
                _ => None?,
            },
            ScopeKind::MethodDecl(m) => m.variables(db),
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

    pub fn is_namespace(&self) -> bool {
        matches!(self.kind, ScopeKind::Namespace(_))
    }

    pub fn is_pou(&self) -> bool {
        matches!(self.kind, ScopeKind::Pou(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ScopeKind<'db> {
    Global,
    Namespace(NamespaceDecl<'db>),
    Pou(PouDecl<'db>),
    MethodDecl(MethodDecl<'db>)
}
