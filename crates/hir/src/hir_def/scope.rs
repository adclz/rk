use auto_lsp::default::db::{BaseDatabase, file::File};

use crate::hir_def::{
    namespace::NamespaceDecl, pous::pou::PouDecl, using::Using, visibility::Visibility,
};

#[salsa::tracked(debug)]
pub struct FileScopeId<'db> {
    pub file: File,

    pub scope: usize,
}

impl<'db> From<(&'db dyn BaseDatabase, File, usize)> for FileScopeId<'db> {
    fn from(data: (&'db dyn BaseDatabase, File, usize)) -> Self {
        FileScopeId::new(data.0, data.1, data.2)
    }
}

impl<'db> FileScopeId<'db> {
    pub fn global(db: &'db dyn BaseDatabase, file: File) -> Self {
        FileScopeId::new(db, file, usize::MAX)
    }

    pub fn is_global(&self, db: &'db dyn BaseDatabase) -> bool {
        self.scope(db) == usize::MAX
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct Scope<'db> {
    pub file: File,

    // from the standard: "A USING namespace directive enables the types contained in the given namespace,
    // but specifically does not enable types contained in nested namespaces."

    // TLDR: Using directives are not recursive
    pub usings: Vec<Using<'db>>,

    pub kind: ScopeKind<'db>,

    pub id: FileScopeId<'db>,

    // If None, this is the global scope
    pub parent: Option<FileScopeId<'db>>,

    pub visibility: Visibility,
}

impl<'db> Scope<'db> {
    pub fn new(
        file: File,
        kind: ScopeKind<'db>,
        usings: Vec<Using<'db>>,
        id: FileScopeId<'db>,
        visibility: Visibility,
        parent: Option<FileScopeId<'db>>,
    ) -> Self {
        Self {
            file,
            usings,
            kind,
            visibility,
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
}
